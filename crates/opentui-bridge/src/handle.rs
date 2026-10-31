#![forbid(unsafe_code)]
//! Native ABI handle + render status (mirrors `lib.zig`).

/// Zig `NativeHandle` (= `handles.Handle`): 32-bit slot. 0 = invalid.
pub type NativeHandle = u32;
/// Zig `INVALID_HANDLE`.
pub const INVALID_HANDLE: u32 = 0;

/// Zig `renderer.RenderStatus` discriminant, returned as `u8` by `render`.
pub type RenderStatus = u8;
/// Render produced output.
pub const RENDER_STATUS_RENDERED: RenderStatus = 0;
/// Render skipped (nothing changed).
pub const RENDER_STATUS_SKIPPED: RenderStatus = 1;
/// Render failed (e.g. invalid handle).
pub const RENDER_STATUS_FAILED: RenderStatus = 2;

/// Fail-closed discriminant check.
#[must_use]
pub const fn render_status_is_known(status: u8) -> bool {
    matches!(status, 0..=2)
}

/// Opaque single-owner bridge handle (mirrors `Renderer` claim pattern).
///
/// Exactly one live [`BridgeHandle`] at a time, enforced by `CLAIMED`.
/// `Copy`/`Clone` are deliberately absent: ownership moves, never duplicates.
/// Use [`BridgeHandle::claim`] to take the slot, [`BridgeHandle::close`] to
/// release early; [`Drop`] releases otherwise. After close/drop the slot is
/// free and any `live()` check fails [`BridgeError::InvalidHandle`].
#[derive(Debug)]
pub struct BridgeHandle {
    raw: NativeHandle,
    _no_send: std::marker::PhantomData<*const ()>,
}

use std::sync::atomic::{AtomicBool, Ordering};

use crate::safe_renderer::BridgeError;

/// Global single-owner claim. Held while a [`BridgeHandle`] is live.
static CLAIMED: AtomicBool = AtomicBool::new(false);

impl BridgeHandle {
    /// Take the single-owner slot for `raw`. Fails `InvalidHandle` on
    /// [`INVALID_HANDLE`] (claim untouched) or `AlreadyLive` when held.
    pub fn claim(raw: NativeHandle) -> Result<Self, BridgeError> {
        if raw == INVALID_HANDLE {
            return Err(BridgeError::InvalidHandle);
        }
        if CLAIMED.swap(true, Ordering::AcqRel) {
            return Err(BridgeError::AlreadyLive);
        }
        Ok(Self {
            raw,
            _no_send: std::marker::PhantomData,
        })
    }

    /// Borrow the live raw slot, or `InvalidHandle` once closed.
    pub fn live(&self) -> Result<NativeHandle, BridgeError> {
        if self.raw == INVALID_HANDLE {
            return Err(BridgeError::InvalidHandle);
        }
        Ok(self.raw)
    }

    /// Slot still owned (not yet closed).
    #[must_use]
    pub fn is_live(&self) -> bool {
        self.raw != INVALID_HANDLE
    }

    fn release(&mut self) {
        if self.raw == INVALID_HANDLE {
            return;
        }
        self.raw = INVALID_HANDLE;
        CLAIMED.store(false, Ordering::Release);
    }

    /// Release early and free the claim; further `live()` fails
    /// `InvalidHandle`. Idempotent.
    pub fn close(&mut self) {
        self.release();
    }
}

impl Drop for BridgeHandle {
    fn drop(&mut self) {
        self.release();
    }
}

const _: () = assert!(std::mem::size_of::<NativeHandle>() == 4);
const _: () = assert!(std::mem::size_of::<RenderStatus>() == 1);

// ---------------------------------------------------------------------------
// Generational resource handles (FFI-03).
//
// The audited OpenTUI header describes handles as `(context id, slot,
// generation)` and states that copying a handle does not retain it and that a
// stale generation must fail closed. The vendored library does not export the
// checked context/slot API (verified: no `context*`/`slot*` exports), so this
// is a Rust-side model of the same contract: packed `(slot, generation)` in a
// `u32`, 0 reserved for "never valid".
// ---------------------------------------------------------------------------

/// Bits reserved for the arena slot index.
pub const SLOT_BITS: u32 = 20;
/// Bits reserved for the generation counter.
pub const GENERATION_BITS: u32 = 12;
/// Highest usable slot index (`2^20 - 1`).
pub const SLOT_MAX: u32 = (1 << SLOT_BITS) - 1;
/// Highest generation before a slot is retired.
pub const GENERATION_MAX: u32 = (1 << GENERATION_BITS) - 1;
/// Bounded resource capacity of one [`Arena`] (fail-closed, no unbounded map).
pub const MAX_RESOURCES: usize = 4096;

/// Packed slot + generation handle. `GenHandle::INVALID` (0) never resolves.
///
/// Copying a `GenHandle` is fine: it is an identifier, not ownership. Ownership
/// lives in the [`Arena`]; a copied handle stops resolving as soon as the
/// owning entry is removed (generation bump).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GenHandle(u32);

impl GenHandle {
    /// Sentinel that never resolves (`raw == 0`).
    pub const INVALID: GenHandle = GenHandle(0);

    /// Pack `slot`/`generation`. `None` when the slot is out of range or the
    /// generation is 0 (reserved) or 1-based above [`GENERATION_MAX`].
    #[must_use]
    pub const fn new(slot: u32, generation: u32) -> Option<Self> {
        if slot > SLOT_MAX || generation == 0 || generation > GENERATION_MAX {
            return None;
        }
        Some(Self((generation << SLOT_BITS) | slot))
    }

    /// Rebuild from the packed wire value. `None` for 0 / generation 0.
    #[must_use]
    pub const fn from_bits(raw: u32) -> Option<Self> {
        let handle = Self(raw);
        if raw == 0 || handle.generation() == 0 {
            None
        } else {
            Some(handle)
        }
    }

    /// Packed wire value.
    #[must_use]
    pub const fn to_bits(self) -> u32 {
        self.0
    }

    /// Slot index inside the owning arena.
    #[must_use]
    pub const fn slot(self) -> u32 {
        self.0 & SLOT_MAX
    }

    /// Generation counter (1-based).
    #[must_use]
    pub const fn generation(self) -> u32 {
        self.0 >> SLOT_BITS
    }

    /// Never true for [`GenHandle::INVALID`].
    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.0 != 0 && self.generation() != 0
    }
}

impl Default for GenHandle {
    fn default() -> Self {
        Self::INVALID
    }
}

/// Fail-closed resource errors (distinct from [`BridgeError`] so callers can
/// tell a lifetime violation from a renderer failure).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceError {
    /// Handle was `0` / never allocated.
    NullHandle,
    /// Slot exists but its generation moved on (resource replaced/released).
    StaleHandle,
    /// Arena is at [`MAX_RESOURCES`].
    Full,
    /// Resource touched from a thread other than its creating thread.
    WrongThread,
    /// Shared value still has other owners; exclusive access is impossible.
    Shared,
    /// Value construction failed; no handle was allocated.
    Construct,
}

impl core::fmt::Display for ResourceError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::NullHandle => "null resource handle",
            Self::StaleHandle => "stale resource handle",
            Self::Full => "resource arena full",
            Self::WrongThread => "resource used from non-owner thread",
            Self::Shared => "resource is still shared",
            Self::Construct => "resource construction failed",
        })
    }
}

impl std::error::Error for ResourceError {}

struct Slot<T> {
    generation: u32,
    value: Option<T>,
}

/// Bounded generational arena: the owned side of [`GenHandle`].
///
/// Bound to its creating thread, mirroring the native context rule that a
/// context may only be destroyed/used from the thread that created it.
pub struct Arena<T> {
    slots: Vec<Slot<T>>,
    free: Vec<u32>,
    len: usize,
    owner: std::thread::ThreadId,
}

impl<T> Arena<T> {
    /// Empty arena owned by the calling thread.
    #[must_use]
    pub fn new() -> Self {
        Self {
            slots: Vec::new(),
            free: Vec::new(),
            len: 0,
            owner: std::thread::current().id(),
        }
    }

    /// Owner-thread check. Every public operation below starts here.
    fn check_owner(&self) -> Result<(), ResourceError> {
        if self.owner != std::thread::current().id() {
            return Err(ResourceError::WrongThread);
        }
        Ok(())
    }

    /// Number of live resources.
    #[must_use]
    pub fn len(&self) -> usize {
        self.len
    }

    /// True when no resource is live.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Allocate `value`. The slot's generation starts (or resumes) at 1 and is
    /// bumped on every removal, so a handle from a released resource cannot
    /// alias a later one.
    pub fn insert(&mut self, value: T) -> Result<GenHandle, ResourceError> {
        self.check_owner()?;
        if let Some(slot) = self.free.pop() {
            let entry = &mut self.slots[slot as usize];
            entry.value = Some(value);
            self.len += 1;
            return GenHandle::new(slot, entry.generation).ok_or(ResourceError::StaleHandle);
        }
        if self.slots.len() >= MAX_RESOURCES {
            return Err(ResourceError::Full);
        }
        let slot = self.slots.len() as u32;
        self.slots.push(Slot {
            generation: 1,
            value: Some(value),
        });
        self.len += 1;
        GenHandle::new(slot, 1).ok_or(ResourceError::StaleHandle)
    }

    fn resolve(&self, handle: GenHandle) -> Result<usize, ResourceError> {
        if !handle.is_valid() {
            return Err(ResourceError::NullHandle);
        }
        let slot = handle.slot() as usize;
        let entry = self.slots.get(slot).ok_or(ResourceError::StaleHandle)?;
        if entry.generation != handle.generation() || entry.value.is_none() {
            return Err(ResourceError::StaleHandle);
        }
        Ok(slot)
    }

    /// Borrow a live resource.
    pub fn get(&self, handle: GenHandle) -> Result<&T, ResourceError> {
        self.check_owner()?;
        let slot = self.resolve(handle)?;
        self.slots[slot]
            .value
            .as_ref()
            .ok_or(ResourceError::StaleHandle)
    }

    /// Mutably borrow a live resource.
    pub fn get_mut(&mut self, handle: GenHandle) -> Result<&mut T, ResourceError> {
        self.check_owner()?;
        let slot = self.resolve(handle)?;
        self.slots[slot]
            .value
            .as_mut()
            .ok_or(ResourceError::StaleHandle)
    }

    /// True when `handle` still resolves (owner thread only).
    pub fn contains(&self, handle: GenHandle) -> bool {
        self.check_owner().is_ok() && self.resolve(handle).is_ok()
    }

    /// Remove and return the value, bumping the slot generation.
    ///
    /// A slot whose generation would exceed [`GENERATION_MAX`] is retired
    /// instead of wrapping (guaranteeing no stale handle resurrection).
    pub fn remove(&mut self, handle: GenHandle) -> Result<T, ResourceError> {
        self.check_owner()?;
        let slot = self.resolve(handle)?;
        let entry = &mut self.slots[slot];
        let value = entry.value.take().ok_or(ResourceError::StaleHandle)?;
        self.len -= 1;
        if entry.generation >= GENERATION_MAX {
            // Retired: keep the slot allocated but empty and never reuse it.
            entry.generation = GENERATION_MAX;
        } else {
            entry.generation += 1;
            self.free.push(slot as u32);
        }
        Ok(value)
    }

    /// Live handles in slot order.
    pub fn handles(&self) -> Vec<GenHandle> {
        let mut out = Vec::with_capacity(self.len);
        for (slot, entry) in self.slots.iter().enumerate() {
            if entry.value.is_some() {
                if let Some(handle) = GenHandle::new(slot as u32, entry.generation) {
                    out.push(handle);
                }
            }
        }
        out
    }
}

impl<T> Default for Arena<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: core::fmt::Debug> core::fmt::Debug for Arena<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Arena").field("len", &self.len).finish()
    }
}

/// Fallible construction: allocates only when `build` succeeds, so a failed
/// native construction never leaves a live handle behind.
pub fn try_construct<T, F>(arena: &mut Arena<T>, build: F) -> Result<GenHandle, ResourceError>
where
    F: FnOnce() -> Option<T>,
{
    arena.check_owner()?;
    let value = build().ok_or(ResourceError::Construct)?;
    arena.insert(value)
}

/// Shared ownership of one native-backed value (Zig refcounted resources).
///
/// Clones are explicit through [`SharedResource::share`]; `Deref` gives
/// read-only access. Exclusive access requires [`SharedResource::try_unwrap`]
/// to succeed, which fails while any other owner exists.
pub struct SharedResource<T> {
    inner: std::rc::Rc<T>,
}

impl<T> SharedResource<T> {
    /// First owner.
    #[must_use]
    pub fn new(value: T) -> Self {
        Self {
            inner: std::rc::Rc::new(value),
        }
    }

    /// Add one owner.
    #[must_use]
    pub fn share(&self) -> Self {
        Self {
            inner: std::rc::Rc::clone(&self.inner),
        }
    }

    /// Number of live owners.
    #[must_use]
    pub fn strong_count(&self) -> usize {
        std::rc::Rc::strong_count(&self.inner)
    }

    /// True when this is the only owner.
    #[must_use]
    pub fn is_unique(&self) -> bool {
        self.strong_count() == 1
    }

    /// Take exclusive ownership, or hand the shared value back.
    pub fn try_unwrap(self) -> Result<T, Self> {
        match std::rc::Rc::try_unwrap(self.inner) {
            Ok(value) => Ok(value),
            Err(inner) => Err(Self { inner }),
        }
    }

    /// Explicitly tagged borrow that keeps the originating handle visible.
    #[must_use]
    pub fn borrow(&self, handle: GenHandle) -> Borrowed<'_, T> {
        Borrowed {
            handle,
            value: &self.inner,
        }
    }
}

impl<T> Clone for SharedResource<T> {
    fn clone(&self) -> Self {
        self.share()
    }
}

impl<T> std::ops::Deref for SharedResource<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.inner
    }
}

impl<T: core::fmt::Debug> core::fmt::Debug for SharedResource<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SharedResource")
            .field("strong_count", &self.strong_count())
            .finish()
    }
}

/// Non-owning view of a resource plus the handle it came from.
#[derive(Debug, Clone, Copy)]
pub struct Borrowed<'a, T> {
    handle: GenHandle,
    value: &'a T,
}

impl<'a, T> Borrowed<'a, T> {
    /// Pair a reference with its handle (callers get this from
    /// [`SharedResource::borrow`] or an [`Arena`] lookup).
    #[must_use]
    pub const fn new(handle: GenHandle, value: &'a T) -> Self {
        Self { handle, value }
    }

    /// Borrowed value.
    #[must_use]
    pub const fn get(&self) -> &'a T {
        self.value
    }

    /// Handle the borrow was taken from.
    #[must_use]
    pub const fn handle(&self) -> GenHandle {
        self.handle
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::size_of;
    use std::sync::Mutex;

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn handle_sizes_stable() {
        assert_eq!(size_of::<NativeHandle>(), 4);
        assert_eq!(size_of::<RenderStatus>(), 1);
    }

    #[test]
    fn invalid_handle_is_zero() {
        assert_eq!(INVALID_HANDLE, 0);
        assert_ne!(1 as NativeHandle, INVALID_HANDLE);
    }

    #[test]
    fn render_status_known_only_for_defined() {
        assert_eq!(
            (
                RENDER_STATUS_RENDERED,
                RENDER_STATUS_SKIPPED,
                RENDER_STATUS_FAILED
            ),
            (0, 1, 2)
        );
        assert!(render_status_is_known(RENDER_STATUS_RENDERED));
        assert!(render_status_is_known(RENDER_STATUS_SKIPPED));
        assert!(render_status_is_known(RENDER_STATUS_FAILED));
        assert!(!render_status_is_known(3));
        assert!(!render_status_is_known(u8::MAX));
    }

    #[test]
    fn claim_release_cycle() {
        let _guard = TEST_LOCK.lock().unwrap();
        let mut h = BridgeHandle::claim(7).expect("claim");
        assert!(h.is_live());
        assert_eq!(h.live(), Ok(7));
        h.close();
        assert!(!h.is_live());
        assert_eq!(h.live(), Err(BridgeError::InvalidHandle));
        // Slot freed: reclaim works.
        let _again = BridgeHandle::claim(9).expect("reclaim after close");
    }

    #[test]
    fn double_claim_rejected() {
        let _guard = TEST_LOCK.lock().unwrap();
        let _held = BridgeHandle::claim(7).expect("first claim");
        assert_eq!(
            BridgeHandle::claim(8).unwrap_err(),
            BridgeError::AlreadyLive
        );
    }

    #[test]
    fn zero_raw_rejected_without_touching_claim() {
        let _guard = TEST_LOCK.lock().unwrap();
        assert_eq!(
            BridgeHandle::claim(INVALID_HANDLE).unwrap_err(),
            BridgeError::InvalidHandle
        );
        // Claim untouched, still free.
        let _held = BridgeHandle::claim(7).expect("claim after zero rejected");
    }

    #[test]
    fn close_idempotent() {
        let _guard = TEST_LOCK.lock().unwrap();
        let mut h = BridgeHandle::claim(7).expect("claim");
        h.close();
        h.close();
        assert_eq!(h.live(), Err(BridgeError::InvalidHandle));
    }

    #[test]
    fn drop_releases_claim() {
        let _guard = TEST_LOCK.lock().unwrap();
        {
            let _held = BridgeHandle::claim(7).expect("claim");
        }
        let _again = BridgeHandle::claim(8).expect("claim after drop");
    }

    #[test]
    fn closed_handle_rejected() {
        let _guard = TEST_LOCK.lock().unwrap();
        let mut h = BridgeHandle::claim(7).expect("claim");
        h.close();
        assert_eq!(h.live().unwrap_err(), BridgeError::InvalidHandle);
    }

    #[test]
    fn gen_handle_packing_roundtrip() {
        let zero = GenHandle::new(0, 1).expect("slot 0 gen 1");
        assert_ne!(zero.to_bits(), 0, "slot 0 must still pack non-zero");
        assert_eq!(zero.slot(), 0);
        assert_eq!(zero.generation(), 1);
        assert_eq!(GenHandle::from_bits(zero.to_bits()), Some(zero));

        let high = GenHandle::new(SLOT_MAX, GENERATION_MAX).expect("max slot/gen");
        assert_eq!(high.slot(), SLOT_MAX);
        assert_eq!(high.generation(), GENERATION_MAX);
        assert_eq!(high.to_bits(), u32::MAX);
        assert_eq!(GenHandle::from_bits(u32::MAX), Some(high));

        assert_eq!(GenHandle::new(SLOT_MAX + 1, 1), None, "slot overflow");
        assert_eq!(GenHandle::new(0, 0), None, "generation 0 reserved");
        assert_eq!(GenHandle::new(0, GENERATION_MAX + 1), None, "gen overflow");
        assert_eq!(GenHandle::from_bits(0), None);
        assert!(!GenHandle::INVALID.is_valid());
        assert_eq!(GenHandle::default(), GenHandle::INVALID);
    }

    #[test]
    fn arena_stale_generation_rejected() {
        let mut arena = Arena::new();
        let first = arena.insert("a").expect("insert");
        assert_eq!(arena.get(first), Ok(&"a"));
        assert_eq!(arena.remove(first), Ok("a"));
        assert_eq!(arena.get(first), Err(ResourceError::StaleHandle));
        assert_eq!(arena.remove(first), Err(ResourceError::StaleHandle));

        // The freed slot is reused with a bumped generation: the old handle
        // must not alias the new resource.
        let second = arena.insert("b").expect("reinsert");
        assert_eq!(second.slot(), first.slot(), "slot reused");
        assert_ne!(second, first, "generation bumped");
        assert_eq!(arena.get(second), Ok(&"b"));
        assert_eq!(arena.get(first), Err(ResourceError::StaleHandle));
        assert!(arena.contains(second));
        assert!(!arena.contains(first));
    }

    #[test]
    fn arena_unknown_and_null_handles_rejected() {
        let arena: Arena<u8> = Arena::new();
        assert_eq!(arena.get(GenHandle::INVALID), Err(ResourceError::NullHandle));
        assert_eq!(
            arena.get(GenHandle::new(9, 1).unwrap()),
            Err(ResourceError::StaleHandle)
        );
        assert!(!arena.contains(GenHandle::INVALID));
    }

    #[test]
    fn arena_len_handles_and_is_empty() {
        let mut arena = Arena::new();
        assert!(arena.is_empty());
        let a = arena.insert(1u8).unwrap();
        let b = arena.insert(2u8).unwrap();
        assert_eq!(arena.len(), 2);
        assert_eq!(arena.handles(), vec![a, b]);
        *arena.get_mut(b).unwrap() = 9;
        assert_eq!(arena.get(b), Ok(&9));
        arena.remove(a).unwrap();
        assert_eq!(arena.len(), 1);
        assert_eq!(arena.handles(), vec![b]);
    }

    #[test]
    fn arena_is_bounded() {
        let mut arena: Arena<u32> = Arena::new();
        for i in 0..MAX_RESOURCES {
            arena.insert(i as u32).expect("within bound");
        }
        assert_eq!(arena.insert(0), Err(ResourceError::Full));
        arena.remove(arena.handles()[0]).expect("free one");
        assert!(arena.insert(0).is_ok(), "freed slot reusable");
    }

    #[test]
    fn arena_rejects_other_threads() {
        let mut arena: Arena<u8> = Arena::new();
        let handle = arena.insert(1).unwrap();
        // Moving the arena is allowed; using it off the creating thread is not.
        let (arena, observed) = std::thread::scope(|scope| {
            scope
                .spawn(move || {
                    let observed = (
                        arena.get(handle).copied(),
                        arena.contains(handle),
                        arena.remove(handle),
                    );
                    (arena, observed)
                })
                .join()
                .unwrap()
        });
        assert_eq!(observed.0, Err(ResourceError::WrongThread));
        assert!(!observed.1, "contains is false off-thread");
        assert_eq!(observed.2, Err(ResourceError::WrongThread));
        // Owner thread still sees the untouched resource.
        assert_eq!(arena.get(handle), Ok(&1));
    }

    #[test]
    fn arena_retires_exhausted_generation() {
        let mut arena = Arena::new();
        let handle = arena.insert(1u8).unwrap();
        // Fast-forward the slot to the last generation.
        let slot = handle.slot();
        arena.slots[slot as usize].generation = GENERATION_MAX;
        let last = GenHandle::new(slot, GENERATION_MAX).unwrap();
        let retired = arena.remove(last).unwrap();
        assert_eq!(retired, 1);
        let next = arena.insert(2u8).unwrap();
        assert_ne!(next.slot(), handle.slot(), "exhausted slot never reused");
        assert_eq!(arena.get(handle), Err(ResourceError::StaleHandle));
    }

    #[test]
    fn failed_construction_leaves_no_handle() {
        let mut arena = Arena::new();
        assert_eq!(
            try_construct(&mut arena, || None::<u8>),
            Err(ResourceError::Construct)
        );
        assert!(arena.is_empty());
        assert!(arena.handles().is_empty());
        let handle = try_construct(&mut arena, || Some(7u8)).unwrap();
        assert_eq!(arena.get(handle), Ok(&7));
    }

    #[test]
    fn shared_resource_explicit_lifetime() {
        let shared = SharedResource::new(5u32);
        assert!(shared.is_unique());
        let alias = shared.share();
        assert_eq!(shared.strong_count(), 2);
        assert!(!shared.is_unique());
        assert_eq!(*alias, 5);
        // Borrow keeps the originating handle visible.
        let borrowed = shared.borrow(GenHandle::new(3, 2).unwrap());
        assert_eq!(*borrowed.get(), 5);
        assert_eq!(borrowed.handle().slot(), 3);
        // Exclusive ownership requires uniqueness.
        let shared = shared.try_unwrap().expect_err("still shared");
        drop(alias);
        assert_eq!(shared.try_unwrap().ok(), Some(5));
    }
}

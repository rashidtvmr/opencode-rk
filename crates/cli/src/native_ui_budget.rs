//! Aggregate limits for one presentation-config/theme loading operation.
#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};

pub const MAX_PATHS: usize = 64;
pub const MAX_PATH_BYTES: usize = 32 * 1024;
pub const MAX_READ_BYTES: usize = 2 * 1024 * 1024;

/// Concurrent animated rows (session spinner, compaction spinner, tool rows)
/// allowed to request repaint timers. Beyond this the row paints static
/// animations-off frames instead of adding timers.
pub const MAX_ACTIVE_ANIMATIONS: usize = 64;

/// Coalesced repaint requests one render tick may service.
pub const MAX_REPAINTS_PER_TICK: usize = MAX_ACTIVE_ANIMATIONS;

#[derive(Default)]
pub struct ReadBudget {
    paths: usize,
    path_bytes: usize,
    read_bytes: usize,
}

impl ReadBudget {
    pub fn check_path(&mut self, path: &Path) -> Result<(), String> {
        let paths = self.paths.saturating_add(1);
        let bytes = self.path_bytes.saturating_add(path.as_os_str().len());
        if paths > MAX_PATHS || bytes > MAX_PATH_BYTES {
            return Err("TUI config/theme aggregate path budget exceeded".into());
        }
        self.paths = paths;
        self.path_bytes = bytes;
        Ok(())
    }

    pub fn read(
        &mut self,
        path: &Path,
        per_file: usize,
        label: &str,
    ) -> Result<Option<String>, String> {
        self.check_path(path)?;
        let remaining = MAX_READ_BYTES.saturating_sub(self.read_bytes);
        if remaining == 0 {
            return Err("TUI config/theme aggregate read budget exceeded".into());
        }
        let limit = per_file.min(remaining.saturating_sub(1));
        let text = match crate::native_ui_preferences::read_bounded(path, limit, label) {
            Ok(text) => text,
            Err(error) => {
                // A failed bounded read may have consumed its sentinel byte.
                // Charge the worst case so retries cannot reset the allowance.
                self.read_bytes = self
                    .read_bytes
                    .saturating_add(limit.saturating_add(1).min(remaining));
                return Err(error);
            }
        };
        if let Some(text) = &text {
            self.read_bytes = self.read_bytes.saturating_add(text.len());
        }
        Ok(text)
    }
}

pub fn check_paths(paths: &[PathBuf]) -> Result<(), String> {
    let mut budget = ReadBudget::default();
    for path in paths {
        budget.check_path(path)?;
    }
    Ok(())
}

/// Identifies one animation slot so `release` is idempotent and cannot free
/// another row's slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AnimationToken(u64);

/// Bounds animated rows and repaint requests for one presentation tick.
///
/// Upstream stops a spinner timer when its node is hidden/culled; the Rust
/// wiring should release the token at the same points so culling restores
/// animation capacity. An exhausted slot budget is not an error: the row
/// falls back to the progress spinner's `animations = false` paint (`⋯`).
#[derive(Clone, Debug)]
pub struct AnimationBudget {
    limit: usize,
    active: Vec<u64>,
    next_id: u64,
    refused: bool,
    repaint_limit: usize,
    repaints: usize,
}

impl Default for AnimationBudget {
    fn default() -> Self {
        Self::new(MAX_ACTIVE_ANIMATIONS)
    }
}

impl AnimationBudget {
    #[must_use]
    pub fn new(limit: usize) -> Self {
        Self::with_repaint_limit(limit, MAX_REPAINTS_PER_TICK)
    }

    #[must_use]
    pub fn with_repaint_limit(limit: usize, repaint_limit: usize) -> Self {
        Self {
            limit,
            active: Vec::new(),
            next_id: 0,
            refused: false,
            repaint_limit,
            repaints: 0,
        }
    }

    /// Reserve one animation slot. `None` means the row must paint the static
    /// fallback; the refusal is recorded for diagnostics.
    pub fn acquire(&mut self) -> Option<AnimationToken> {
        if self.active.len() >= self.limit {
            self.refused = true;
            return None;
        }
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        self.active.push(id);
        Some(AnimationToken(id))
    }

    /// Release a slot. Returns false for an unknown or already-released token.
    pub fn release(&mut self, token: AnimationToken) -> bool {
        match self.active.iter().position(|id| *id == token.0) {
            Some(index) => {
                self.active.remove(index);
                true
            }
            None => false,
        }
    }

    #[must_use]
    pub fn active(&self) -> usize {
        self.active.len()
    }

    #[must_use]
    pub fn limit(&self) -> usize {
        self.limit
    }

    /// True when no further animations can start right now.
    #[must_use]
    pub fn is_exhausted(&self) -> bool {
        self.active.len() >= self.limit
    }

    /// Whether any acquire/repaint request was refused since construction.
    #[must_use]
    pub fn had_refusal(&self) -> bool {
        self.refused
    }

    /// Start a presentation tick; the repaint allowance is per tick.
    pub fn begin_tick(&mut self) {
        self.repaints = 0;
    }

    /// Request one coalesced repaint. False means the request joins an
    /// already-scheduled paint for this tick rather than adding another.
    pub fn request_repaint(&mut self) -> bool {
        if self.repaints >= self.repaint_limit {
            self.refused = true;
            return false;
        }
        self.repaints += 1;
        true
    }

    #[must_use]
    pub fn repaints_this_tick(&self) -> usize {
        self.repaints
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn animation_slots_exhaustion_and_release_restore_capacity() {
        let mut budget = AnimationBudget::new(2);
        assert!(!budget.is_exhausted());
        let first = budget.acquire().expect("first slot");
        let second = budget.acquire().expect("second slot");
        assert_eq!(budget.active(), 2);
        assert!(budget.is_exhausted());
        assert!(budget.acquire().is_none(), "third slot is refused");
        assert!(budget.had_refusal());
        assert!(budget.release(first));
        assert_eq!(budget.active(), 1);
        assert!(!budget.is_exhausted());
        assert!(!budget.release(first), "release is idempotent");
        assert!(budget.release(second));
        assert_eq!(budget.active(), 0);
    }

    #[test]
    fn zero_animation_budget_is_immediately_exhausted() {
        let mut budget = AnimationBudget::new(0);
        assert!(budget.is_exhausted());
        assert!(budget.acquire().is_none());
        assert!(budget.had_refusal());
    }

    #[test]
    fn repaint_requests_coalesce_per_tick_then_reset() {
        let mut budget = AnimationBudget::with_repaint_limit(4, 2);
        budget.begin_tick();
        assert!(budget.request_repaint());
        assert!(budget.request_repaint());
        assert!(!budget.request_repaint(), "third request joins this paint");
        assert_eq!(budget.repaints_this_tick(), 2);
        assert!(budget.had_refusal());
        budget.begin_tick();
        assert!(budget.request_repaint(), "new tick restores allowance");
        assert_eq!(budget.repaints_this_tick(), 1);
    }

    #[test]
    fn missing_paths_still_consume_the_probe_budget() {
        let mut budget = ReadBudget::default();
        for _ in 0..MAX_PATHS {
            budget.check_path(Path::new("/missing/tui.json")).unwrap();
        }
        assert!(budget.check_path(Path::new("/one-more")).is_err());
    }

    #[test]
    fn long_paths_have_a_cumulative_limit() {
        let mut budget = ReadBudget::default();
        let path = PathBuf::from("x".repeat(MAX_PATH_BYTES / 2));
        budget.check_path(&path).unwrap();
        budget.check_path(&path).unwrap();
        assert!(budget.check_path(Path::new("x")).is_err());
    }

    #[test]
    fn cumulative_bytes_reduce_the_next_read_limit() {
        let mut budget = ReadBudget {
            read_bytes: MAX_READ_BYTES,
            ..ReadBudget::default()
        };
        // Existing nonempty source is rejected with a zero remaining budget;
        // no further filesystem probes occur after byte exhaustion.
        assert!(budget
            .read(Path::new(file!()), 1024 * 1024, "budget fixture")
            .is_err());
        assert!(budget
            .read(
                Path::new("/no-such-opencode-config-budget-fixture"),
                1024,
                "fixture"
            )
            .is_err());
    }

    #[test]
    fn individually_valid_files_cannot_exceed_aggregate_input_limit() {
        let directory = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "oc-ui-budget-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&directory).unwrap();
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _cleanup = Cleanup(directory.clone());
        let mut budget = ReadBudget::default();
        for index in 0..3 {
            let path = directory.join(format!("{index}.json"));
            std::fs::write(&path, " ".repeat(900 * 1024)).unwrap();
            let result = budget.read(&path, 1024 * 1024, "aggregate fixture");
            if index < 2 {
                assert_eq!(result.unwrap().unwrap().len(), 900 * 1024);
            } else {
                assert!(
                    result.is_err(),
                    "third individually valid file exceeds aggregate budget"
                );
            }
        }
        assert!(
            budget
                .read(&directory.join("0.json"), 1024 * 1024, "retry")
                .is_err(),
            "retries cannot restore an exhausted byte allowance"
        );
    }
}

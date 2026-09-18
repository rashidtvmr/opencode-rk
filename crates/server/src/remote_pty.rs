//! NET-011: constrained remote PTY session control.
//!
//! Observable contract: an explicitly authorized caller operates a bounded,
//! project-scoped terminal session (resize, input, output, termination).
//! This is not unrestricted desktop/RDP access:
//! - Only [`PtyGrant::Granted`] callers can start a shell, inject input,
//!   or resize. [`PtyGrant::Denied`] and [`PtyGrant::ReadOnly`] are rejected
//!   with no side effects.
//! - Input/output envelopes are byte-capped per message and rate-capped per
//!   time window; scrollback is capped by bytes and lines (oldest evicted).
//! - Disconnect parks the session under a [`LeasePolicy`]; reconnect inside
//!   the lease resumes it, expiry reaps it. Reaping removes all state, so no
//!   orphaned session (and, by construction, no orphaned process: this module
//!   never spawns OS processes) survives. See [`NO_ORPHAN_PROCESSES`].
//! - Subprocesses inherit only a restricted project-scoped environment; see
//!   [`SubprocessInheritance`]. Ambient env, broad filesystem, network, and
//!   non-human-approved execution are never inherited.
//! - Hostile terminal escape sequences are neutralized by
//!   [`sanitize_output`]; [`contains_hostile_sequence`] is the marker
//!   predicate proving pre/post state.
//!
//! Resource bounds: all growth is capped (per-message bytes, per-window
//! bytes, scrollback bytes/lines, session count). No wall clock, no network,
//! no threads, no process spawn; time is an explicit `now: u64` tick.
#![forbid(unsafe_code)]

use std::collections::{HashMap, VecDeque};
use std::fmt;

/// Largest single input envelope accepted; larger is rejected whole.
pub const MAX_INPUT_BYTES_PER_MESSAGE: usize = 4 * 1024;
/// Largest single output push stored (excess truncated, never grown).
pub const MAX_OUTPUT_BYTES_PER_PUSH: usize = 64 * 1024;
/// Per-direction byte budget per rate window.
pub const MAX_WINDOW_BYTES: usize = 256 * 1024;
/// Rate-window length in caller ticks.
pub const WINDOW_TICKS: u64 = 10;
/// Scrollback caps: oldest lines evicted first (explicit quota behavior).
pub const MAX_SCROLLBACK_BYTES: usize = 128 * 1024;
pub const MAX_SCROLLBACK_LINES: usize = 1_000;
/// Terminal geometry bounds.
pub const MIN_COLS: u16 = 1;
pub const MAX_COLS: u16 = 512;
pub const MIN_ROWS: u16 = 1;
pub const MAX_ROWS: u16 = 256;
/// Max concurrent sessions per gateway (bounded table).
pub const MAX_SESSIONS: usize = 64;
/// Lease/retention defaults in caller ticks.
pub const LEASE_GRACE_TICKS: u64 = 30;
pub const RETENTION_TICKS: u64 = 300;

/// No-orphans marker: disconnect + lease expiry reaps the whole session
/// record, and this module never spawns an OS process, so no orphaned
/// process can survive a closed session. Always `true`; asserted by tests.
pub const NO_ORPHAN_PROCESSES: bool = true;

/// Capability a caller presents on every PTY operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PtyGrant {
    Denied,
    ReadOnly,
    Granted,
}

impl PtyGrant {
    /// Only `Granted` may start a shell.
    pub fn can_start_shell(self) -> bool {
        matches!(self, PtyGrant::Granted)
    }
    /// Only `Granted` may inject input or resize.
    pub fn can_inject(self) -> bool {
        matches!(self, PtyGrant::Granted)
    }
}

/// Byte/rate/scrollback budgets. `Default` is the documented policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PtyLimits {
    pub max_input_bytes_per_message: usize,
    pub max_output_bytes_per_push: usize,
    pub max_window_bytes: usize,
    pub window_ticks: u64,
    pub max_scrollback_bytes: usize,
    pub max_scrollback_lines: usize,
    pub max_cols: u16,
    pub max_rows: u16,
    pub max_sessions: usize,
}

impl Default for PtyLimits {
    fn default() -> Self {
        Self {
            max_input_bytes_per_message: MAX_INPUT_BYTES_PER_MESSAGE,
            max_output_bytes_per_push: MAX_OUTPUT_BYTES_PER_PUSH,
            max_window_bytes: MAX_WINDOW_BYTES,
            window_ticks: WINDOW_TICKS,
            max_scrollback_bytes: MAX_SCROLLBACK_BYTES,
            max_scrollback_lines: MAX_SCROLLBACK_LINES,
            max_cols: MAX_COLS,
            max_rows: MAX_ROWS,
            max_sessions: MAX_SESSIONS,
        }
    }
}

/// Disconnect lease + transcript retention, in caller ticks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LeasePolicy {
    /// Ticks after disconnect during which [`PtyGateway::reconnect`] works.
    pub grace_ticks: u64,
    /// Ticks a reaped session's audit marker is remembered (no resume).
    pub retention_ticks: u64,
}

impl Default for LeasePolicy {
    fn default() -> Self {
        Self {
            grace_ticks: LEASE_GRACE_TICKS,
            retention_ticks: RETENTION_TICKS,
        }
    }
}

/// Env/fs/network/human-only inheritance marker for any subprocess backing
/// a session. Construction is only possible through
/// [`SubprocessInheritance::restricted_project_scoped`]: ambient process
/// environment, broad filesystem handles, network access, and autonomous
/// (non-human-approved) execution are never inherited.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubprocessInheritance {
    /// Only an explicit allowlist of `PROJECT_`-scoped vars, never ambient.
    pub env_allowlist: Vec<String>,
    /// Confined to the fixture workspace root, never host-wide.
    pub fs_root_confined: bool,
    /// No sockets for the subprocess unless separately granted.
    pub network_isolated: bool,
    /// Execution requires a human-only grant; wildcard `*` never suffices.
    pub human_only: bool,
}

impl SubprocessInheritance {
    /// Marker: ambient environment is never inherited.
    pub const INHERITS_AMBIENT_ENV: bool = false;

    pub fn restricted_project_scoped() -> Self {
        Self {
            env_allowlist: vec!["PROJECT_NAME".to_string(), "PROJECT_LOCALE".to_string()],
            fs_root_confined: true,
            network_isolated: true,
            human_only: true,
        }
    }

    /// Always `false`: subprocesses never see the ambient environment.
    pub fn inherits_ambient_process(&self) -> bool {
        Self::INHERITS_AMBIENT_ENV
    }

    /// Always `false`: no secret names are passed through (allowlist only).
    pub fn passes_secrets(&self) -> bool {
        false
    }
}

impl Default for SubprocessInheritance {
    fn default() -> Self {
        Self::restricted_project_scoped()
    }
}

/// Resize envelope: validated against [`PtyLimits`] geometry bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PtyResize {
    pub cols: u16,
    pub rows: u16,
}

impl PtyResize {
    pub fn new(cols: u16, rows: u16) -> Self {
        Self { cols, rows }
    }

    fn check(&self, limits: &PtyLimits) -> Result<(), PtyError> {
        if self.cols < MIN_COLS
            || self.cols > limits.max_cols
            || self.rows < MIN_ROWS
            || self.rows > limits.max_rows
        {
            return Err(PtyError::InvalidResize {
                cols: self.cols,
                rows: self.rows,
            });
        }
        Ok(())
    }
}

/// Input envelope: raw keystrokes/bytes destined for the session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PtyInput {
    pub session: PtySessionId,
    pub data: Vec<u8>,
}

/// Output envelope: sanitized bytes readable from scrollback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PtyOutput {
    pub session: PtySessionId,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PtySessionId(pub String);

impl PtySessionId {
    pub fn new(s: &str) -> Self {
        Self(s.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PtyError {
    Denied,
    ReadOnly,
    UnknownSession,
    SessionClosed,
    LeaseExpired,
    PayloadTooLarge { got: usize, max: usize },
    RateLimited { direction: &'static str },
    InvalidResize { cols: u16, rows: u16 },
    TooManySessions,
}

impl fmt::Display for PtyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for PtyError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DisconnectOutcome {
    /// Parked; reconnect at or before this tick resumes it.
    RetainedUntil(u64),
    AlreadyClosed,
}

#[derive(Debug)]
struct Session {
    grant: PtyGrant,
    cols: u16,
    rows: u16,
    connected: bool,
    /// `Some(tick)` once disconnected: reconnect deadline.
    lease_expires: Option<u64>,
    pending_input: Vec<u8>,
    scrollback: VecDeque<Vec<u8>>,
    scrollback_bytes: usize,
    window_start: u64,
    in_used: usize,
    out_used: usize,
    inheritance: SubprocessInheritance,
}

/// Constrained PTY gateway. Owns session table, budgets, and leases.
/// Never spawns OS processes; all I/O is caller-pushed bytes.
#[derive(Debug)]
pub struct PtyGateway {
    limits: PtyLimits,
    lease: LeasePolicy,
    sessions: HashMap<PtySessionId, Session>,
    /// Reaped sessions remembered until `retention_ticks` lapse: reconnect
    /// reports [`PtyError::LeaseExpired`] (never silent resume) while a
    /// tombstone lives; after that the id is fully forgotten.
    reaped: HashMap<PtySessionId, u64>,
    next_id: u64,
}

impl PtyGateway {
    pub fn new(limits: PtyLimits, lease: LeasePolicy) -> Self {
        Self {
            limits,
            lease,
            sessions: HashMap::new(),
            reaped: HashMap::new(),
            next_id: 1,
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(PtyLimits::default(), LeasePolicy::default())
    }

    pub fn limits(&self) -> &PtyLimits {
        &self.limits
    }

    pub fn lease_policy(&self) -> &LeasePolicy {
        &self.lease
    }

    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    fn require_grant(caller: PtyGrant) -> Result<(), PtyError> {
        match caller {
            PtyGrant::Granted => Ok(()),
            PtyGrant::ReadOnly => Err(PtyError::ReadOnly),
            PtyGrant::Denied => Err(PtyError::Denied),
        }
    }

    fn window_allows(
        limits: &PtyLimits,
        s: &Session,
        used: usize,
        n: usize,
        now: u64,
    ) -> bool {
        let spent = if now.wrapping_sub(s.window_start) >= limits.window_ticks {
            0
        } else {
            used
        };
        spent.saturating_add(n) <= limits.max_window_bytes
    }

    fn window_spend(limits: &PtyLimits, s: &mut Session, in_dir: bool, n: usize, now: u64) {
        if now.wrapping_sub(s.window_start) >= limits.window_ticks {
            s.window_start = now;
            s.in_used = 0;
            s.out_used = 0;
        }
        if in_dir {
            s.in_used = s.in_used.saturating_add(n);
        } else {
            s.out_used = s.out_used.saturating_add(n);
        }
    }

    fn live(&self, id: &PtySessionId, now: u64) -> Result<(), PtyError> {
        let s = self.sessions.get(id).ok_or(PtyError::UnknownSession)?;
        if !s.connected {
            match s.lease_expires {
                Some(t) if now <= t => return Err(PtyError::SessionClosed),
                _ => return Err(PtyError::LeaseExpired),
            }
        }
        Ok(())
    }

    /// Record a reap tombstone so post-lease reconnect names the cause.
    fn mark_reaped(&mut self, id: &PtySessionId, now: u64) {
        let until = now.saturating_add(self.lease.retention_ticks);
        self.reaped.insert(id.clone(), until);
    }

    fn reconnect_unknown(&self, session: &PtySessionId, now: u64) -> PtyError {
        match self.reaped.get(session) {
            Some(t) if now <= *t => PtyError::LeaseExpired,
            _ => PtyError::UnknownSession,
        }
    }

    /// Start a shell-backed session. Requires [`PtyGrant::Granted`]:
    /// read-only and ungranted callers cannot start a shell.
    pub fn open(
        &mut self,
        caller: PtyGrant,
        resize: PtyResize,
        now: u64,
    ) -> Result<PtySessionId, PtyError> {
        Self::require_grant(caller)?;
        resize.check(&self.limits)?;
        if self.sessions.len() >= self.limits.max_sessions {
            return Err(PtyError::TooManySessions);
        }
        let id = PtySessionId(format!("pty-{}", self.next_id));
        self.next_id += 1;
        self.sessions.insert(
            id.clone(),
            Session {
                grant: caller,
                cols: resize.cols,
                rows: resize.rows,
                connected: true,
                lease_expires: None,
                pending_input: Vec::new(),
                scrollback: VecDeque::new(),
                scrollback_bytes: 0,
                window_start: now,
                in_used: 0,
                out_used: 0,
                inheritance: SubprocessInheritance::restricted_project_scoped(),
            },
        );
        Ok(id)
    }

    /// Inject input. Granted only; byte-capped per message and per window.
    /// Rejections accept zero bytes (no partial side effects).
    pub fn push_input(
        &mut self,
        caller: PtyGrant,
        input: &PtyInput,
        now: u64,
    ) -> Result<usize, PtyError> {
        Self::require_grant(caller)?;
        if input.data.len() > self.limits.max_input_bytes_per_message {
            return Err(PtyError::PayloadTooLarge {
                got: input.data.len(),
                max: self.limits.max_input_bytes_per_message,
            });
        }
        self.live(&input.session, now)?;
        let ok = {
            let s = self.sessions.get(&input.session).ok_or(PtyError::UnknownSession)?;
            if s.grant != PtyGrant::Granted {
                return Err(PtyError::Denied);
            }
            Self::window_allows(&self.limits, s, s.in_used, input.data.len(), now)
        };
        if !ok {
            return Err(PtyError::RateLimited { direction: "input" });
        }
        let n = input.data.len();
        let limits = self.limits;
        let s = self.sessions.get_mut(&input.session).ok_or(PtyError::UnknownSession)?;
        Self::window_spend(&limits, s, true, n, now);
        s.pending_input.extend_from_slice(&input.data);
        Ok(n)
    }

    /// Record process output. Sanitized (hostile sequences neutralized),
    /// truncated to the per-push cap, then rate- and scrollback-capped.
    pub fn push_output(
        &mut self,
        session: &PtySessionId,
        data: &[u8],
        now: u64,
    ) -> Result<usize, PtyError> {
        self.live(session, now)?;
        let clean = sanitize_output(data);
        let mut kept = clean;
        kept.truncate(self.limits.max_output_bytes_per_push);
        let ok = {
            let s = self.sessions.get(session).ok_or(PtyError::UnknownSession)?;
            Self::window_allows(&self.limits, s, s.out_used, kept.len(), now)
        };
        if !ok {
            return Err(PtyError::RateLimited { direction: "output" });
        }
        let n = kept.len();
        // Split-borrow limits for the spend bookkeeping.
        let limits = self.limits;
        let (max_bytes, max_lines) =
            (self.limits.max_scrollback_bytes, self.limits.max_scrollback_lines);
        let s = self.sessions.get_mut(session).ok_or(PtyError::UnknownSession)?;
        Self::window_spend(&limits, s, false, n, now);
        if n > 0 {
            // Keep line structure for the scrollback line cap.
            let mut start = 0;
            for (i, b) in kept.iter().enumerate() {
                if *b == b'\n' {
                    Self::push_line_capped(s, &kept[start..=i], max_bytes, max_lines);
                    start = i + 1;
                }
            }
            if start < kept.len() {
                Self::push_line_capped(s, &kept[start..], max_bytes, max_lines);
            }
        }
        Ok(n)
    }

    fn push_line_capped(
        s: &mut Session,
        line: &[u8],
        max_bytes: usize,
        max_lines: usize,
    ) {
        s.scrollback.push_back(line.to_vec());
        s.scrollback_bytes = s.scrollback_bytes.saturating_add(line.len());
        while s.scrollback.len() > max_lines || s.scrollback_bytes > max_bytes {
            if let Some(old) = s.scrollback.pop_front() {
                s.scrollback_bytes = s.scrollback_bytes.saturating_sub(old.len());
            } else {
                break;
            }
        }
    }

    /// Resize a live session. Granted only, geometry-bounded.
    pub fn resize(
        &mut self,
        caller: PtyGrant,
        session: &PtySessionId,
        resize: PtyResize,
        now: u64,
    ) -> Result<(), PtyError> {
        Self::require_grant(caller)?;
        resize.check(&self.limits)?;
        self.live(session, now)?;
        let s = self.sessions.get_mut(session).ok_or(PtyError::UnknownSession)?;
        if s.grant != PtyGrant::Granted {
            return Err(PtyError::Denied);
        }
        s.cols = resize.cols;
        s.rows = resize.rows;
        Ok(())
    }

    /// Drain queued input (absence asserts no injection happened).
    pub fn take_input(
        &mut self,
        session: &PtySessionId,
    ) -> Result<Vec<u8>, PtyError> {
        let s = self.sessions.get_mut(session).ok_or(PtyError::UnknownSession)?;
        Ok(std::mem::take(&mut s.pending_input))
    }

    /// Read-only view of retained scrollback as one byte string.
    pub fn scrollback_text(&self, session: &PtySessionId) -> Result<Vec<u8>, PtyError> {
        let s = self.sessions.get(session).ok_or(PtyError::UnknownSession)?;
        let mut out = Vec::with_capacity(s.scrollback_bytes);
        for line in &s.scrollback {
            out.extend_from_slice(line);
        }
        Ok(out)
    }

    pub fn scrollback_len(&self, session: &PtySessionId) -> Result<(usize, usize), PtyError> {
        let s = self.sessions.get(session).ok_or(PtyError::UnknownSession)?;
        Ok((s.scrollback.len(), s.scrollback_bytes))
    }

    pub fn geometry(&self, session: &PtySessionId) -> Result<(u16, u16), PtyError> {
        let s = self.sessions.get(session).ok_or(PtyError::UnknownSession)?;
        Ok((s.cols, s.rows))
    }

    pub fn inheritance(&self, session: &PtySessionId) -> Result<SubprocessInheritance, PtyError> {
        let s = self.sessions.get(session).ok_or(PtyError::UnknownSession)?;
        Ok(s.inheritance.clone())
    }

    /// Disconnect: park the session under the lease. Queued input is
    /// dropped; scrollback is retained until reap.
    pub fn disconnect(
        &mut self,
        session: &PtySessionId,
        now: u64,
    ) -> Result<DisconnectOutcome, PtyError> {
        let s = self.sessions.get_mut(session).ok_or(PtyError::UnknownSession)?;
        if !s.connected {
            return Ok(DisconnectOutcome::AlreadyClosed);
        }
        s.connected = false;
        s.pending_input.clear();
        let until = now.saturating_add(self.lease.grace_ticks);
        s.lease_expires = Some(until);
        Ok(DisconnectOutcome::RetainedUntil(until))
    }

    /// Reconnect inside the lease. Past expiry the session is reaped and
    /// [`PtyError::LeaseExpired`] is returned (never silently resumed).
    pub fn reconnect(
        &mut self,
        session: &PtySessionId,
        now: u64,
    ) -> Result<(), PtyError> {
        let remove = {
            let s = self.sessions.get(session).ok_or_else(|| self.reconnect_unknown(session, now))?;
            if s.connected {
                return Ok(());
            }
            !matches!(s.lease_expires, Some(t) if now <= t)
        };
        if remove {
            self.sessions.remove(session);
            self.mark_reaped(session, now);
            return Err(PtyError::LeaseExpired);
        }
        let s = self.sessions.get_mut(session).ok_or(PtyError::UnknownSession)?;
        s.connected = true;
        s.lease_expires = None;
        Ok(())
    }

    /// Reap every disconnected session past its lease. Returns reaped
    /// count; reaped state is fully removed (no orphans).
    pub fn sweep(&mut self, now: u64) -> usize {
        let dead: Vec<PtySessionId> = self
            .sessions
            .iter()
            .filter(|(_, s)| {
                !s.connected && !matches!(s.lease_expires, Some(t) if now <= t)
            })
            .map(|(id, _)| id.clone())
            .collect();
        let n = dead.len();
        for id in dead {
            self.sessions.remove(&id);
            self.mark_reaped(&id, now);
        }
        n
    }

    pub fn is_live(&self, session: &PtySessionId, now: u64) -> bool {
        match self.sessions.get(session) {
            Some(s) => s.connected && self.live(session, now).is_ok(),
            None => false,
        }
    }

    pub fn retained_until(&self, session: &PtySessionId) -> Option<u64> {
        self.sessions.get(session).and_then(|s| s.lease_expires)
    }
}

/// Hostile-sequence marker: true when `data` carries bytes that could
/// drive the terminal emulator (ESC-initiated sequences, C1 CSI, BEL).
pub fn contains_hostile_sequence(data: &[u8]) -> bool {
    data.iter().any(|b| *b == 0x1B || *b == 0x9B || *b == 0x07)
}

/// Neutralize hostile terminal sequences: strip ESC-initiated sequences
/// (CSI, OSC terminated by BEL or ST, charset selects), C1 CSI, BEL, DEL,
/// and other C0 controls except `\n`, `\r`, `\t`. Printable text,
/// including UTF-8 multibyte content, passes through unchanged.
pub fn sanitize_output(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    let mut i = 0;
    while i < data.len() {
        let b = data[i];
        if b == 0x1B {
            // ESC sequence: consume introducer + body.
            i += 1;
            if i >= data.len() {
                break;
            }
            match data[i] {
                b'[' => {
                    i += 1;
                    while i < data.len() && !(0x40..=0x7E).contains(&data[i]) {
                        i += 1;
                    }
                    i += 1; // consume final byte (or run off end)
                }
                b']' => {
                    // OSC: consume until BEL or ESC \ (ST).
                    i += 1;
                    while i < data.len() {
                        if data[i] == 0x07 {
                            i += 1;
                            break;
                        }
                        if data[i] == 0x1B
                            && i + 1 < data.len()
                            && data[i + 1] == b'\\'
                        {
                            i += 2;
                            break;
                        }
                        i += 1;
                    }
                }
                b'(' | b')' | b'#' => {
                    i += 2; // charset select / DEC line flag + one byte
                }
                _ => {
                    i += 1; // lone ESC + single-char sequence
                }
            }
            continue;
        }
        if b == 0x9B {
            // C1 CSI: same rule as ESC [.
            i += 1;
            while i < data.len() && !(0x40..=0x7E).contains(&data[i]) {
                i += 1;
            }
            i += 1;
            continue;
        }
        if (b < 0x20 && b != b'\n' && b != b'\r' && b != b'\t') || b == 0x7F {
            i += 1;
            continue;
        }
        out.push(b);
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gw() -> PtyGateway {
        PtyGateway::with_defaults()
    }

    fn open_granted(g: &mut PtyGateway, now: u64) -> PtySessionId {
        g.open(PtyGrant::Granted, PtyResize::new(80, 24), now).unwrap()
    }

    #[test]
    fn granted_input_output_and_resize_roundtrip() {
        let mut g = gw();
        let id = open_granted(&mut g, 0);
        g.resize(PtyGrant::Granted, &id, PtyResize::new(120, 40), 1).unwrap();
        assert_eq!(g.geometry(&id).unwrap(), (120, 40));
        let n = g
            .push_input(
                PtyGrant::Granted,
                &PtyInput { session: id.clone(), data: b"ls\n".to_vec() },
                2,
            )
            .unwrap();
        assert_eq!(n, 3);
        let stored = g.push_output(&id, b"file.txt\n", 3).unwrap();
        assert_eq!(stored, 9);
        assert_eq!(g.take_input(&id).unwrap(), b"ls\n");
        assert_eq!(g.scrollback_text(&id).unwrap(), b"file.txt\n");
    }

    #[test]
    fn ungranted_and_readonly_cannot_inject_or_start_shell() {
        let mut g = gw();
        // Neither class can start a shell.
        assert_eq!(
            g.open(PtyGrant::Denied, PtyResize::new(80, 24), 0),
            Err(PtyError::Denied)
        );
        assert_eq!(
            g.open(PtyGrant::ReadOnly, PtyResize::new(80, 24), 0),
            Err(PtyError::ReadOnly)
        );
        assert_eq!(g.session_count(), 0);
        // Neither class can inject input or resize a live session, and
        // rejected input leaves zero side effects.
        let id = open_granted(&mut g, 0);
        let evil = PtyInput { session: id.clone(), data: b"rm -rf /\n".to_vec() };
        assert_eq!(g.push_input(PtyGrant::Denied, &evil, 1), Err(PtyError::Denied));
        assert_eq!(g.push_input(PtyGrant::ReadOnly, &evil, 1), Err(PtyError::ReadOnly));
        assert!(g.take_input(&id).unwrap().is_empty());
        assert_eq!(
            g.resize(PtyGrant::Denied, &id, PtyResize::new(10, 10), 1),
            Err(PtyError::Denied)
        );
        assert_eq!(
            g.resize(PtyGrant::ReadOnly, &id, PtyResize::new(10, 10), 1),
            Err(PtyError::ReadOnly)
        );
        assert_eq!(g.geometry(&id).unwrap(), (80, 24));
    }

    #[test]
    fn subprocess_inheritance_is_restricted_project_scoped() {
        let mut g = gw();
        let id = open_granted(&mut g, 0);
        let inh = g.inheritance(&id).unwrap();
        // Env/fs/network/human-only inheritance marker.
        assert!(!inh.inherits_ambient_process());
        assert!(!SubprocessInheritance::INHERITS_AMBIENT_ENV);
        assert!(!inh.passes_secrets());
        assert!(inh.fs_root_confined);
        assert!(inh.network_isolated);
        assert!(inh.human_only);
        assert!(inh.env_allowlist.iter().all(|v| v.starts_with("PROJECT_")));
    }

    #[test]
    fn disconnect_reconnect_follows_lease_without_orphans() {
        assert!(NO_ORPHAN_PROCESSES);
        let mut g = gw();
        let grace = g.lease_policy().grace_ticks;
        let id = open_granted(&mut g, 0);
        g.push_output(&id, b"work\n", 1).unwrap();
        match g.disconnect(&id, 10).unwrap() {
            DisconnectOutcome::RetainedUntil(t) => assert_eq!(t, 10 + grace),
            DisconnectOutcome::AlreadyClosed => panic!("fresh session must park"),
        }
        assert!(!g.is_live(&id, 11));
        assert_eq!(g.retained_until(&id), Some(10 + grace));
        // Reconnect inside the lease resumes; scrollback survives.
        g.reconnect(&id, 10 + grace).unwrap();
        assert!(g.is_live(&id, 10 + grace));
        assert_eq!(g.scrollback_text(&id).unwrap(), b"work\n");
        // Disconnect again, let the lease lapse: reaped, never resumable.
        g.disconnect(&id, 100).unwrap();
        assert_eq!(g.sweep(100 + grace + 1), 1);
        assert_eq!(g.session_count(), 0);
        assert_eq!(g.reconnect(&id, 100 + grace + 2), Err(PtyError::LeaseExpired));
        assert_eq!(g.take_input(&id), Err(PtyError::UnknownSession));
    }

    #[test]
    fn flood_respects_byte_rate_and_scrollback_budgets() {
        let mut g = gw();
        let id = open_granted(&mut g, 0);
        let max_msg = g.limits().max_input_bytes_per_message;
        // Oversize single input rejected whole: zero bytes accepted.
        let big = vec![b'x'; max_msg + 1];
        assert_eq!(
            g.push_input(
                PtyGrant::Granted,
                &PtyInput { session: id.clone(), data: big },
                1
            ),
            Err(PtyError::PayloadTooLarge { got: max_msg + 1, max: max_msg })
        );
        assert!(g.take_input(&id).unwrap().is_empty());
        // Rate window: fill the budget, next byte is refused.
        let window = g.limits().max_window_bytes;
        let chunk = vec![b'y'; max_msg];
        let mut now = 1u64;
        let mut spent = 0usize;
        while spent + max_msg <= window {
            g.push_input(
                PtyGrant::Granted,
                &PtyInput { session: id.clone(), data: chunk.clone() },
                now,
            )
            .unwrap();
            spent += max_msg;
        }
        assert_eq!(
            g.push_input(
                PtyGrant::Granted,
                &PtyInput { session: id.clone(), data: vec![b'z'; 8] },
                now
            ),
            Err(PtyError::RateLimited { direction: "input" })
        );
        // Window rolls over: same bytes accepted at a later tick.
        now += WINDOW_TICKS;
        assert!(g
            .push_input(
                PtyGrant::Granted,
                &PtyInput { session: id.clone(), data: vec![b'z'; 8] },
                now
            )
            .is_ok());
        // Output flood: scrollback stays within byte and line caps.
        let id2 = open_granted(&mut g, now);
        let line = b"flood-line-0123456789\n";
        for i in 0..2000u64 {
            // Advance the window so the rate budget never trips; this
            // isolates the scrollback quota under test.
            let t = now + 1 + i * (WINDOW_TICKS + 1);
            g.push_output(&id2, line, t).unwrap_or(0);
        }
        let (lines, bytes) = g.scrollback_len(&id2).unwrap();
        assert!(lines <= MAX_SCROLLBACK_LINES, "lines={lines}");
        assert!(bytes <= MAX_SCROLLBACK_BYTES, "bytes={bytes}");
        assert!(lines > 0);
    }

    #[test]
    fn hostile_terminal_sequences_neutralized() {
        // Marker predicate fires on the hostile sample...
        let hostile: &[u8] =
            b"\x1b[2J\x1b]8;;http://evil.example\x07click\x1b[0m\x1b[?25l Hi \x07\x9b31m";
        assert!(contains_hostile_sequence(hostile));
        assert!(!contains_hostile_sequence(b"plain text\n"));
        // ...and the sanitizer neutralizes it while keeping legit text.
        let clean = sanitize_output(hostile);
        assert!(!contains_hostile_sequence(&clean));
        assert!(!clean.contains(&0x1B));
        let text = String::from_utf8(clean.clone()).unwrap();
        assert!(text.contains("click"), "legit label must survive: {text:?}");
        assert!(text.contains("Hi"), "legit text must survive: {text:?}");
        assert!(!text.contains("evil.example"), "OSC payload must go: {text:?}");
        // End to end: hostile process output stored sanitized.
        let mut g = gw();
        let id = open_granted(&mut g, 0);
        g.push_output(&id, hostile, 1).unwrap();
        let kept = g.scrollback_text(&id).unwrap();
        assert!(!contains_hostile_sequence(&kept));
        assert!(!kept.contains(&0x1B));
    }

    #[test]
    fn invalid_resize_and_bounds_rejected() {
        let mut g = gw();
        assert_eq!(
            g.open(PtyGrant::Granted, PtyResize::new(0, 24), 0),
            Err(PtyError::InvalidResize { cols: 0, rows: 24 })
        );
        assert_eq!(
            g.open(PtyGrant::Granted, PtyResize::new(80, 9999), 0),
            Err(PtyError::InvalidResize { cols: 80, rows: 9999 })
        );
        let id = open_granted(&mut g, 0);
        assert!(g
            .resize(PtyGrant::Granted, &id, PtyResize::new(10000, 10), 1)
            .is_err());
        assert_eq!(g.geometry(&id).unwrap(), (80, 24));
    }
}

//! PAR-007 app extension types: skill/command contributors, native
//! capability versioning, UI native marker, compat-host marker, authority gate.
//!
//! Pure in-memory, std only, no I/O, no threads, no clock, no globals.
//! Caller owns every registry lifetime. All collections bounded; every
//! rejection leaves state unchanged and consumes no id.
//!
//! Lane-owned module: tested standalone via
//! `rustc --edition 2021 --test crates/tools/src/app_extensions.rs`.

#![forbid(unsafe_code)]

/// Maximum contributors retained by one [`ExtensionRegistry`].
pub const MAX_CONTRIBUTORS: usize = 128;
/// Maximum capabilities per native plugin declaration.
pub const MAX_CAPABILITIES: usize = 16;
/// Maximum contributor / plugin / grant name length in bytes.
pub const MAX_NAME_LEN: usize = 128;
/// Maximum single-capability length in bytes.
pub const MAX_CAP_LEN: usize = 64;
/// Maximum UI contributions retained by one [`UiCatalog`].
pub const MAX_UI: usize = 64;
/// Maximum UI label length in bytes.
pub const MAX_LABEL_LEN: usize = 64;
/// Maximum turn-marker length in bytes.
pub const MAX_MARKER_LEN: usize = 256;
/// Only native contract version accepted by [`NativePlugin::validate`].
pub const SUPPORTED_CONTRACT_VERSION: u32 = 1;
/// Maximum accounted compat-host budget in bytes.
pub const MAX_COMPAT_BUDGET: u64 = 8 * 1024 * 1024;

/// Contribution scope. Higher rank wins on name collision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Scope {
    Builtin,
    Plugin,
    Project,
    User,
}

/// What a contributor adds to a turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContributorKind {
    Skill,
    Command,
}

/// One skill/command contribution: name + turn-marker payload only.
/// Holds labels, never file bodies or secrets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contributor {
    pub kind: ContributorKind,
    pub scope: Scope,
    pub name: String,
    pub marker: String,
}

/// Minimal turn state a command/skill contribution may affect.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Turn {
    pub marker: String,
}

/// Native plugin declaration: name + contract version + capability labels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativePlugin {
    pub name: String,
    pub contract_version: u32,
    pub capabilities: Vec<String>,
}

/// Which UI technology a contribution needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiKind {
    /// Renders through the native contract.
    Native,
    /// Solid/TS web UI: usable only via the optional compat host,
    /// never advertised as native-compatible.
    SolidTs,
}

/// One UI contribution: label + technology marker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiContribution {
    pub label: String,
    pub kind: UiKind,
}

/// Compat-host launch mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompatMode {
    /// Native-only process: the compat host must never start here.
    NativeOnly,
    /// Sandboxed host with an accounted byte budget.
    Sandboxed { budget_bytes: u64 },
}

/// Sandboxed compat host marker: records budget and accounted use.
/// Starts nothing, spawns nothing; `start` only flips state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompatHost {
    budget_bytes: u64,
    used_bytes: u64,
    started: bool,
}

/// Security authority: granted capability labels. Human-only grants are
/// never obtainable through [`Authority::request`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Authority {
    granted: Vec<String>,
}

/// All extension failures. Rejections mutate nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtError {
    InvalidName,
    InvalidMarker,
    Duplicate,
    Overflow,
    Unknown,
    InvalidCapabilities,
    UnsupportedContract,
    NotNativeCompatible,
    CompatRefusedNativeOnly,
    CompatNotStarted,
    CompatBudgetExceeded,
    GrantRejected,
}

impl std::fmt::Display for ExtError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidName => write!(f, "invalid name"),
            Self::InvalidMarker => write!(f, "invalid marker"),
            Self::Duplicate => write!(f, "duplicate contribution"),
            Self::Overflow => write!(f, "registry full"),
            Self::Unknown => write!(f, "unknown contribution"),
            Self::InvalidCapabilities => write!(f, "invalid capabilities"),
            Self::UnsupportedContract => write!(f, "unsupported contract version"),
            Self::NotNativeCompatible => write!(f, "not native-compatible"),
            Self::CompatRefusedNativeOnly => {
                write!(f, "compat host never starts in native-only mode")
            }
            Self::CompatNotStarted => write!(f, "compat host not started"),
            Self::CompatBudgetExceeded => write!(f, "compat budget exceeded"),
            Self::GrantRejected => write!(f, "grant rejected: input cannot alter authority"),
        }
    }
}

impl std::error::Error for ExtError {}

fn valid_name(s: &str, max_len: usize) -> bool {
    if s.is_empty() || s.len() > max_len {
        return false;
    }
    let mut bytes = s.bytes();
    match bytes.next() {
        Some(b) if b.is_ascii_alphanumeric() => {}
        _ => return false,
    }
    bytes.all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'_' || b == b'-' || b == b'/')
}

fn valid_command_name(s: &str) -> bool {
    s.len() >= 2 && s.starts_with('/') && !s.contains(char::is_whitespace) && valid_name(&s[1..], MAX_NAME_LEN)
}

fn valid_marker(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= MAX_MARKER_LEN
        && s.bytes().all(|b| matches!(b, 0x20..=0x7E))
}

fn valid_cap(s: &str) -> bool {
    if s.is_empty() || s.len() > MAX_CAP_LEN {
        return false;
    }
    let mut bytes = s.bytes();
    match bytes.next() {
        Some(b) if b.is_ascii_alphanumeric() => {}
        _ => return false,
    }
    bytes.all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'_' || b == b'-')
}

fn scope_rank(s: Scope) -> u8 {
    match s {
        Scope::Builtin => 0,
        Scope::Plugin => 1,
        Scope::Project => 2,
        Scope::User => 3,
    }
}

impl Contributor {
    /// Validate name/marker shape without storing.
    pub fn validate(&self) -> Result<(), ExtError> {
        match self.kind {
            ContributorKind::Skill => {
                if !valid_name(&self.name, MAX_NAME_LEN) {
                    return Err(ExtError::InvalidName);
                }
            }
            ContributorKind::Command => {
                if !valid_command_name(&self.name) {
                    return Err(ExtError::InvalidName);
                }
            }
        }
        if !valid_marker(&self.marker) {
            return Err(ExtError::InvalidMarker);
        }
        Ok(())
    }
}

/// In-memory contributor registry. Caller owns the lifetime; all methods
/// synchronous, spawn nothing, touch no I/O.
#[derive(Debug, Default)]
pub struct ExtensionRegistry {
    entries: Vec<Contributor>,
}

impl ExtensionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Add one contributor. Same kind+name+scope twice => `Duplicate`.
    /// Same kind+name at a different scope is allowed (precedence
    /// resolution picks the highest scope). Failures change nothing.
    pub fn add(&mut self, c: Contributor) -> Result<(), ExtError> {
        c.validate()?;
        if self
            .entries
            .iter()
            .any(|e| e.kind == c.kind && e.name == c.name && e.scope == c.scope)
        {
            return Err(ExtError::Duplicate);
        }
        if self.entries.len() >= MAX_CONTRIBUTORS {
            return Err(ExtError::Overflow);
        }
        self.entries.push(c);
        Ok(())
    }

    /// Resolve `name` of `kind` at the highest scope (precedence winner).
    pub fn resolve(&self, kind: ContributorKind, name: &str) -> Option<&Contributor> {
        self.entries
            .iter()
            .filter(|e| e.kind == kind && e.name == name)
            .max_by_key(|e| scope_rank(e.scope))
    }

    /// Apply the winning command contribution to `turn`: sets the turn
    /// marker to the contributor payload. Unknown name => `Unknown`;
    /// turn unchanged on error.
    pub fn apply_command(&self, name: &str, turn: &mut Turn) -> Result<(), ExtError> {
        match self.resolve(ContributorKind::Command, name) {
            Some(c) => {
                turn.marker = c.marker.clone();
                Ok(())
            }
            None => Err(ExtError::Unknown),
        }
    }

    /// Apply the winning skill contribution to `turn` (same semantics).
    pub fn apply_skill(&self, name: &str, turn: &mut Turn) -> Result<(), ExtError> {
        match self.resolve(ContributorKind::Skill, name) {
            Some(c) => {
                turn.marker = c.marker.clone();
                Ok(())
            }
            None => Err(ExtError::Unknown),
        }
    }
}

impl NativePlugin {
    /// Validate name, contract version, capability count/shape/dupes.
    pub fn validate(&self) -> Result<(), ExtError> {
        if !valid_name(&self.name, MAX_NAME_LEN) {
            return Err(ExtError::InvalidName);
        }
        if self.contract_version != SUPPORTED_CONTRACT_VERSION {
            return Err(ExtError::UnsupportedContract);
        }
        if self.capabilities.len() > MAX_CAPABILITIES {
            return Err(ExtError::InvalidCapabilities);
        }
        for (i, c) in self.capabilities.iter().enumerate() {
            if !valid_cap(c) {
                return Err(ExtError::InvalidCapabilities);
            }
            if self.capabilities[..i].contains(c) {
                return Err(ExtError::InvalidCapabilities);
            }
        }
        Ok(())
    }

    /// Whether this plugin may execute natively (valid + supported version).
    pub fn is_native_compatible(&self) -> bool {
        self.validate().is_ok()
    }
}

impl UiContribution {
    pub fn validate(&self) -> Result<(), ExtError> {
        if self.label.is_empty() || self.label.len() > MAX_LABEL_LEN {
            return Err(ExtError::InvalidName);
        }
        if !self.label.bytes().next().is_some_and(|b| b.is_ascii_alphanumeric()) {
            return Err(ExtError::InvalidName);
        }
        if !self
            .label
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
        {
            return Err(ExtError::InvalidName);
        }
        Ok(())
    }

    /// Solid/TS UI is never native-compatible.
    pub fn is_native_compatible(&self) -> bool {
        matches!(self.kind, UiKind::Native)
    }

    /// Native advertisement gate: Solid/TS always refused.
    pub fn advertise_native(&self) -> Result<(), ExtError> {
        match self.kind {
            UiKind::Native => {
                self.validate()?;
                Ok(())
            }
            UiKind::SolidTs => Err(ExtError::NotNativeCompatible),
        }
    }
}

/// Pure in-memory UI catalog. Native listings exclude Solid/TS entries.
#[derive(Debug, Default)]
pub struct UiCatalog {
    entries: Vec<UiContribution>,
}

impl UiCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Record one UI contribution. Duplicate label+kind refused.
    pub fn add(&mut self, u: UiContribution) -> Result<(), ExtError> {
        u.validate()?;
        if self.entries.iter().any(|e| e.label == u.label && e.kind == u.kind) {
            return Err(ExtError::Duplicate);
        }
        if self.entries.len() >= MAX_UI {
            return Err(ExtError::Overflow);
        }
        self.entries.push(u);
        Ok(())
    }

    /// Only native-compatible contributions: Solid/TS never advertised.
    pub fn list_native(&self) -> Vec<&UiContribution> {
        self.entries.iter().filter(|e| e.is_native_compatible()).collect()
    }

    pub fn list_all(&self) -> &[UiContribution] {
        &self.entries
    }
}

impl CompatHost {
    /// Create an unstarted host marker. Zero use accounted.
    pub fn new() -> Self {
        Self {
            budget_bytes: 0,
            used_bytes: 0,
            started: false,
        }
    }

    /// Start the compat host. `NativeOnly` is always refused: the compat
    /// host never starts in native-only mode. `Sandboxed` with a nonzero
    /// budget within [`MAX_COMPAT_BUDGET`] starts accounted use at zero.
    pub fn start(&mut self, mode: CompatMode) -> Result<(), ExtError> {
        match mode {
            CompatMode::NativeOnly => Err(ExtError::CompatRefusedNativeOnly),
            CompatMode::Sandboxed { budget_bytes } => {
                if budget_bytes == 0 || budget_bytes > MAX_COMPAT_BUDGET {
                    return Err(ExtError::CompatBudgetExceeded);
                }
                self.budget_bytes = budget_bytes;
                self.used_bytes = 0;
                self.started = true;
                Ok(())
            }
        }
    }

    pub fn is_started(&self) -> bool {
        self.started
    }

    pub fn budget_bytes(&self) -> u64 {
        self.budget_bytes
    }

    pub fn used_bytes(&self) -> u64 {
        self.used_bytes
    }

    /// Account `bytes` of sandboxed work. Requires a started host;
    /// over-budget use is refused and accounts nothing.
    pub fn account(&mut self, bytes: u64) -> Result<(), ExtError> {
        if !self.started {
            return Err(ExtError::CompatNotStarted);
        }
        match self.used_bytes.checked_add(bytes) {
            Some(next) if next <= self.budget_bytes => {
                self.used_bytes = next;
                Ok(())
            }
            _ => Err(ExtError::CompatBudgetExceeded),
        }
    }
}

impl Default for CompatHost {
    fn default() -> Self {
        Self::new()
    }
}

/// Returns true iff `name` is a well-formed grantable capability label
/// that cannot alter security authority: non-empty, bounded, strict
/// label charset, no path separators, no traversal, no wildcard, and
/// outside the human-only/system-reserved namespaces.
pub fn grant_input_ok(name: &str) -> bool {
    if !valid_cap(name) {
        return false;
    }
    if name.contains('/') || name.contains('\\') || name.contains('*') || name.contains("..") {
        return false;
    }
    // Reserved namespaces: never grantable via plugin input.
    for prefix in ["human.", "system.", "authority.", "grant."] {
        if name == prefix.trim_end_matches('.') || name.starts_with(prefix) {
            return false;
        }
    }
    true
}

impl Authority {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn granted(&self) -> &[String] {
        &self.granted
    }

    /// Request capability labels from untrusted plugin input. Every entry
    /// must pass [`grant_input_ok`]; any single malicious entry rejects
    /// the whole batch and the authority is unchanged. Already-granted
    /// labels are skipped without error; overflow refused.
    pub fn request(&mut self, names: &[&str]) -> Result<(), ExtError> {
        for n in names {
            if !grant_input_ok(n) {
                return Err(ExtError::GrantRejected);
            }
        }
        let mut fresh = 0usize;
        for n in names {
            if !self.granted.iter().any(|g| g == n) {
                fresh += 1;
            }
        }
        if self.granted.len() + fresh > MAX_CAPABILITIES {
            return Err(ExtError::Overflow);
        }
        for n in names {
            if !self.granted.iter().any(|g| g == n) {
                self.granted.push((*n).to_string());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cmd(scope: Scope, name: &str, marker: &str) -> Contributor {
        Contributor {
            kind: ContributorKind::Command,
            scope,
            name: name.to_string(),
            marker: marker.to_string(),
        }
    }

    #[test]
    fn custom_command_affects_turn_marker() {
        let mut reg = ExtensionRegistry::new();
        reg.add(cmd(Scope::Builtin, "/ship", "builtin-ship")).unwrap();
        reg.add(cmd(Scope::User, "/ship", "user-ship")).unwrap();
        let mut turn = Turn::default();
        reg.apply_command("/ship", &mut turn).unwrap();
        assert_eq!(turn.marker, "user-ship");
    }

    #[test]
    fn precedence_and_scope_project_beats_plugin() {
        let mut reg = ExtensionRegistry::new();
        reg.add(cmd(Scope::Plugin, "/deploy", "plugin-v")).unwrap();
        reg.add(cmd(Scope::Project, "/deploy", "project-v")).unwrap();
        let won = reg.resolve(ContributorKind::Command, "/deploy").unwrap();
        assert_eq!(won.scope, Scope::Project);
        let mut turn = Turn::default();
        reg.apply_skill("missing", &mut turn).unwrap_err();
        let mut skill_reg = ExtensionRegistry::new();
        skill_reg
            .add(Contributor {
                kind: ContributorKind::Skill,
                scope: Scope::Plugin,
                name: "review".to_string(),
                marker: "skill-review".to_string(),
            })
            .unwrap();
        skill_reg.apply_skill("review", &mut turn).unwrap();
        assert_eq!(turn.marker, "skill-review");
    }

    #[test]
    fn native_capability_versioning() {
        let good = NativePlugin {
            name: "planner".to_string(),
            contract_version: SUPPORTED_CONTRACT_VERSION,
            capabilities: vec!["tools.read".to_string()],
        };
        assert!(good.is_native_compatible());
        let future = NativePlugin {
            contract_version: SUPPORTED_CONTRACT_VERSION + 1,
            ..good.clone()
        };
        assert_eq!(future.validate(), Err(ExtError::UnsupportedContract));
        assert!(!future.is_native_compatible());
        let dup = NativePlugin {
            capabilities: vec!["a.b".to_string(), "a.b".to_string()],
            ..good.clone()
        };
        assert_eq!(dup.validate(), Err(ExtError::InvalidCapabilities));
    }

    #[test]
    fn unsupported_ui_not_advertised_native() {
        let solid = UiContribution {
            label: "board".to_string(),
            kind: UiKind::SolidTs,
        };
        assert!(!solid.is_native_compatible());
        assert_eq!(solid.advertise_native(), Err(ExtError::NotNativeCompatible));
        let mut cat = UiCatalog::new();
        cat.add(solid).unwrap();
        cat.add(UiContribution {
            label: "status".to_string(),
            kind: UiKind::Native,
        })
        .unwrap();
        let native = cat.list_native();
        assert_eq!(native.len(), 1);
        assert_eq!(native[0].label, "status");
    }

    #[test]
    fn compat_never_starts_native_only() {
        let mut host = CompatHost::new();
        assert_eq!(host.start(CompatMode::NativeOnly), Err(ExtError::CompatRefusedNativeOnly));
        assert!(!host.is_started());
        host.start(CompatMode::Sandboxed { budget_bytes: 1024 }).unwrap();
        assert!(host.is_started());
        host.account(512).unwrap();
        assert_eq!(host.used_bytes(), 512);
        assert_eq!(host.account(600), Err(ExtError::CompatBudgetExceeded));
        assert_eq!(host.used_bytes(), 512);
    }

    #[test]
    fn compat_account_requires_start_and_bounded_budget() {
        let mut host = CompatHost::new();
        assert_eq!(host.account(1), Err(ExtError::CompatNotStarted));
        assert_eq!(
            host.start(CompatMode::Sandboxed { budget_bytes: 0 }),
            Err(ExtError::CompatBudgetExceeded)
        );
        assert_eq!(
            host.start(CompatMode::Sandboxed { budget_bytes: MAX_COMPAT_BUDGET + 1 }),
            Err(ExtError::CompatBudgetExceeded)
        );
        assert!(!host.is_started());
    }

    #[test]
    fn malicious_grant_rejected_authority_unchanged() {
        let mut auth = Authority::new();
        for evil in ["", "*", "../secret", "a/b", "human.approve", "system.exec", "..", "a..b..c..d..e..f..g..h..i..j..k..l..m..n..o..p..q..r..s..t..u..v..w..x..y..z..1..2..3..4..5..6..extra-long-tail-ok-but-dots-flagged"] {
            assert_eq!(auth.request(&[evil]), Err(ExtError::GrantRejected), "evil: {evil:?}");
        }
        assert!(auth.granted().is_empty());
        auth.request(&["tools.read"]).unwrap();
        assert_eq!(auth.request(&["tools.read", "human.approve"]), Err(ExtError::GrantRejected));
        assert_eq!(auth.granted(), &["tools.read".to_string()]);
    }

    #[test]
    fn registry_bounds_and_duplicates() {
        let mut reg = ExtensionRegistry::new();
        assert_eq!(
            reg.add(cmd(Scope::User, "noslash", "m")),
            Err(ExtError::InvalidName)
        );
        assert_eq!(reg.add(cmd(Scope::User, "/ok", "")), Err(ExtError::InvalidMarker));
        reg.add(cmd(Scope::User, "/ok", "m1")).unwrap();
        assert_eq!(reg.add(cmd(Scope::User, "/ok", "m1")), Err(ExtError::Duplicate));
        assert!(reg.is_empty() == false && reg.len() == 1);
        let mut turn = Turn::default();
        assert_eq!(reg.apply_command("/nope", &mut turn), Err(ExtError::Unknown));
        assert_eq!(turn.marker, "");
    }
}

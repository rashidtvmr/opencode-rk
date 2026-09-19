#![forbid(unsafe_code)]

//! Installed oc2 command inventory and binary identity (DISC-102/APP-010).
//!
//! Pure, std-only inventory boundary over the CLI surface declared in
//! `crates/cli/src/main.rs` (`Command`: Doctor/Session/Models/Serve/Web/Tui/Run;
//! no subcommand opens the chat path). No I/O, no env, no threads; the caller
//! owns process install, checksum verification, and help rendering.
//!
//! Contract:
//! - [`BINARY_NAME`] is the installed artifact identity (`oc2`).
//! - [`command_inventory`] is the exact shippable surface; unknown names are
//!   rejected by [`unknown_subcommand_exit`] with nonzero exit and no daemon.
//! - [`release_artifact_ok`] fail-closes on unknown platform/checksum state.
//! - Bounded: command tables are fixed-size; inputs length-capped.

/// Installed binary identity. Packaged output must never say `opencode-rk`.
pub const BINARY_NAME: &str = "oc2";
/// Legacy development binary name; must not appear in packaged output.
pub const LEGACY_BINARY_NAME: &str = "opencode-rk";
/// Maximum accepted command-name length, in bytes.
pub const MAX_COMMAND_NAME_LEN: usize = 64;
/// Exit code for unknown subcommands (actionable help, nonzero, no daemon).
pub const UNKNOWN_COMMAND_EXIT: i32 = 64;

/// Shippable top-level command surface (mirrors `main.rs:Command`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstalledCommand {
    Doctor,
    Session,
    Models,
    Serve,
    Web,
    Tui,
    Run,
}

impl InstalledCommand {
    /// Canonical command name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            InstalledCommand::Doctor => "doctor",
            InstalledCommand::Session => "session",
            InstalledCommand::Models => "models",
            InstalledCommand::Serve => "serve",
            InstalledCommand::Web => "web",
            InstalledCommand::Tui => "tui",
            InstalledCommand::Run => "run",
        }
    }
}

/// Exact installed command inventory. No-subcommand (chat) is entry, not list.
#[must_use]
pub const fn command_inventory() -> [InstalledCommand; 7] {
    [
        InstalledCommand::Doctor,
        InstalledCommand::Session,
        InstalledCommand::Models,
        InstalledCommand::Serve,
        InstalledCommand::Web,
        InstalledCommand::Tui,
        InstalledCommand::Run,
    ]
}

/// Look up a command by name. Empty/over-long/unknown names return `None`.
#[must_use]
pub fn lookup_command(name: &str) -> Option<InstalledCommand> {
    if name.is_empty() || name.len() > MAX_COMMAND_NAME_LEN {
        return None;
    }
    for cmd in command_inventory() {
        if cmd.name() == name {
            return Some(cmd);
        }
    }
    None
}

/// No-subcommand entry: discover/start daemon and open native TUI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoSubcommandEntry {
    OpenNativeTui,
}

/// Resolve bare invocation (no subcommand) to the real application entry.
#[must_use]
pub const fn no_subcommand_entry() -> NoSubcommandEntry {
    NoSubcommandEntry::OpenNativeTui
}

/// Unknown-subcommand outcome: actionable help text + nonzero exit, no daemon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownCommandHelp {
    pub name: String,
    pub exit: i32,
}

/// Build the unknown-subcommand help outcome. Never starts a daemon.
#[must_use]
pub fn unknown_subcommand_exit(name: &str) -> Option<UnknownCommandHelp> {
    if name.is_empty() || name.len() > MAX_COMMAND_NAME_LEN {
        return None;
    }
    if lookup_command(name).is_some() {
        return None;
    }
    let mut valid: Vec<&str> = Vec::new();
    for cmd in command_inventory() {
        valid.push(cmd.name());
    }
    Some(UnknownCommandHelp {
        name: format!("unknown subcommand '{name}'; valid: {}", valid.join(", ")),
        exit: UNKNOWN_COMMAND_EXIT,
    })
}

/// Supported install platforms for release artifacts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallPlatform {
    LinuxX64,
    LinuxArm64,
    MacosX64,
    MacosArm64,
    WindowsX64,
}

impl InstallPlatform {
    /// Canonical platform triple used by install scripts.
    #[must_use]
    pub const fn triple(self) -> &'static str {
        match self {
            InstallPlatform::LinuxX64 => "linux-x64",
            InstallPlatform::LinuxArm64 => "linux-arm64",
            InstallPlatform::MacosX64 => "macos-x64",
            InstallPlatform::MacosArm64 => "macos-arm64",
            InstallPlatform::WindowsX64 => "windows-x64",
        }
    }
}

/// All platforms a release must ship install scripts + checksums for.
#[must_use]
pub const fn install_platforms() -> [InstallPlatform; 5] {
    [
        InstallPlatform::LinuxX64,
        InstallPlatform::LinuxArm64,
        InstallPlatform::MacosX64,
        InstallPlatform::MacosArm64,
        InstallPlatform::WindowsX64,
    ]
}

/// Release-artifact gate: fail closed on unknown platform or bad checksum.
#[must_use]
pub fn release_artifact_ok(platform: Option<InstallPlatform>, checksum_ok: bool) -> bool {
    platform.is_some() && checksum_ok
}

/// Packaged-output identity check: artifact text must name oc2 and
/// never contain the legacy development binary name.
#[must_use]
pub fn packaged_output_names_oc2(text: &str) -> bool {
    text.contains(BINARY_NAME) && !text.contains(LEGACY_BINARY_NAME)
}

// ---------------------------------------------------------------------------
// Tests (frozen RED: must compile, must fail for the missing behavior)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disc102_t01_no_subcommand_opens_native_tui() {
        assert_eq!(no_subcommand_entry(), NoSubcommandEntry::OpenNativeTui);
    }

    #[test]
    fn disc102_t02_inventory_matches_upstream_surface() {
        let names: Vec<&str> = command_inventory().iter().map(|c| c.name()).collect();
        assert_eq!(
            names,
            vec!["doctor", "session", "models", "serve", "web", "tui", "run"]
        );
        assert_eq!(lookup_command("serve"), Some(InstalledCommand::Serve));
        assert_eq!(lookup_command("bogus"), None);
    }

    #[test]
    fn disc102_t03_artifacts_identify_as_oc2() {
        assert_eq!(BINARY_NAME, "oc2");
        assert!(packaged_output_names_oc2("oc2 v1.0 linux-x64"));
        assert!(!packaged_output_names_oc2("opencode-rk v1.0 linux-x64"));
    }

    #[test]
    fn disc102_t04_checksums_fail_closed_per_platform() {
        assert_eq!(install_platforms().len(), 5);
        assert!(release_artifact_ok(Some(InstallPlatform::LinuxX64), true));
        assert!(!release_artifact_ok(None, true));
        assert!(!release_artifact_ok(Some(InstallPlatform::LinuxX64), false));
        assert!(!release_artifact_ok(None, false));
    }

    #[test]
    fn disc102_t05_unknown_subcommand_help_nonzero_no_daemon() {
        let help = unknown_subcommand_exit("frobnicate").expect("help expected");
        assert_eq!(help.exit, UNKNOWN_COMMAND_EXIT);
        assert!(help.exit != 0);
        assert!(help.name.contains("frobnicate"));
        assert!(help.name.contains("serve"));
        assert!(unknown_subcommand_exit("serve").is_none());
    }
}

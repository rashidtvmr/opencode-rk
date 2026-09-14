//! Manual-only destructive instructions (SEC-013). Agent presents cmd to user, never executes directly.
use std::fmt;
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManualInstruction {
    pub command: String,
    pub warning: String,
}
impl ManualInstruction {
    #[must_use]
    pub fn new(command: impl Into<String>, warning: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            warning: warning.into(),
        }
    }
}
impl fmt::Display for ManualInstruction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "manual-only: {}\nwarning: {}",
            self.command, self.warning
        )
    }
}
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ManualOnlyPolicy {}
impl ManualOnlyPolicy {
    #[must_use]
    pub fn new() -> Self {
        Self {}
    }
    #[must_use]
    pub fn is_manual_only(&self, program: &str, args: &[String]) -> bool {
        is_manual_only(program, args)
    }
    #[must_use]
    pub fn present_to_user(&self, program: &str, args: &[String]) -> ManualInstruction {
        present_to_user(&join_argv(program, args))
    }
}
#[must_use]
pub fn is_manual_only(program: &str, args: &[String]) -> bool {
    let exe = basename(program);
    match exe.as_str() {
        "rm" => is_manual_rm(args),
        "mkfs" | "mkfs.ext4" | "mkfs.xfs" | "mkfs.vfat" | "mkfs.btrfs" | "format" | "fdisk"
        | "sfdisk" | "parted" | "iptables" | "ip6tables" | "reboot" | "shutdown" | "halt"
        | "poweroff" | "useradd" | "userdel" | "adduser" | "deluser" | "passwd" | "chpasswd" => {
            true
        }
        "systemctl" => args.iter().any(|a| {
            let l = a.to_ascii_lowercase();
            l == "stop" || l == "disable" || l == "halt" || l == "poweroff" || l == "reboot"
        }),
        _ => false,
    }
}
#[must_use]
pub fn present_to_user(cmd: &str) -> ManualInstruction {
    ManualInstruction::new(cmd.trim(),format!("Manual-only command; run it yourself, the agent never executes it directly: {cmd}. Verify targets and backups first."))
}
fn is_manual_rm(args: &[String]) -> bool {
    let rec = args.iter().any(|a| {
        a == "-r"
            || a == "-R"
            || a == "--recursive"
            || a == "-rf"
            || a == "-fr"
            || a == "-rm"
            || (a.starts_with('-') && !a.starts_with("--") && a[1..].contains('r'))
    });
    if !rec {
        return false;
    }
    args.iter()
        .filter(|a| !a.starts_with('-'))
        .any(|t| is_system_target(t))
}
fn is_system_target(t: &str) -> bool {
    let n = t.replace('\\', "/").to_ascii_lowercase();
    let t = n.as_str();
    t == "/"
        || t == "/*"
        || t == "//"
        || t == "."
        || t == ".."
        || t == "./*"
        || t == "*"
        || t.starts_with("/etc")
        || t.starts_with("/usr")
        || t.starts_with("/bin")
        || t.starts_with("/sbin")
        || t.starts_with("/boot")
        || t.starts_with("/dev")
        || t.starts_with("/proc")
        || t.starts_with("/sys")
        || t.starts_with("/system")
        || t.starts_with("c:/windows")
        || t.starts_with("c:/program files")
}
fn basename(p: &str) -> String {
    p.replace('\\', "/")
        .rsplit('/')
        .next()
        .unwrap_or(p)
        .trim_end_matches(".exe")
        .to_ascii_lowercase()
}
fn join_argv(program: &str, args: &[String]) -> String {
    let mut parts = vec![program.to_owned()];
    parts.extend(args.iter().cloned());
    parts
        .iter()
        .map(|p| {
            if p.is_empty()
                || p.chars()
                    .any(|c| c.is_whitespace() || c == '"' || c == '\'')
            {
                format!("'{0}'", p.replace('\'', "_'"))
            } else {
                p.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
#[cfg(test)]
mod tests {
    use super::*;
    fn v(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }
    #[test]
    fn rm_rf_root_manual() {
        assert!(is_manual_only("rm", &v(&["-rf", "/"])));
        assert!(is_manual_only("/bin/rm", &v(&["-rf", "/etc/passwd"])));
        assert!(ManualOnlyPolicy::new().is_manual_only("rm", &v(&["-r", "/usr/bin"])));
    }
    #[test]
    fn mkfs_manual() {
        assert!(is_manual_only("mkfs", &v(&["/dev/sda1"])));
        assert!(is_manual_only("mkfs.ext4", &v(&["/dev/sda1"])));
        assert!(is_manual_only("format", &v(&["D:"])));
        assert!(is_manual_only("fdisk", &v(&["/dev/sda"])));
        assert!(is_manual_only("parted", &v(&["/dev/sda", "print"])));
        assert!(is_manual_only("iptables", &v(&["-F"])));
        assert!(is_manual_only("reboot", &v(&[])));
        assert!(is_manual_only("shutdown", &v(&["-h", "now"])));
        assert!(is_manual_only("useradd", &v(&["bob"])));
        assert!(is_manual_only("userdel", &v(&["bob"])));
        assert!(is_manual_only("passwd", &v(&["bob"])));
    }
    #[test]
    fn safe_ls_not_manual() {
        assert!(!is_manual_only("ls", &v(&["-la"])));
        assert!(!is_manual_only("git", &v(&["status"])));
        assert!(!is_manual_only("cargo", &v(&["test"])));
        assert!(!is_manual_only("rm", &v(&["file.txt"])));
        assert!(!is_manual_only("systemctl", &v(&["status", "nginx"])));
    }
    #[test]
    fn systemctl_stop_manual() {
        assert!(is_manual_only("systemctl", &v(&["stop", "nginx"])));
        assert!(is_manual_only("systemctl", &v(&["disable", "nginx"])));
        assert!(is_manual_only("/bin/systemctl", &v(&["stop", "nginx"])));
        assert!(!is_manual_only("systemctl", &v(&["start", "nginx"])));
    }
    #[test]
    fn present_generates_instruction() {
        let i = present_to_user("mkfs /dev/sda1");
        assert_eq!(i.command, "mkfs /dev/sda1");
        assert!(!i.warning.is_empty());
        let p = ManualOnlyPolicy::new().present_to_user("systemctl", &v(&["stop", "nginx"]));
        assert!(p.command.contains("systemctl"));
        assert!(p.command.contains("stop"));
        assert!(!p.warning.is_empty());
        assert!(format!("{i}").contains("mkfs"));
    }
}

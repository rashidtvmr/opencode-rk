#![forbid(unsafe_code)]
//! Ctx kind registry: names for 22 upstream context files (registry only).

/// Number of registered context kinds.
pub const KIND_COUNT: usize = 22;
/// Context kind (one per upstream `packages/tui/src/context/` file).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CtxKind {
    Args,
    Clipboard,
    Data,
    Directory,
    Editor,
    Epilogue,
    Event,
    Exit,
    Helper,
    Kv,
    Local,
    Location,
    Pathfmt,
    Perm,
    Project,
    Promptref,
    Route,
    Runtime,
    Sdk,
    Sync,
    Theme,
    Thinking,
}

impl CtxKind {
    /// Lowercase registry name.
    #[must_use]
    pub const fn kind_name(self) -> &'static str {
        match self {
            Self::Args => "args",
            Self::Clipboard => "clipboard",
            Self::Data => "data",
            Self::Directory => "directory",
            Self::Editor => "editor",
            Self::Epilogue => "epilogue",
            Self::Event => "event",
            Self::Exit => "exit",
            Self::Helper => "helper",
            Self::Kv => "kv",
            Self::Local => "local",
            Self::Location => "location",
            Self::Pathfmt => "pathfmt",
            Self::Perm => "perm",
            Self::Project => "project",
            Self::Promptref => "promptref",
            Self::Route => "route",
            Self::Runtime => "runtime",
            Self::Sdk => "sdk",
            Self::Sync => "sync",
            Self::Theme => "theme",
            Self::Thinking => "thinking",
        }
    }
}

/// All kinds in registry order.
#[must_use]
pub const fn all_kinds() -> [CtxKind; KIND_COUNT] {
    use CtxKind::*;
    [
        Args, Clipboard, Data, Directory, Editor, Epilogue, Event, Exit, Helper, Kv, Local,
        Location, Pathfmt, Perm, Project, Promptref, Route, Runtime, Sdk, Sync, Theme, Thinking,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn count_matches() {
        assert_eq!(all_kinds().len(), KIND_COUNT);
        assert_eq!(KIND_COUNT, 22);
    }
    #[test]
    fn spot_unique() {
        assert_eq!(CtxKind::Args.kind_name(), "args");
        assert_eq!(CtxKind::Theme.kind_name(), "theme");
        assert_eq!(CtxKind::Thinking.kind_name(), "thinking");
        let k = all_kinds();
        for i in 0..k.len() {
            for j in (i + 1)..k.len() {
                assert_ne!(k[i].kind_name(), k[j].kind_name());
            }
        }
    }
    #[test]
    fn copy_eq() {
        let a = CtxKind::Kv;
        let b = a;
        assert_eq!((a, format!("{a:?}")), (b, String::from("Kv")));
    }
}

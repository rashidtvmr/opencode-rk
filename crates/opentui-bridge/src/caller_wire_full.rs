#![forbid(unsafe_code)]

//! Caller wire table: live symbols a CLI can invoke (FIX-04).
//!
//! ponytail: static table only; dynamic dispatch when CLI wired.

/// One callable symbol in a bridge module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WireCall {
    pub module: &'static str,
    pub symbol: &'static str,
}

/// Live (module, symbol) pairs reachable from CLI.
pub const WIRE_CALLS: &[(&str, &str)] = &[
    // CLI: render --dirty -> RenderLoop::request
    ("render_loop_full", "RenderLoop::request"),
    // CLI: page show <name> -> PageRouter::show
    ("page_router", "PageRouter::show"),
    // CLI: session open <id> -> SessionRoute::open
    ("route_session", "SessionRoute::open"),
    // CLI: footer append <commit> -> RunFooter::append
    ("run_footer", "RunFooter::append"),
    // CLI: stream push <chunk> -> StreamBuf::push
    ("run_stream", "StreamBuf::push"),
    // CLI: scrollback push <kind> <body> -> ScrollbackWriter::push
    ("scrollback_family", "ScrollbackWriter::push"),
    // CLI: sdk push <event> -> SdkStream::push
    ("sdk_stream", "SdkStream::push"),
    // CLI: theme apply <name> -> ThemeEngine::apply
    ("theme_engine", "ThemeEngine::apply"),
    // CLI: plugin register <name> -> PluginRuntime::register
    ("plugin_runtime", "PluginRuntime::register"),
];

impl WireCall {
    /// Const constructor for table use.
    #[must_use]
    pub const fn new(module: &'static str, symbol: &'static str) -> Self {
        Self { module, symbol }
    }
}

/// Number of wired calls.
#[must_use]
pub const fn wire_count() -> usize {
    WIRE_CALLS.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn count_is_nine() {
        assert_eq!(wire_count(), 9);
        assert_eq!(WIRE_CALLS.len(), 9);
    }

    #[test]
    fn entries_are_nonempty_qualified() {
        for (m, s) in WIRE_CALLS {
            assert!(!m.is_empty());
            assert!(s.contains("::"), "{s}");
        }
    }

    #[test]
    fn spot_check_ends() {
        assert_eq!(WIRE_CALLS[0], ("render_loop_full", "RenderLoop::request"));
        assert_eq!(WIRE_CALLS[8], ("plugin_runtime", "PluginRuntime::register"));
        let w = WireCall::new(WIRE_CALLS[2].0, WIRE_CALLS[2].1);
        assert_eq!(w.module, "route_session");
    }
}

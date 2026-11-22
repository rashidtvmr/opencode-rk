//! macOS `/var`/`/private/var` alias coverage for the real UI preference store.
//!
//! Keep the production modules in the test crate's root.  In particular,
//! `native_ui_preferences` resolves its preference and navigation types through
//! the real application modules; these declarations are intentionally not test
//! doubles.

// This is an integration crate, rather than `src/main.rs`, so the production
// preference module does not automatically inherit the application's sibling
// module graph.  Keep the graph real: these are the same source modules used
// by the native binary, not test doubles or reduced enum copies.  The
// preference tests exercise the module's `cfg(test)` coverage too, which is
// why the full native dependency closure is declared here.
//
// The list below is exactly the binary's non-test module set (`src/main.rs`),
// minus the `#[cfg(all(test, ...))]` support modules (`security_preferences`,
// `upstream_view_parity`, `native_context_tests`, `native_context_security_tests`,
// `native_compaction_settings_tests`, `native_compaction_settings_provenance_tests`)
// that only the binary's own test build needs and that nothing in this closure
// references.  `native_transcript` is declared at the crate root because
// `tui_entry` resolves `crate::native_transcript` directly; `tui_entry` also
// loads the same file through its own `#[path]` include, which is the
// pre-existing `duplicate_mod` warning inherited from `main.rs` (its fix lives
// in `tui_entry.rs`, outside this grant).
#[path = "../src/ci_output.rs"]
mod ci_output;
#[path = "../src/ci_run.rs"]
mod ci_run;
#[path = "../src/composer.rs"]
mod composer;
#[path = "../src/tui_entry.rs"]
mod tui_entry;
#[path = "../src/app_start.rs"]
mod app_start;
#[path = "../src/chat.rs"]
mod chat;
#[path = "../src/daemon_client.rs"]
mod daemon_client;
#[path = "../src/diagnostics.rs"]
mod diagnostics;
#[path = "../src/graph.rs"]
mod graph;
#[path = "../src/headless_engine.rs"]
mod headless_engine;
#[path = "../src/install_commands.rs"]
mod install_commands;
#[path = "../src/medown.rs"]
mod medown;
#[path = "../src/modals.rs"]
mod modals;

#[path = "../src/native_app.rs"]
mod native_app;
#[path = "../src/native_appearance.rs"]
mod native_appearance;
#[path = "../src/native_approvals.rs"]
mod native_approvals;
#[path = "../src/native_auth_login.rs"]
mod native_auth_login;
#[path = "../src/native_box_style.rs"]
mod native_box_style;
#[path = "../src/native_chat_history.rs"]
mod native_chat_history;
#[path = "../src/native_chat_projection.rs"]
mod native_chat_projection;
#[path = "../src/native_command_palette.rs"]
mod native_command_palette;
#[path = "../src/native_settings.rs"]
mod native_settings;
#[path = "../src/native_children.rs"]
mod native_children;
#[path = "../src/native_composer.rs"]
mod native_composer;
#[path = "../src/native_completion.rs"]
mod native_completion;
#[path = "../src/native_workspace.rs"]
mod native_workspace;
#[path = "../src/native_debug.rs"]
mod native_debug;
#[path = "../src/native_clock.rs"]
mod native_clock;
#[path = "../src/native_capture.rs"]
mod native_capture;
#[path = "../src/native_memory.rs"]
mod native_memory;
#[path = "../src/native_compaction_settings.rs"]
mod native_compaction_settings;
#[path = "../src/native_context.rs"]
mod native_context;
#[path = "../src/native_context_grid.rs"]
mod native_context_grid;
#[path = "../src/native_layout.rs"]
mod native_layout;
#[path = "../src/native_navigation.rs"]
mod native_navigation;
#[path = "../src/native_palette.rs"]
mod native_palette;
#[path = "../src/native_shell.rs"]
mod native_shell;
#[path = "../src/native_status.rs"]
mod native_status;
#[path = "../src/native_tab_context_menu.rs"]
mod native_tab_context_menu;
#[path = "../src/native_tabs.rs"]
mod native_tabs;
#[path = "../src/native_tab_rail.rs"]
mod native_tab_rail;
#[path = "../src/native_tab_resize.rs"]
mod native_tab_resize;
#[path = "../src/native_text_selection.rs"]
mod native_text_selection;
#[path = "../src/native_theme.rs"]
mod native_theme;
#[path = "../src/native_theme_assets.rs"]
mod native_theme_assets;
#[path = "../src/native_theme_runtime.rs"]
mod native_theme_runtime;
#[path = "../src/markdown_runs.rs"]
mod markdown_runs;
#[path = "../src/native_timeline.rs"]
mod native_timeline;
#[path = "../src/native_transcript.rs"]
mod native_transcript;
#[path = "../src/native_diff_view.rs"]
mod native_diff_view;
#[path = "../src/native_ui_budget.rs"]
mod native_ui_budget;
#[path = "../src/native_ui_preferences.rs"]
mod native_ui_preferences;
#[path = "../src/native_ui_style.rs"]
mod native_ui_style;
#[path = "../src/native_toast.rs"]
mod native_toast;
#[path = "../src/native_overlay_stack.rs"]
mod native_overlay_stack;
#[path = "../src/native_progress.rs"]
mod native_progress;
#[path = "../src/native_sidebar.rs"]
mod native_sidebar;
#[path = "../src/native_status_rail.rs"]
mod native_status_rail;
#[path = "../src/native_paste.rs"]
mod native_paste;
#[path = "../src/native_message_rows.rs"]
mod native_message_rows;
#[path = "../src/native_prompt_pane.rs"]
mod native_prompt_pane;
#[path = "../src/native_markdown_flow.rs"]
mod native_markdown_flow;
#[path = "../src/native_dock.rs"]
mod native_dock;
#[path = "../src/native_diff_inline.rs"]
mod native_diff_inline;
#[path = "../src/native_home.rs"]
mod native_home;
#[path = "../src/native_pane_resize.rs"]
mod native_pane_resize;
#[path = "../src/native_modal_layer.rs"]
mod native_modal_layer;
#[path = "../src/native_mermaid.rs"]
mod native_mermaid;
#[path = "../src/native_math.rs"]
mod native_math;
#[path = "../src/native_math_engine.rs"]
mod native_math_engine;
#[path = "../src/native_image_preview.rs"]
mod native_image_preview;
#[path = "../src/native_graph.rs"]
mod native_graph;
#[path = "../src/native_shell_frame.rs"]
mod native_shell_frame;
#[path = "../src/native_host.rs"]
mod native_host;
#[path = "../src/native_config_import.rs"]
mod native_config_import;
#[path = "../src/native_import_dialog.rs"]
mod native_import_dialog;
#[path = "../src/native_import_sources.rs"]
mod native_import_sources;
#[path = "../src/native_permissions.rs"]
mod native_permissions;
#[path = "../src/native_view.rs"]
mod native_view;
#[path = "../src/native_assistant_facts.rs"]
mod native_assistant_facts;
#[path = "../src/native_footer.rs"]
mod native_footer;
#[path = "../src/native_view_chat.rs"]
mod native_view_chat;
#[path = "../src/native_prompt_layout.rs"]
mod native_prompt_layout;
#[path = "../src/native_home_logo.rs"]
mod native_home_logo;
#[path = "../src/native_view_dialogs.rs"]
mod native_view_dialogs;
#[path = "../src/native_view_layout.rs"]
mod native_view_layout;
#[path = "../src/session_list.rs"]
mod session_list;
#[path = "../src/native_view_pages.rs"]
mod native_view_pages;
#[path = "../src/native_view_sessions.rs"]
mod native_view_sessions;
#[path = "../src/native_view_status.rs"]
mod native_view_status;
#[path = "../src/onboarding.rs"]
mod onboarding;
#[path = "../src/pair.rs"]
mod pair;
#[path = "../src/service_commands.rs"]
mod service_commands;
#[path = "../src/shutdown.rs"]
mod shutdown;
#[path = "../src/terminal_host.rs"]
mod terminal_host;
#[path = "../src/themes.rs"]
mod themes;
#[path = "../src/title.rs"]
mod title;
#[path = "../src/transcript.rs"]
mod transcript;
#[path = "../src/tui_paint.rs"]
mod tui_paint;

#[cfg(target_os = "macos")]
mod platform_alias_tests {
    use super::native_ui_preferences::UiPreferences;
    use std::{
        fs,
        os::unix::fs::{symlink, PermissionsExt},
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    struct Fixture {
        root: PathBuf,
        alias: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let canonical = std::env::temp_dir().canonicalize().unwrap();
            assert!(
                canonical.starts_with("/private/var"),
                "macOS test temp directory must use the platform var alias"
            );
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let root = canonical.join(format!(
                "state-alias-{}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&root).unwrap();
            fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
            let alias = PathBuf::from("/").join(root.strip_prefix("/private").unwrap());
            Self { root, alias }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn platform_var_alias_loads_existing_preferences_without_diagnostic() {
        let fixture = Fixture::new();
        fs::write(
            fixture.root.join("kv.json"),
            br#"{"theme":"nord","sidebar":"hide","unrelated":7}"#,
        )
        .unwrap();
        let preferences = UiPreferences::load(&[], &fixture.alias.join("kv.json")).unwrap();
        assert!(
            preferences.diagnostics.is_empty(),
            "{:?}",
            preferences.diagnostics
        );
        assert_eq!(preferences.theme.as_deref(), Some("nord"));
        assert!(preferences.sidebar_hidden);
    }

    #[test]
    fn platform_var_alias_persists_without_replacing_unrelated_preferences() {
        let fixture = Fixture::new();
        fs::write(fixture.root.join("kv.json"), br#"{"unrelated":7}"#).unwrap();
        UiPreferences::persist_sidebar(&fixture.alias.join("kv.json"), true).unwrap();
        let value: serde_json::Value =
            serde_json::from_slice(&fs::read(fixture.root.join("kv.json")).unwrap()).unwrap();
        assert_eq!(value["unrelated"], 7);
        assert_eq!(value["sidebar"], "hide");
    }

    #[test]
    fn platform_var_alias_creates_missing_owned_state_parent() {
        let fixture = Fixture::new();
        UiPreferences::persist_theme(&fixture.alias.join("state/kv.json"), Some("nord")).unwrap();
        assert_eq!(
            fs::metadata(fixture.root.join("state"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        assert_eq!(
            UiPreferences::load(&[], &fixture.alias.join("state/kv.json"))
                .unwrap()
                .theme
                .as_deref(),
            Some("nord")
        );
    }

    #[test]
    fn platform_alias_does_not_allow_nested_symlink_redirection() {
        let fixture = Fixture::new();
        let outside = fixture.root.join("outside");
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("sentinel"), b"unchanged").unwrap();
        symlink(&outside, fixture.root.join("state")).unwrap();
        assert!(UiPreferences::persist_sidebar(&fixture.alias.join("state/kv.json"), true).is_err());
        assert!(!outside.join("kv.json").exists());
        assert_eq!(fs::read(outside.join("sentinel")).unwrap(), b"unchanged");
    }

    #[test]
    fn platform_alias_does_not_allow_hardlinked_state_destination() {
        let fixture = Fixture::new();
        let target = fixture.root.join("target.json");
        let bytes = br#"{"unrelated":7}"#;
        fs::write(&target, bytes).unwrap();
        fs::hard_link(&target, fixture.root.join("kv.json")).unwrap();
        assert!(UiPreferences::persist_sidebar(&fixture.alias.join("kv.json"), true).is_err());
        assert_eq!(fs::read(target).unwrap(), bytes);
    }
}

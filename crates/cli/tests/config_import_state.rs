//! Disposable-fixture tests for the native config import state/preview/apply
//! contract. The module is included via `#[path]` so CLI registration is not
//! required yet; native setup/settings wiring remains pending.

#[path = "../src/native_config_import.rs"]
mod native_config_import;
#[path = "../src/native_import_dialog.rs"]
mod native_import_dialog;

use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{json, Map, Value};

use native_config_import::{
    apply_confirmed, mark_offer_decision, preview, read_existing_config, reopen_offer,
    show_offer, source_bytes_unchanged, ConfigWritePolicy, ImportError, OfferDecision,
    SourceMeta, MAX_DEPTH, MAX_KEY_BYTES, MAX_STRING_BYTES,
};
use native_import_dialog::{ImportAction, ImportDialog};

struct Sandbox {
    root: PathBuf,
}

impl Sandbox {
    fn new(label: &str) -> Self {
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "rk-config-import-{label}-{}-{nanos}-{}",
            process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).expect("sandbox root");
        Self { root }
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

struct Allow;

impl ConfigWritePolicy for Allow {
    fn authorize_config_import(
        &self,
        _dest: &Path,
        _sources: &[SourceMeta],
    ) -> Result<(), ImportError> {
        Ok(())
    }
}

struct Deny;

impl ConfigWritePolicy for Deny {
    fn authorize_config_import(
        &self,
        _dest: &Path,
        _sources: &[SourceMeta],
    ) -> Result<(), ImportError> {
        Err(ImportError::PolicyDenied("test policy".to_owned()))
    }
}

fn source(label: &str, path: PathBuf) -> SourceMeta {
    SourceMeta {
        label: label.to_owned(),
        path,
    }
}

fn write(path: &Path, bytes: &[u8]) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("fixture parent");
    }
    fs::write(path, bytes).expect("fixture write");
}

fn temp_leftovers(dir: &Path) -> Vec<String> {
    fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .filter(|name| name.ends_with(".tmp"))
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn offer_is_shown_once_and_settings_can_reopen_it() {
    let sandbox = Sandbox::new("offer");
    let state = sandbox.path("state/import-state.json");

    assert!(show_offer(&state).expect("fresh state should offer"));

    write(&state, br#"{"version":1,"decision":"maybe"}"#);
    assert!(show_offer(&state).expect("unknown decision is offer-pending"));
    write(&state, br#"{"version":2,"decision":"accepted"}"#);
    assert!(show_offer(&state).expect("unknown version is offer-pending"));
    write(&state, br#"{"version":1}"#);
    assert!(show_offer(&state).expect("missing decision is offer-pending"));

    mark_offer_decision(&state, OfferDecision::Accepted).expect("record accept");
    // Simulated restart: only the persisted marker is consulted.
    assert!(!show_offer(&state).expect("accepted state"));
    assert!(!show_offer(&state).expect("still accepted"));

    reopen_offer(&state).expect("settings reopen");
    assert!(show_offer(&state).expect("reopened state"));

    mark_offer_decision(&state, OfferDecision::Declined).expect("record decline");
    assert!(!show_offer(&state).expect("declined state"));

    reopen_offer(&state).expect("settings reopen after decline");
    assert!(show_offer(&state).expect("reopened after decline"));

    let state_json: Value =
        serde_json::from_slice(&fs::read(&state).expect("state bytes")).expect("state json");
    assert_eq!(state_json["version"], json!(1));
    assert!(state_json.get("decision").is_none());
    assert!(temp_leftovers(state.parent().expect("state parent")).is_empty());
}

#[test]
fn preview_redacts_secrets_and_keeps_existing_values_on_conflict() {
    let existing = json!({
        "model": "local/model",
        "apiKey": "old-secret",
        "theme": "dark",
        "hooks": {"stop": ["keep"]}
    });
    let patch = json!({
        "model": "upstream/model",
        "apiKey": "new-secret",
        "theme": "light",
        "newSetting": {"enabled": true},
        "headers": {"Authorization": "Bearer new-secret"}
    });

    let result = preview(Some(&existing), &patch).expect("preview");
    assert_eq!(result.merged["model"], "local/model");
    assert_eq!(result.merged["apiKey"], "old-secret");
    assert_eq!(result.merged["theme"], "dark");
    assert_eq!(result.merged["newSetting"]["enabled"], true);
    assert!(result.conflicts.contains(&"/model".to_owned()));
    assert!(result.conflicts.contains(&"/apiKey".to_owned()));
    assert!(result.conflicts.contains(&"/theme".to_owned()));
    assert_eq!(result.redacted["apiKey"], "<redacted>");
    assert_eq!(result.redacted["headers"]["Authorization"], "<redacted>");
    assert_eq!(result.redacted["model"], "local/model");

    let shown = serde_json::to_string(&result.redacted).expect("redacted json");
    assert!(!shown.contains("old-secret"));
    assert!(!shown.contains("new-secret"));
    assert!(!shown.contains("Bearer"));
}

#[test]
fn preview_and_apply_preserve_commands_inertly_without_execution() {
    let sandbox = Sandbox::new("inert-commands");
    let source_file = sandbox.path("opencode/opencode.json");
    write(&source_file, br#"{"model":"upstream"}"#);
    let source_before = fs::read(&source_file).expect("source bytes");
    let source_root = source_file.parent().expect("source root").to_path_buf();
    let sources = [source("opencode", source_root)];

    let sentinel = sandbox.path("executed-sentinel");
    let touch = format!("touch {}", sentinel.display());
    let patch = json!({
        "command": {"deploy": {"template": "deploy", "shell": touch.clone()}},
        "commands": {"deploy": touch.clone()},
        "hooks": {"pre": touch.clone()},
        "provider": {"example": {"options": {"command": "example-mcp-server"}}},
        "agent": {"build": {"prompt": "ok"}}
    });

    let previewed = preview(None, &patch).expect("preview");
    assert!(previewed.merged["commands"].get("deploy").is_some());
    assert!(previewed.merged["hooks"].get("pre").is_some());
    assert_eq!(
        previewed.merged["provider"]["example"]["options"]["command"],
        "example-mcp-server"
    );

    let dest = sandbox.path("native/config.json");
    write(&dest, br#"{"commands":{"local":"echo local"}}"#);
    let report = apply_confirmed(&dest, &patch, true, &sources, &Allow).expect("apply");
    let written: Value =
        serde_json::from_slice(&fs::read(&dest).expect("dest bytes")).expect("dest json");

    // Command/hook definitions are ordinary inert config data: preserved,
    // merged with existing user commands, never executed.
    assert_eq!(written["commands"]["local"], "echo local");
    assert_eq!(written["commands"]["deploy"], Value::String(touch.clone()));
    assert_eq!(written["hooks"]["pre"], Value::String(touch.clone()));
    assert!(written["command"]["deploy"].get("shell").is_some());
    assert_eq!(
        written["provider"]["example"]["options"]["command"],
        "example-mcp-server"
    );
    assert_eq!(written["agent"]["build"]["prompt"], "ok");
    assert!(!sentinel.exists());
    assert!(report.conflicts.is_empty());
    assert_eq!(fs::read(&source_file).expect("source reread"), source_before);
}

#[test]
fn apply_requires_confirmation_and_policy_consent() {
    let sandbox = Sandbox::new("consent");
    let dest = sandbox.path("native/opencode-rk.json");
    let sources = [source("opencode", sandbox.path("opencode"))];
    let patch = json!({"model": "x"});

    assert!(matches!(
        apply_confirmed(&dest, &patch, false, &sources, &Allow),
        Err(ImportError::ConfirmationRequired)
    ));
    assert!(!dest.exists());

    assert!(matches!(
        apply_confirmed(&dest, &patch, true, &sources, &Deny),
        Err(ImportError::PolicyDenied(_))
    ));
    assert!(!dest.exists());
    assert!(!dest.parent().expect("dest parent").exists());
}

#[test]
fn apply_writes_own_destination_atomically_and_leaves_source_unchanged() {
    let sandbox = Sandbox::new("apply");
    let source_file = sandbox.path("opencode/opencode.json");
    write(&source_file, br#"{"model":"upstream/model","theme":"dark"}"#);
    let source_before = fs::read(&source_file).expect("source bytes");
    let source_root = source_file.parent().expect("source root").to_path_buf();
    let sources = [source("opencode global", source_root)];

    let dest = sandbox.path("native/config.json");
    write(&dest, br#"{"model":"local/model","keep":true}"#);
    let patch = json!({"model": "upstream/model", "new": 1});

    let report =
        apply_confirmed(&dest, &patch, true, &sources, &Allow).expect("apply");

    let written: Value =
        serde_json::from_slice(&fs::read(&dest).expect("dest bytes")).expect("dest json");
    assert_eq!(written["model"], "local/model");
    assert_eq!(written["keep"], true);
    assert_eq!(written["new"], 1);
    assert!(report.conflicts.contains(&"/model".to_owned()));
    assert!(report.bytes_written > 0);
    assert_eq!(report.dest, dest);

    assert_eq!(fs::read(&source_file).expect("source reread"), source_before);
    assert!(source_bytes_unchanged(&source_file, &source_before).expect("source compare"));
    assert!(temp_leftovers(dest.parent().expect("dest parent")).is_empty());
}

#[test]
fn apply_refuses_to_overwrite_malformed_existing_config() {
    let sandbox = Sandbox::new("malformed");
    let dest = sandbox.path("native/config.json");
    let broken = b"{ not json";
    write(&dest, broken);
    let sources = [source("opencode", sandbox.path("opencode"))];

    assert!(matches!(
        read_existing_config(&dest),
        Err(ImportError::MalformedExistingConfig)
    ));
    assert!(matches!(
        apply_confirmed(&dest, &json!({"a": 1}), true, &sources, &Allow),
        Err(ImportError::MalformedExistingConfig)
    ));
    assert_eq!(fs::read(&dest).expect("dest reread"), broken);
    assert!(temp_leftovers(dest.parent().expect("dest parent")).is_empty());
}

#[test]
fn apply_rejects_destination_inside_the_source_tree() {
    let sandbox = Sandbox::new("overlap");
    let source_root = sandbox.path("opencode");
    let source_file = source_root.join("opencode.json");
    write(&source_file, br#"{"model":"upstream"}"#);
    let sources = [source("opencode", source_root.clone())];
    let dest = source_root.join("native.json");

    assert!(matches!(
        apply_confirmed(&dest, &json!({"model": "x"}), true, &sources, &Allow),
        Err(ImportError::OverlappingSource)
    ));
    assert!(!dest.exists());
    assert_eq!(
        fs::read(&source_file).expect("source reread"),
        br#"{"model":"upstream"}"#
    );
}

#[test]
fn apply_allows_own_config_sibling_to_a_source_directory() {
    let sandbox = Sandbox::new("sibling");
    let source_root = sandbox.path("config/opencode");
    let source_file = source_root.join("opencode.json");
    write(&source_file, br#"{"model":"upstream"}"#);
    let sources = [source("opencode", source_root)];
    let dest = sandbox.path("config/opencode-rk/config.json");

    let report = apply_confirmed(&dest, &json!({"model": "imported"}), true, &sources, &Allow)
        .expect("own config sibling of a source directory must be allowed");
    assert!(report.conflicts.is_empty());
    let written: Value =
        serde_json::from_slice(&fs::read(&dest).expect("dest bytes")).expect("dest json");
    assert_eq!(written["model"], "imported");
    assert_eq!(
        fs::read(&source_file).expect("source reread"),
        br#"{"model":"upstream"}"#
    );
}

#[cfg(unix)]
#[test]
fn apply_rejects_destination_parent_symlinked_into_source() {
    use std::os::unix::fs::symlink;

    let sandbox = Sandbox::new("symlink-parent");
    let source_root = sandbox.path("opencode");
    let source_file = source_root.join("opencode.json");
    write(&source_file, br#"{"model":"upstream"}"#);
    let source_before = fs::read(&source_file).expect("source bytes");
    let sources = [source("opencode", source_root.clone())];

    let link_dir = sandbox.path("native-link");
    symlink(&source_root, &link_dir).expect("directory symlink");
    let dest = link_dir.join("config.json");

    assert!(matches!(
        apply_confirmed(&dest, &json!({"model": "x"}), true, &sources, &Allow),
        Err(ImportError::OverlappingSource)
    ));
    assert!(!source_root.join("config.json").exists());
    assert_eq!(fs::read(&source_file).expect("source reread"), source_before);
}

#[cfg(unix)]
#[test]
fn apply_rejects_a_symlinked_destination_file() {
    use std::os::unix::fs::symlink;

    let sandbox = Sandbox::new("symlink-dest");
    let source_root = sandbox.path("opencode");
    let source_file = source_root.join("opencode.json");
    write(&source_file, br#"{"model":"upstream"}"#);
    let sources = [source("opencode", source_root)];
    let dest = sandbox.path("native/config.json");
    fs::create_dir_all(dest.parent().expect("dest parent")).expect("dest parent");
    symlink(&source_file, &dest).expect("file symlink");

    assert!(matches!(
        apply_confirmed(&dest, &json!({"model": "x"}), true, &sources, &Allow),
        Err(ImportError::UnsafePath(_))
    ));
    assert_eq!(
        fs::read(&source_file).expect("source reread"),
        br#"{"model":"upstream"}"#
    );
}

#[test]
fn apply_rejects_relative_or_traversing_source_paths() {
    let sandbox = Sandbox::new("source-path");
    let dest = sandbox.path("native/config.json");
    let patch = json!({"model": "x"});

    for bad in [
        PathBuf::from("relative/opencode"),
        sandbox.root.join("../escape"),
    ] {
        let sources = [source("bad", bad)];
        assert!(matches!(
            apply_confirmed(&dest, &patch, true, &sources, &Allow),
            Err(ImportError::UnsafeSource(_))
        ));
    }
    assert!(!dest.exists());
}

#[test]
fn preview_rejects_unbounded_or_malformed_patches() {
    let mut deep = json!({"leaf": true});
    for _ in 0..=MAX_DEPTH {
        deep = json!({"nested": deep});
    }
    assert!(matches!(
        preview(None, &deep),
        Err(ImportError::InvalidPatch(_))
    ));

    let mut long_key = Map::new();
    long_key.insert("k".repeat(MAX_KEY_BYTES + 1), json!(1));
    assert!(matches!(
        preview(None, &Value::Object(long_key)),
        Err(ImportError::InvalidPatch(_))
    ));

    let long_value = json!({"instructions": "v".repeat(MAX_STRING_BYTES + 1)});
    assert!(matches!(
        preview(None, &long_value),
        Err(ImportError::InvalidPatch(_))
    ));

    assert!(matches!(
        preview(None, &json!([1, 2, 3])),
        Err(ImportError::InvalidPatch(_))
    ));
    assert!(matches!(
        preview(Some(&json!([1, 2, 3])), &json!({})),
        Err(ImportError::MalformedExistingConfig)
    ));
}

#[test]
fn repeated_apply_is_idempotent_and_reports_no_conflicts_for_equal_values() {
    let sandbox = Sandbox::new("repeat");
    let source_root = sandbox.path("opencode");
    let sources = [source("opencode", source_root.clone())];
    let dest = sandbox.path("native/config.json");
    let patch = json!({
        "imported": {"enabled": true},
        "commands": {"deploy": "echo deploy"}
    });

    let first = apply_confirmed(&dest, &patch, true, &sources, &Allow).expect("first apply");
    let first_bytes = fs::read(&dest).expect("first bytes");
    let second = apply_confirmed(&dest, &patch, true, &sources, &Allow).expect("second apply");

    assert_eq!(first_bytes, fs::read(&dest).expect("second bytes"));
    assert!(first.conflicts.is_empty());
    assert!(second.conflicts.is_empty());
    let written: Value =
        serde_json::from_slice(&fs::read(&dest).expect("dest bytes")).expect("dest json");
    assert_eq!(written["imported"]["enabled"], true);
    assert_eq!(written["commands"]["deploy"], "echo deploy");
    assert!(!dest.starts_with(&source_root));
    assert!(temp_leftovers(dest.parent().expect("dest parent")).is_empty());
}

#[test]
fn import_dialog_not_now_is_silent_until_settings_reopens() {
    let sandbox = Sandbox::new("dialog-not-now");
    let state = sandbox.path("state/import.json");
    let dest = sandbox.path("native/config.json");
    let sources = [source("opencode", sandbox.path("opencode"))];
    let patch = || json!({"model": "x"});

    let mut dialog =
        ImportDialog::startup(&state, &dest, sources.to_vec(), patch()).expect("startup offer");
    assert!(dialog.is_open());
    assert!(!dest.exists());
    assert!(!state.exists(), "startup must not write the decision marker");
    let visible = dialog.render(80, 24).join("\n");
    assert!(visible.contains("Import settings from OpenCode?"));
    assert!(visible.contains("Import (preview first)"));

    dialog.action(ImportAction::NotNow, &Allow);
    assert!(!dialog.is_open());
    let marker: Value =
        serde_json::from_slice(&fs::read(&state).expect("marker bytes")).expect("marker json");
    assert_eq!(marker["decision"], json!("declined"));
    assert!(!dest.exists());

    let restarted =
        ImportDialog::startup(&state, &dest, sources.to_vec(), patch()).expect("silent restart");
    assert!(!restarted.is_open());
    assert!(restarted.render(80, 24).is_empty());

    let mut reopened = ImportDialog::from_settings(&state, &dest, sources.to_vec(), patch())
        .expect("settings reopen");
    assert!(reopened.is_open());
    let cleared: Value =
        serde_json::from_slice(&fs::read(&state).expect("cleared bytes")).expect("cleared json");
    assert_eq!(cleared["version"], json!(1));
    assert!(cleared.get("decision").is_none());
    assert!(show_offer(&state).expect("offer pending after reopen"));

    reopened.action(ImportAction::NotNow, &Allow);
    assert!(!reopened.is_open());
    assert!(!dest.exists());
}

#[test]
fn import_dialog_preview_apply_preserves_existing_values_and_restarts_silent() {
    let sandbox = Sandbox::new("dialog-apply");
    let source_root = sandbox.path("opencode\n\u{1b}[31m");
    let source_file = source_root.join("opencode.json");
    write(
        &source_file,
        br#"{"model":"upstream/model","theme":"light","apiKey":"source-secret"}"#,
    );
    let source_before = fs::read(&source_file).expect("source bytes");
    let sources = [source("opencode\u{1b}[31m\nsource", source_root)];

    let dest = sandbox.path("native\n\u{1b}/config.json");
    write(
        &dest,
        br#"{"model":"local/model","apiKey":"local-secret","keep":true}"#,
    );
    let patch = json!({
        "model": "upstream/model",
        "apiKey": "upstream-secret",
        "theme": "light",
        "new": 1
    });
    let state = sandbox.path("state/import.json");

    let mut dialog =
        ImportDialog::startup(&state, &dest, sources.to_vec(), patch).expect("startup");
    dialog.action(ImportAction::Import, &Allow);
    assert!(dialog.is_open(), "preview stays open");
    assert!(dialog.error().is_none());

    let redacted = dialog.redacted_preview().expect("redacted preview");
    assert_eq!(redacted["apiKey"], json!("<redacted>"));
    assert_eq!(redacted["model"], json!("local/model"));
    assert!(dialog.conflicts().iter().any(|conflict| conflict == "/model"));
    assert!(dialog.conflicts().iter().any(|conflict| conflict == "/apiKey"));

    let lines = dialog.render(120, 80);
    assert!(
        lines
            .iter()
            .all(|line| !line.chars().any(|character| character.is_control())),
        "source labels and paths must be control-sanitized for display"
    );
    let shown = lines.join("\n");
    assert!(!shown.contains("local-secret"));
    assert!(!shown.contains("upstream-secret"));
    assert!(
        !shown.contains("source-secret"),
        "source credentials must never be read or rendered"
    );
    assert!(shown.contains("/model"));
    assert!(shown.contains("/apiKey"));
    assert_eq!(
        fs::read(&source_file).expect("source after preview"),
        source_before
    );

    dialog.action(ImportAction::Apply, &Allow);
    assert!(dialog.error().is_none());
    assert_eq!(dialog.selected_action(), Some(ImportAction::Close));
    let written: Value =
        serde_json::from_slice(&fs::read(&dest).expect("dest bytes")).expect("dest json");
    assert_eq!(written["model"], json!("local/model"));
    assert_eq!(written["apiKey"], json!("local-secret"));
    assert_eq!(written["keep"], json!(true));
    assert_eq!(written["theme"], json!("light"));
    assert_eq!(written["new"], json!(1));
    assert_eq!(
        fs::read(&source_file).expect("source after apply"),
        source_before
    );

    let marker: Value =
        serde_json::from_slice(&fs::read(&state).expect("marker bytes")).expect("marker json");
    assert_eq!(marker["decision"], json!("accepted"));
    let restarted =
        ImportDialog::startup(&state, &dest, sources.to_vec(), json!({"model": "ignored"}))
            .expect("restart");
    assert!(!restarted.is_open());
}

#[test]
fn import_dialog_policy_denial_writes_nothing_and_close_hides() {
    let sandbox = Sandbox::new("dialog-deny");
    let state = sandbox.path("state/import.json");
    let dest = sandbox.path("native/config.json");
    let source_root = sandbox.path("opencode");
    fs::create_dir_all(&source_root).expect("source root");
    let sources = [source("opencode", source_root)];

    let mut dialog =
        ImportDialog::startup(&state, &dest, sources.to_vec(), json!({"model": "x"}))
            .expect("startup");
    dialog.action(ImportAction::Import, &Allow);
    assert!(dialog.is_open());
    dialog.action(ImportAction::Apply, &Deny);

    assert!(!dest.exists());
    assert!(!dest.parent().expect("dest parent").exists());
    let error = dialog.error().expect("visible error").to_owned();
    assert!(error.contains("denied"), "got: {error}");
    let rendered = dialog.render(80, 24).join("\n");
    assert!(rendered.contains("Import failed"));
    assert!(rendered.contains("denied"));
    assert_eq!(dialog.selected_action(), Some(ImportAction::Close));

    dialog.action(ImportAction::Close, &Allow);
    assert!(!dialog.is_open());
    assert!(dialog.render(80, 24).is_empty());

    let marker: Value =
        serde_json::from_slice(&fs::read(&state).expect("marker bytes")).expect("marker json");
    assert_eq!(marker["decision"], json!("accepted"));
    let restarted =
        ImportDialog::startup(&state, &dest, sources.to_vec(), json!({"model": "x"}))
            .expect("restart");
    assert!(!restarted.is_open());
    assert!(!dest.exists());
}

#[test]
fn import_dialog_cropped_choice_label_is_inert_at_every_narrow_width() {
    let sandbox = Sandbox::new("dialog-width");
    let mut dialog = ImportDialog::startup(
        &sandbox.path("state/import.json"),
        &sandbox.path("native/config.json"),
        vec![source("opencode", sandbox.path("opencode"))],
        json!({"model": "x"}),
    )
    .expect("startup");

    let full = dialog.render(200, 24);
    let mut choices = 0usize;
    for (row, line) in full.iter().enumerate() {
        let Some(action) = dialog.action_at(200, 24, row, 0) else {
            continue;
        };
        choices += 1;
        let required = line.chars().count();
        for width in 0..required {
            assert_eq!(
                dialog.action_at(width, 24, row, 0),
                None,
                "{action:?} must be inert when its label is cropped at width {width}"
            );
        }
        assert_eq!(dialog.action_at(required, 24, row, 0), Some(action));
        assert_eq!(
            dialog.action_at(required, 24, row, required),
            None,
            "past the rendered label is inert"
        );
    }
    assert_eq!(choices, 2);
}

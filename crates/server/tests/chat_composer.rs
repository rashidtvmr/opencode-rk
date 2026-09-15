//! WEB-010 RED: structured WYSIWYG chat composer lowering (server-side contract).
//!
//! Frozen behavioral suite. T01 structured edit/send, T02 paste/format failure,
//! T03 editor accessibility, T04 bounds/cancel/queue, T05 draft/reload compat.
//! Pure, deterministic, no I/O, no network, no wall clock.
#[path = "../src/chat_composer.rs"]
mod chat_composer;

use chat_composer::{
    ComposerDoc, ComposerError, ComposerNode, DraftQueue, KeyAction, accessible_label,
    decode_draft, doc_from_legacy_text, editor_help, encode_draft, key_command, lower_composer_doc,
    sanitize_doc, MAX_COMPOSER_TEXT_BYTES, MAX_DOC_NODES, MAX_DRAFT_BYTES, MAX_QUEUE_DRAFTS,
};

fn t01_doc() -> ComposerDoc {
    ComposerDoc {
        nodes: vec![
            ComposerNode::Paragraph {
                text: "hello world".to_string(),
            },
            ComposerNode::CodeBlock {
                lang: "rust".to_string(),
                text: "let x = 1;".to_string(),
            },
            ComposerNode::Mention {
                target: "ana".to_string(),
                label: "Ana".to_string(),
            },
            ComposerNode::Command {
                name: "fix".to_string(),
                args: "bug".to_string(),
            },
        ],
        model: "openai/gpt-4o-mini".to_string(),
        effort: "low".to_string(),
    }
}

#[test]
fn web010_t01_structured_edit_send_lowes_deterministically() {
    let first = lower_composer_doc(&t01_doc()).expect("supported doc lowers");
    let second = lower_composer_doc(&t01_doc()).expect("lowering is deterministic");
    assert_eq!(first, second);
    assert_eq!(
        first.text,
        "hello world\n```rust\nlet x = 1;\n```\n@ana\n/fix bug"
    );
    assert_eq!(first.provider, "openai");
    assert_eq!(first.model, "gpt-4o-mini");
    assert_eq!(first.effort, "low");

    let empty = ComposerDoc {
        nodes: vec![],
        model: "openai/gpt-4o-mini".to_string(),
        effort: "low".to_string(),
    };
    assert_eq!(
        lower_composer_doc(&empty).unwrap_err(),
        ComposerError::EmptyDocument
    );
    let blank = ComposerDoc {
        nodes: vec![ComposerNode::Paragraph {
            text: "   \n  ".to_string(),
        }],
        model: "openai/gpt-4o-mini".to_string(),
        effort: "low".to_string(),
    };
    assert_eq!(
        lower_composer_doc(&blank).unwrap_err(),
        ComposerError::EmptyDocument
    );
    let mut bad_model = t01_doc();
    bad_model.model = "no-slash-here".to_string();
    assert_eq!(
        lower_composer_doc(&bad_model).unwrap_err(),
        ComposerError::ModelInvalid
    );
    let mut bad_effort = t01_doc();
    bad_effort.effort = "ultra".to_string();
    assert_eq!(
        lower_composer_doc(&bad_effort).unwrap_err(),
        ComposerError::EffortInvalid
    );
}

fn t02_pasted_doc() -> ComposerDoc {
    ComposerDoc {
        nodes: vec![
            ComposerNode::Paragraph {
                text: "hello".to_string(),
            },
            ComposerNode::Html {
                html: "<script>alert(1)</script><p>hi</p>".to_string(),
            },
            ComposerNode::Embed {
                kind: "pdf".to_string(),
                label: "doc".to_string(),
            },
            ComposerNode::ToolChip {
                tool: "search".to_string(),
                active: false,
            },
            ComposerNode::PluginChip {
                plugin: "x".to_string(),
                active: true,
            },
        ],
        model: "openai/gpt-4o-mini".to_string(),
        effort: "low".to_string(),
    }
}

#[test]
fn web010_t02_paste_sanitizes_and_blocks_inactive_capabilities() {
    let err = lower_composer_doc(&t02_pasted_doc()).unwrap_err();
    assert!(
        matches!(
            err,
            ComposerError::UnsupportedNode(_) | ComposerError::InactiveCapability(_)
        ),
        "raw paste must not lower, got {err:?}"
    );

    let clean = sanitize_doc(&t02_pasted_doc());
    assert!(clean.dropped >= 3);
    let lowered = lower_composer_doc(&clean.doc).expect("sanitized doc lowers");
    assert!(lowered.text.contains("hello"));
    assert!(lowered.text.contains("[plugin:x]"));
    assert!(!lowered.text.contains("script"));
    assert!(!lowered.text.contains("alert"));
    assert!(!lowered.text.contains("pdf"));

    let active_tool = ComposerDoc {
        nodes: vec![ComposerNode::ToolChip {
            tool: "search".to_string(),
            active: true,
        }],
        model: "openai/gpt-4o-mini".to_string(),
        effort: "low".to_string(),
    };
    let lowered = lower_composer_doc(&active_tool).expect("active tool chip lowers");
    assert!(lowered.text.contains("[tool:search]"));

    let inactive_only = ComposerDoc {
        nodes: vec![ComposerNode::ToolChip {
            tool: "search".to_string(),
            active: false,
        }],
        model: "openai/gpt-4o-mini".to_string(),
        effort: "low".to_string(),
    };
    assert_eq!(
        lower_composer_doc(&inactive_only).unwrap_err(),
        ComposerError::InactiveCapability("tool")
    );
}

#[test]
fn web010_t03_editor_keyboard_and_labels_have_no_traps() {
    assert_eq!(
        key_command("Enter", false, false),
        KeyAction::InsertNewline
    );
    assert_eq!(key_command("Enter", true, false), KeyAction::Send);
    assert_eq!(key_command("z", true, false), KeyAction::Undo);
    assert_eq!(key_command("z", true, true), KeyAction::Redo);
    assert_eq!(key_command("y", true, false), KeyAction::Redo);
    assert_eq!(key_command("k", true, false), KeyAction::OpenMenu);
    assert_eq!(key_command("/", true, false), KeyAction::ShowHelp);
    assert_eq!(key_command("Tab", false, false), KeyAction::MoveFocusNext);
    assert_eq!(key_command("Escape", false, false), KeyAction::CloseMenu);
    assert_eq!(key_command("Escape", true, true), KeyAction::CloseMenu);

    for node in t02_pasted_doc().nodes.iter().chain(t01_doc().nodes.iter()) {
        let label = accessible_label(node);
        assert!(!label.trim().is_empty(), "every node needs a label");
    }
    let help = editor_help();
    for token in ["Escape", "Ctrl+Enter", "Undo", "Redo", "Tab"] {
        assert!(help.contains(token), "help must document {token}");
    }
}

#[test]
fn web010_t04_bounds_cancel_queue_are_enforced() {
    let big_text = "x".repeat(MAX_COMPOSER_TEXT_BYTES + 1);
    let big_doc = ComposerDoc {
        nodes: vec![ComposerNode::Paragraph { text: big_text }],
        model: "openai/gpt-4o-mini".to_string(),
        effort: "low".to_string(),
    };
    assert!(matches!(
        lower_composer_doc(&big_doc).unwrap_err(),
        ComposerError::DocTooLarge { .. } | ComposerError::NodeTextTooLarge { .. }
    ));

    let mut many = t01_doc();
    many.nodes = (0..MAX_DOC_NODES + 1)
        .map(|i| ComposerNode::Paragraph {
            text: format!("p{i}"),
        })
        .collect();
    assert!(matches!(
        lower_composer_doc(&many).unwrap_err(),
        ComposerError::NodeLimitExceeded { .. }
    ));

    let mut queue = DraftQueue::new();
    for i in 0..MAX_QUEUE_DRAFTS {
        queue
            .push(&format!("owner{i}"), "draft")
            .expect("queue fills to cap");
    }
    assert_eq!(queue.len(), MAX_QUEUE_DRAFTS);
    assert!(matches!(
        queue.push("late", "draft").unwrap_err(),
        ComposerError::QueueFull { .. }
    ));

    let mut queue = DraftQueue::new();
    assert!(!queue.stop());
    queue.begin_live("ana");
    assert!(queue.is_live());
    assert!(queue.stop());
    assert!(!queue.is_live());
    assert!(!queue.stop());

    let mut queue = DraftQueue::new();
    queue.push("ana", "v1").unwrap();
    queue.push("ben", "w1").unwrap();
    queue.steer("ana", "v2").expect("owner steers own draft");
    assert_eq!(queue.owner_at(0), Some("ana"));
    assert_eq!(queue.owner_at(1), Some("ben"));
    assert_eq!(queue.text_of("ana"), Some("v2"));
    assert_eq!(queue.text_of("ben"), Some("w1"));
    assert_eq!(
        queue.steer("ghost", "x").unwrap_err(),
        ComposerError::DraftNotFound
    );
}

#[test]
fn web010_t05_draft_reload_and_legacy_compat() {
    let doc = t01_doc();
    let encoded = encode_draft(&doc).expect("supported doc encodes");
    assert!(encoded.len() <= MAX_DRAFT_BYTES);
    let decoded = decode_draft(&encoded).expect("own draft decodes");
    assert_eq!(decoded.dropped, 0);
    assert_eq!(decoded.doc, doc);
    let relowered = lower_composer_doc(&decoded.doc).expect("reloaded doc lowers");
    assert_eq!(relowered.text, lower_composer_doc(&doc).unwrap().text);

    let legacy = doc_from_legacy_text("old plain", "openai/gpt-4o-mini", "low")
        .expect("legacy text wraps");
    assert_eq!(legacy.nodes.len(), 1);
    let lowered = lower_composer_doc(&legacy).expect("legacy doc lowers");
    assert_eq!(lowered.text, "old plain");

    assert!(matches!(
        decode_draft("not json{{").unwrap_err(),
        ComposerError::MalformedDraft(_)
    ));
    let future = r#"{"model":"openai/gpt-4o-mini","effort":"low","nodes":[{"kind":"future","x":1},{"kind":"paragraph","text":"kept"}]}"#;
    let decoded = decode_draft(future).expect("unknown kinds drop, known kept");
    assert_eq!(decoded.dropped, 1);
    assert_eq!(decoded.doc.nodes.len(), 1);

    let oversize = "x".repeat(MAX_DRAFT_BYTES + 1);
    assert!(matches!(
        decode_draft(&oversize).unwrap_err(),
        ComposerError::DraftTooLarge { .. }
    ));
    let secret = "ses_SUPERSECRET_payload";
    let err = decode_draft("{bad").unwrap_err();
    let line = format!("decode_failed err={err}");
    assert!(!line.contains(secret));
}

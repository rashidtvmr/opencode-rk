#[path = "../src/web_attachments.rs"]
mod web_attachments;

use web_attachments::{
    AttachmentStore, AttachError, Source, MAX_ATTACHMENTS_PER_TURN, MAX_ATTACHMENT_BYTES,
    alt_text, chip_label, drop_control, library_control, picker_control, preview_control,
    provider_reference, remove_control, validate_attachment,
};

fn png_bytes() -> Vec<u8> {
    vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 1, 2, 3, 4]
}

#[test]
fn web011_t01_ingest_send() {
    let mut store = AttachmentStore::new();
    let rec = store
        .ingest("shot.png", "image/png", &png_bytes(), Source::Picker)
        .expect("allowed png ingests");
    assert!(chip_label(&rec).contains("shot.png"), "composer chip names file");
    let pref = provider_reference(&rec);
    assert!(pref.starts_with("blob:"), "provider gets real blob ref, got {pref}");
    assert!(pref.contains(&rec.digest), "ref carries digest");
    let again = store
        .ingest("copy.png", "image/png", &png_bytes(), Source::Paste)
        .expect("same bytes ingest");
    assert_eq!(rec.digest, again.digest, "dedupe: one blob");
    assert_eq!(store.blob_count(), 1, "stored once");
    assert!(store.resolve(&rec.digest).is_some(), "resolvable");
}

#[test]
fn web011_t02_rejection() {
    let mut store = AttachmentStore::new();
    let before = store.blob_count();
    let big = vec![0u8; MAX_ATTACHMENT_BYTES + 1];
    assert!(matches!(
        store.ingest("big.png", "image/png", &big, Source::Picker),
        Err(AttachError::Oversize { .. })
    ));
    assert!(matches!(
        store.ingest("run.exe", "application/x-msdownload", b"MZ", Source::Picker),
        Err(AttachError::UnsupportedType(_))
    ));
    assert!(matches!(
        store.ingest("../.env", "text/plain", b"secret", Source::Picker),
        Err(AttachError::Unsafe(_))
    ));
    assert!(matches!(
        store.ingest("", "text/plain", b"x", Source::Picker),
        Err(AttachError::Unsafe(_))
    ));
    assert!(matches!(
        store.ingest("empty.txt", "text/plain", b"", Source::Picker),
        Err(AttachError::Empty)
    ));
    assert!(matches!(
        validate_attachment("ok.txt", "text/plain", 3),
        Ok(())
    ));
    assert_eq!(store.blob_count(), before, "rejections leave no blob");
    assert!(store.resolve("blob:does-not-exist").is_none());
}

#[test]
fn web011_t03_accessible_picker_preview() {
    for ctl in [
        picker_control(),
        drop_control(),
        library_control(),
        preview_control(),
        remove_control(),
    ] {
        assert!(ctl.keyboard_operable, "control {} keyboard operable", ctl.id);
        assert!(!ctl.label.is_empty(), "control {} labelled", ctl.id);
        assert!(!ctl.role.is_empty(), "control {} roled", ctl.id);
    }
    let mut store = AttachmentStore::new();
    let rec = store
        .ingest("shot.png", "image/png", &png_bytes(), Source::DragDrop)
        .unwrap();
    let alt = alt_text(&rec);
    assert!(alt.contains("shot.png"), "preview has text alternative");
    assert!(alt.contains("image/png"), "alternative names type");
}

#[test]
fn web011_t04_resource_lifecycle() {
    let mut store = AttachmentStore::new();
    for i in 0..MAX_ATTACHMENTS_PER_TURN {
        let bytes = vec![i as u8; 8];
        store
            .ingest(&format!("f{i}.txt"), "text/plain", &bytes, Source::Picker)
            .expect("within count bound");
    }
    assert!(matches!(
        store.ingest("extra.txt", "text/plain", b"x", Source::Picker),
        Err(AttachError::TooMany)
    ));
    store.stage_temp(1024).expect("stage temp");
    assert!(store.temp_bytes_used() > 0);
    store.abort_temp();
    assert_eq!(store.temp_bytes_used(), 0, "abort cleans temp");
    let mut shared = AttachmentStore::new();
    let a = shared
        .ingest("a.png", "image/png", &png_bytes(), Source::Picker)
        .unwrap();
    shared
        .ingest("b.png", "image/png", &png_bytes(), Source::Paste)
        .unwrap();
    assert!(shared.remove_attachment(&a.digest), "removal reports true");
    assert!(
        shared.resolve(&a.digest).is_some(),
        "shared blob survives one removal"
    );
    assert!(!shared.remove_attachment("blob:missing"), "missing removal false");
}

#[test]
fn web011_t05_library_reload_fidelity() {
    let mut store = AttachmentStore::new();
    let rec = store
        .ingest("shot.png", "image/png", &png_bytes(), Source::Screenshot)
        .unwrap();
    store
        .add_to_library(&rec.digest, "screenshot:window-1".to_string())
        .expect("library pin");
    let snap = store.export_snapshot();
    let mut fresh = AttachmentStore::new();
    fresh.import_snapshot(&snap).expect("reload restores");
    let by_ref = fresh
        .attach_reference(&rec.digest)
        .expect("attach-by-reference after reload");
    assert_eq!(by_ref.digest, rec.digest, "dedupe digest survives reload");
    assert_eq!(by_ref.source, Source::Library, "reference source kept");
    let entry = fresh
        .library_resolve(&rec.digest)
        .expect("library entry resolvable");
    assert_eq!(entry.origin, "screenshot:window-1", "source metadata survives");
    assert!(fresh.resolve(&rec.digest).is_some(), "blob resolvable");
}

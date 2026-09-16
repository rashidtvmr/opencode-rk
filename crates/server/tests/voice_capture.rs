//! WEB-016 frozen tests T01..T05 (server voice-capture contract, no web/ touch).
//!
//! The lane owns exactly two files and never edits shared `lib.rs`, so the
//! transport module is included via path (the integrator wires
//! `mod voice_capture;` into `lib.rs` later).

#[path = "../src/voice_capture.rs"]
mod voice_capture;

use voice_capture::{MicGrant, MicState, VoiceConfig, VoiceError, VoiceSession};

// WEB-016-T01 (audio happy path: granted capture produces editable transcript).
#[test]
fn web016_t01_audio_happy_path() {
    let mut s = VoiceSession::new(VoiceConfig::default(), MicGrant::Granted, true);
    assert_eq!(s.state(), MicState::Idle);
    s.start().expect("granted + adapter starts");
    assert_eq!(s.state(), MicState::Capturing);
    assert_eq!(s.live_tracks(), 1);
    s.push_utterance(1, "hello world").expect("frame accepted");
    assert_eq!(s.utterance_count(), 1);
    assert!(s.transcript().contains("hello world"));
    s.edit_transcript(1, "hello edited")
        .expect("transcript editable");
    assert_eq!(s.transcript(), "hello edited");
    assert!(s.text_chat_usable());
}

// WEB-016-T02 (permission/adapter failure stays explicit, no fake transcript).
#[test]
fn web016_t02_permission_adapter_failure_explicit() {
    let mut denied = VoiceSession::new(VoiceConfig::default(), MicGrant::Denied, true);
    assert_eq!(denied.start(), Err(VoiceError::PermissionDenied));
    assert_eq!(
        denied.push_utterance(1, "x"),
        Err(VoiceError::PermissionDenied)
    );
    assert!(denied.transcript().is_empty());
    assert_eq!(denied.utterance_count(), 0);
    assert!(denied.text_chat_usable());

    let mut nodev = VoiceSession::new(VoiceConfig::default(), MicGrant::NoDevice, true);
    assert_eq!(nodev.start(), Err(VoiceError::NoDevice));
    assert_eq!(nodev.push_utterance(1, "x"), Err(VoiceError::NoDevice));
    assert!(nodev.transcript().is_empty());
    assert!(nodev.text_chat_usable());

    let mut noadapter = VoiceSession::new(VoiceConfig::default(), MicGrant::Granted, false);
    assert_eq!(noadapter.start(), Err(VoiceError::NoAdapter));
    assert_eq!(
        noadapter.push_utterance(1, "fake"),
        Err(VoiceError::NoAdapter)
    );
    assert!(noadapter.transcript().is_empty());
    assert_eq!(noadapter.utterance_count(), 0);
    assert!(noadapter.text_chat_usable());
}

// WEB-016-T03 (non-audio labels/state, keyboard stop/mute, voice never only path).
#[test]
fn web016_t03_accessibility_labels_and_keyboard_path() {
    let mut s = VoiceSession::new(VoiceConfig::default(), MicGrant::Granted, true);
    assert!(s.text_chat_usable());
    for label in [s.state_label().to_string(), s.mic_label()] {
        assert!(!label.is_empty());
        assert!(label.chars().any(|c| c.is_alphabetic()));
    }
    let controls = s.control_labels();
    assert_eq!(controls.len(), 3);
    for control in controls {
        assert!(!control.is_empty());
    }
    assert!(controls[0] != controls[1] && controls[1] != controls[2] && controls[0] != controls[2]);
    let idle_label = s.state_label();
    s.start().expect("granted + adapter starts");
    assert!(s.text_chat_usable());
    let live_label = s.state_label();
    assert_ne!(idle_label, live_label);
    s.mute().expect("keyboard mute");
    assert_eq!(s.state(), MicState::Muted);
    assert_ne!(s.state_label(), live_label);
    s.unmute().expect("keyboard unmute");
    assert_eq!(s.state(), MicState::Capturing);
    s.stop();
    assert_eq!(s.state(), MicState::Stopped);
    assert!(s.text_chat_usable());
}

// WEB-016-T04 (bounded buffers/duration/reconnects; stop/unmount releases tracks).
#[test]
fn web016_t04_bounds_and_lifecycle_release() {
    let config = VoiceConfig {
        max_utterances: 2,
        max_transcript_chars: 10,
        max_capture_secs: 60,
        max_reconnects: 1,
    };
    let mut s = VoiceSession::new(config, MicGrant::Granted, true);
    s.start().expect("granted + adapter starts");
    assert_eq!(s.live_tracks(), 1);
    s.push_utterance(1, "hello").expect("5 chars fit");
    assert_eq!(s.push_utterance(2, "world!"), Err(VoiceError::TooLarge));
    assert_eq!(s.utterance_count(), 1);
    s.push_utterance(2, "hi").expect("small utterance fits");
    assert_eq!(s.push_utterance(3, "yo"), Err(VoiceError::TooLarge));
    assert_eq!(s.utterance_count(), 2);
    s.mute().expect("mute pauses capture");
    assert_eq!(s.live_tracks(), 0);
    assert_eq!(s.push_utterance(9, "muted speech"), Err(VoiceError::Muted));
    assert_eq!(s.utterance_count(), 2);
    s.unmute().expect("unmute resumes capture");
    assert_eq!(s.live_tracks(), 1);
    assert_eq!(s.advance_secs(61), Err(VoiceError::CaptureTooLong));
    assert!(s.released());
    assert_eq!(s.live_tracks(), 0);
    s.reconnect().expect("first reconnect within bound");
    assert_eq!(s.reconnect(), Err(VoiceError::TooManyReconnects));
    s.start().expect("restart after reconnect");
    s.release();
    assert!(s.released());
    assert_eq!(s.live_tracks(), 0);
    assert_eq!(s.utterance_count(), 2);
    assert!(!s.transcript().is_empty());
}

// WEB-016-T05 (reconnect dedupes utterances; retention is a bounded suffix).
#[test]
fn web016_t05_reconnect_no_duplicates_and_retention() {
    let mut s = VoiceSession::new(VoiceConfig::default(), MicGrant::Granted, true);
    s.start().expect("granted + adapter starts");
    s.push_utterance(7, "first utterance")
        .expect("utterance accepted");
    s.stop();
    s.reconnect().expect("reconnect within bound");
    s.start().expect("restart after reconnect");
    assert_eq!(
        s.push_utterance(7, "first utterance"),
        Err(VoiceError::Duplicate)
    );
    assert_eq!(s.utterance_count(), 1);
    s.push_utterance(8, "second utterance")
        .expect("new utterance accepted");
    assert_eq!(s.utterance_count(), 2);
    let retained = s.retained_transcript(9);
    assert!(retained.chars().count() <= 9);
    assert!(retained.contains("utterance"));
    let dir = tempfile::tempdir().expect("disposable fixture");
    let marker = dir.path().join("transcript.txt");
    std::fs::write(&marker, retained.as_bytes()).expect("fixture write");
    let back = std::fs::read_to_string(&marker).expect("fixture read");
    assert_eq!(back, retained);
    let rendered = format!("{} {:?}", VoiceError::Duplicate, VoiceError::Duplicate);
    assert!(!rendered.contains("utterance"));
}

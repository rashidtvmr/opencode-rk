//! Recovery contract for abandoned terminal reply strings.
//!
//! These tests intentionally require a bounded abandoned state: timing out a
//! control string must not return directly to Ground, because a delayed reply
//! tail would otherwise become composer input.  A fresh quiet ESC is the
//! explicit resynchronization escape hatch, matching abandoned-paste policy.

#[path = "../src/native_input_decoder.rs"]
mod native_input_decoder;

use native_input_decoder::{InputEvent, TerminalInputDecoder, ESCAPE_GRACE};
use std::time::{Duration, Instant};

fn feed(
    decoder: &mut TerminalInputDecoder,
    bytes: &[u8],
    now: Instant,
) -> Vec<InputEvent> {
    let mut events = Vec::new();
    for &byte in bytes {
        decoder.push(byte, now);
        while let Some(event) = decoder.pop() {
            events.push(event);
        }
    }
    events
}

fn bytes(events: Vec<InputEvent>) -> Vec<u8> {
    events
        .into_iter()
        .map(|event| match event {
            InputEvent::Byte(byte) => byte,
            InputEvent::Escape => panic!("unexpected Escape event"),
            InputEvent::Paste(text) => panic!("unexpected Paste event: {text:?}"),
            InputEvent::ArrowUp => panic!("unexpected ArrowUp event"),
            InputEvent::ArrowDown => panic!("unexpected ArrowDown event"),
        })
        .collect()
}

#[test]
fn abandoned_osc_discards_delayed_payload_until_bel_then_preserves_text() {
    let start = Instant::now();
    let abandoned = start + Duration::from_secs(6);
    let mut decoder = TerminalInputDecoder::default();

    assert!(feed(&mut decoder, b"\x1b]11;", start).is_empty());
    decoder.expire(abandoned);

    assert!(feed(
        &mut decoder,
        b"late payload [24;80R",
        abandoned,
    )
    .is_empty());
    assert!(feed(&mut decoder, b"\x07", abandoned).is_empty());
    assert_eq!(bytes(feed(&mut decoder, b"hello", abandoned)), b"hello");
}

#[test]
fn abandoned_dcs_and_apc_require_st_and_discard_late_payload() {
    let start = Instant::now();
    let abandoned = start + Duration::from_secs(6);

    for opener in [b"\x1bP1$r".as_slice(), b"\x1b_X=1;".as_slice()] {
        let mut decoder = TerminalInputDecoder::default();
        assert!(feed(&mut decoder, opener, start).is_empty());
        decoder.expire(abandoned);

        // BEL is body for DCS/APC; it must not release the abandoned frame.
        assert!(feed(&mut decoder, b"late\x07body", abandoned).is_empty());
        assert!(feed(&mut decoder, b"\x1b", abandoned).is_empty());
        assert!(feed(&mut decoder, b"\\", abandoned).is_empty());
        assert_eq!(bytes(feed(&mut decoder, b"hello", abandoned)), b"hello");
    }
}

#[test]
fn fresh_quiet_escape_recovers_abandoned_osc_and_then_allows_text() {
    let start = Instant::now();
    let abandoned = start + Duration::from_secs(6);
    let quiet_escape_expiry = abandoned + ESCAPE_GRACE + Duration::from_millis(1);
    let mut decoder = TerminalInputDecoder::default();

    assert!(feed(&mut decoder, b"\x1b]11;", start).is_empty());
    decoder.expire(abandoned);
    assert!(feed(&mut decoder, b"delayed reply", abandoned).is_empty());

    assert!(feed(&mut decoder, b"\x1b", abandoned).is_empty());
    decoder.expire(quiet_escape_expiry);
    assert_eq!(decoder.pop(), Some(InputEvent::Escape));
    assert_eq!(decoder.pop(), None);
    assert_eq!(bytes(feed(&mut decoder, b"hello", quiet_escape_expiry)), b"hello");
}

#[test]
fn st_during_quiet_escape_closes_abandoned_osc_without_escape_event() {
    let start = Instant::now();
    let abandoned = start + Duration::from_secs(6);
    let before_quiet_expiry = abandoned + ESCAPE_GRACE / 2;
    let mut decoder = TerminalInputDecoder::default();

    assert!(feed(&mut decoder, b"\x1b]11;", start).is_empty());
    decoder.expire(abandoned);
    assert!(feed(&mut decoder, b"\x1b", abandoned).is_empty());
    assert!(feed(&mut decoder, b"\\", before_quiet_expiry).is_empty());
    decoder.expire(abandoned + ESCAPE_GRACE + Duration::from_millis(1));
    assert_eq!(decoder.pop(), None);
    assert_eq!(bytes(feed(&mut decoder, b"hello", before_quiet_expiry)), b"hello");
}

#[test]
fn normal_split_st_with_short_interbyte_delay_never_emits_escape() {
    let start = Instant::now();
    let after_escape = start + ESCAPE_GRACE / 2;

    for opener in [b"\x1b]11;reply".as_slice(), b"\x1bPreply".as_slice(), b"\x1b_X=1;reply".as_slice()] {
        let mut decoder = TerminalInputDecoder::default();
        assert!(feed(&mut decoder, opener, start).is_empty());
        assert!(feed(&mut decoder, b"\x1b", start).is_empty());
        assert!(feed(&mut decoder, b"\\", after_escape).is_empty());
        assert_eq!(bytes(feed(&mut decoder, b"hello", after_escape)), b"hello");
    }
}

#[test]
fn ctrl_c_and_ctrl_d_keep_exit_priority_outside_reply_frames() {
    let now = Instant::now();
    for control in [3u8, 4] {
        let mut decoder = TerminalInputDecoder::default();
        assert_eq!(
            feed(&mut decoder, &[0x1b, b']', b'1', control], now),
            vec![InputEvent::Byte(control)]
        );
    }
}

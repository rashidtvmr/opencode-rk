//! Terminal replies queried during native TUI startup are protocol frames, not
//! composer input. These tests include the production decoder directly so they
//! exercise its byte-stream contract without starting a terminal or CLI.

#[path = "../src/native_input_decoder.rs"]
mod native_input_decoder;

use native_input_decoder::{InputEvent, TerminalInputDecoder, ESCAPE_GRACE};
use std::time::{Duration, Instant};

fn decode_chunks(chunks: &[&[u8]]) -> Vec<InputEvent> {
    let mut decoder = TerminalInputDecoder::default();
    let now = Instant::now();
    let mut events = Vec::new();
    for chunk in chunks {
        for &byte in *chunk {
            decoder.push(byte, now);
            events.extend(std::iter::from_fn(|| decoder.pop()));
        }
    }
    events
}

fn byte_events(events: Vec<InputEvent>) -> Vec<u8> {
    events
        .into_iter()
        .map(|event| match event {
            InputEvent::Byte(byte) => byte,
            other => panic!("expected only ordinary text bytes, got {other:?}"),
        })
        .collect()
}

fn assert_reply_is_not_input(reply: &[u8]) {
    let input = b"hello";
    let mut stream = Vec::from(reply);
    stream.extend_from_slice(input);

    // Every possible two-chunk boundary must have the same behavior. The
    // decoder is deliberately fed one byte at a time within each chunk.
    for split in 0..=stream.len() {
        assert_eq!(
            byte_events(decode_chunks(&[&stream[..split], &stream[split..]])),
            input,
            "terminal reply leaked or swallowed input at byte split {split}: {reply:?}"
        );
    }
}

#[test]
fn osc_10_and_11_color_replies_are_consumed_at_every_chunk_boundary() {
    // OSC 11 is the reply observed in the native composer on a fresh HOME.
    // OSC 10 is the corresponding foreground-color query reply. Exercise both
    // BEL and ST terminators because terminals use either form.
    for reply in [
        b"\x1b]11;rgb:213d/2743/33e7\x07".as_slice(),
        b"\x1b]11;rgb:213d/2743/33e7\x1b\\".as_slice(),
        b"\x1b]10;rgb:ffff/ffff/ffff\x07".as_slice(),
        b"\x1b]10;rgb:ffff/ffff/ffff\x1b\\".as_slice(),
    ] {
        assert_reply_is_not_input(reply);
    }
}

#[test]
fn csi_device_and_cursor_reports_and_dcs_responses_are_not_composer_input() {
    // Primary device attributes, cursor position report, DEC status-string
    // reply, and a Kitty graphics response (DCS) are all terminal protocol.
    for reply in [
        b"\x1b[?1;2c".as_slice(),
        b"\x1b[24;80R".as_slice(),
        b"\x1bP1$r0m\x1b\\".as_slice(),
        b"\x1b_Gi=31;OK\x1b\\".as_slice(),
    ] {
        assert_reply_is_not_input(reply);
    }
}

#[test]
fn printable_color_reply_text_without_an_escape_prefix_remains_typed_text() {
    let typed = b"]11;rgb:213d/2743/33e7";
    assert_eq!(byte_events(decode_chunks(&[typed])), typed);
}

#[test]
fn bracketed_paste_preserves_color_reply_looking_text_verbatim() {
    let pasted = "前置 ]11;rgb:213d/2743/33e7\\ 中间 ]11;rgb:213d/2743/33e7\u{7} 后置";
    let frame = format!("\x1b[200~{pasted}\x1b[201~");
    assert_eq!(
        decode_chunks(&[frame.as_bytes()]),
        vec![InputEvent::Paste(pasted.to_owned())]
    );
}

#[test]
fn oversized_unfinished_osc_is_discarded_without_leaking_or_eating_following_text() {
    let now = Instant::now();
    let mut decoder = TerminalInputDecoder::default();
    let mut events = Vec::new();
    for &byte in b"\x1b]11;rgb:" {
        decoder.push(byte, now);
        events.extend(std::iter::from_fn(|| decoder.pop()));
    }
    // Exercise the streaming/discard path without assuming a particular OSC
    // capacity or requiring access to decoder internals.
    for _ in 0..64 * 1024 {
        decoder.push(b'x', now);
        events.extend(std::iter::from_fn(|| decoder.pop()));
    }
    decoder.expire(now + ESCAPE_GRACE);
    events.extend(std::iter::from_fn(|| decoder.pop()));
    decoder.push(0x07, now + ESCAPE_GRACE);
    events.extend(std::iter::from_fn(|| decoder.pop()));
    for &byte in b"hello" {
        decoder.push(byte, now + ESCAPE_GRACE);
        events.extend(std::iter::from_fn(|| decoder.pop()));
    }

    events.extend(std::iter::from_fn(|| decoder.pop()));
    assert_eq!(byte_events(events), b"hello");
}

#[test]
fn unicode_typing_surrounding_a_color_reply_is_preserved_byte_for_byte() {
    let reply = b"\x1b]11;rgb:213d/2743/33e7\x07";
    let mut stream = "前置".as_bytes().to_vec();
    stream.extend_from_slice(reply);
    stream.extend_from_slice("後置".as_bytes());

    for split in 0..=stream.len() {
        assert_eq!(
            byte_events(decode_chunks(&[&stream[..split], &stream[split..]])),
            "前置後置".as_bytes(),
            "terminal reply leaked or swallowed Unicode input at byte split {split}"
        );
    }
}

#[test]
fn a_standalone_escape_still_emits_escape_after_its_existing_grace_period() {
    let now = Instant::now();
    let mut decoder = TerminalInputDecoder::default();
    decoder.push(0x1b, now);
    decoder.expire(now + ESCAPE_GRACE + Duration::from_millis(1));
    assert_eq!(decoder.pop(), Some(InputEvent::Escape));
    assert_eq!(decoder.pop(), None);
}

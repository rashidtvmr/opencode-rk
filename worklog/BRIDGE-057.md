# BRIDGE-057 transcript_format

Claim: transcript_format.rs pure format-string port.
Source: packages/tui/src/util/transcript.ts:5-112 @ a0d9b6c; index.tsx:1350-1369 synthetic filter.
Target: crates/opentui-bridge/src/transcript_format.rs, reuses crate::transcript, locale::titlecase.
Tests: 9 tests, literal TS strings (header, input/output/error blocks, duration). RED: file absent pre-write; logical GREEN pending cargo (not run per scope).
Unknowns: toLocaleString dates passed preformatted; Failed/error status wording split.

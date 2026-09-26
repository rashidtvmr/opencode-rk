# FIX-07 scratchpad

claim: LivePipe verify/repair.
evidence: live_transport_full.rs; sdk_stream.rs:9-57; run_stream_transport.rs:22-75; ctx_sync_full.rs:4-29.
observed: new() left backoff_ms=0 (Default), spec requires 1s init; next_backoff used `*2` overflow-prone; comment said "no SSE/backoff/batch" contradicting backoff impl.
target: live_transport_full.rs only.
tests: push_basic, cap_512, ack_frees_slot, backoff_doubles_saturates (incl u32::MAX), reset_restores.
decisions: new() seeds INIT 1000; saturating_mul+clamp; trimmed consts to INIT/MAX; 82 lines rustfmt clean.
unknowns: none.

# Claim: BRIDGE-PAR-170 ses_par170
# Source evidence:
# - TS truth: /home/rashid/projects/opencode/packages/opencode/src/cli/cmd/run/stream.transport.ts (buffered: Event[] + drainBuffered FIFO, seq via tick)
# - Crate boundary: crates/opentui-bridge/src/run_stream_transport.rs (Transport ack-model, NOT edited)
# Target boundary: crates/opentui-bridge/src/stream_transport_full.rs (new, standalone, not wired in lib.rs)
# Tests: in-file #[cfg(test)] >=5; verify rustfmt --check only
# Decisions: u64 seq, 4KiB char-boundary trunc, 512 cap evict-oldest, send->u64, recv FIFO remove(0)

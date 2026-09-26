# BRIDGE-PAR-175 scratchpad
claim: BRIDGE-PAR-175 via cc.claim ses_par175 OK
source: crates/cli/src/tui_entry.rs:582 native_interactive_loop, :200 fetch_snapshot, :779 follow_loop, :854 callsite passes Some(&snapshot)
observed: native_interactive_loop takes live Option<&LiveSnapshot> by ref, never re-fetches; render/submit use stale snapshot for whole session
target: in-loop every 20 iters (u32 wrapping counter) when live.is_some, re-fetch via fetch_snapshot(origin, Some(session_id), auth); Ok=>update local, Err=>keep stale no transcript spam
notes: crate::sdk_stream::SdkStream absent in tree (grep sdk_stream zero hits); using fetch_snapshot path per task
tests: rustfmt --check + cargo check
decisions: own refreshed Option<LiveSnapshot> shadow; live_now=refreshed.as_ref().or(live); clone origin/session_id before fetch to end borrow
done: poll added tui_entry.rs:627-643,642; live_now wired :655,:691
verify: cargo check -p opencode-rk-cli --features native green 0 errors (457 pre-existing dead-code warnings); rustfmt --check 13 diffs pre-existing (stash test identical count, only line shift)

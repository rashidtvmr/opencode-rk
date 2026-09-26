# BRIDGE-GAP-22 scratchpad

claim: BRIDGE-GAP-22 ses_gap22 worklog/BRIDGE-GAP-22.md
owned: crates/opentui-bridge/src/run_footer.rs (NEW, run_types.rs untouched)
source: crates/opentui-bridge/src/run_stream.rs StreamCommit{id,text}; run_replay.rs cap idiom; TS run/footer.ts read-only ref (not vendored)
api: RunFooter{view,queue:Vec<StreamCommit> cap128,destroyed} new/view/len/is_empty/is_destroyed/append->bool/flush->Vec/event(FooterEventKind Start|Commit|Finish|Destroy)->FooterView/destroy; FooterView Idle|Streaming|Done; FOOTER_CAP=128; flush=mem::take coalesce; destroy latches+clears, idempotent; events frozen after destroy except Destroy
tests: 6 (append cap evict oldest, flush drains, destroy blocks+clears, event switch, destroy event latch+freeze, double-destroy safe)

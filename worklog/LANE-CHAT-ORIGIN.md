# LANE-CHAT-ORIGIN scratchpad
claim: LANE-CHAT-ORIGIN in-progress ses_f423cd0e6ffeUlSuxlJpbTBk5Y
evidence: chat.rs:443-459 reuse_credential mints via decide_lifecycle_authed, no origin compare; :571 fail-closed ok; daemon_client.rs:801 decide_lifecycle_authed reuses any validated descriptor+healthy probe regardless of probed origin
scenario: foreign listener on probed port answers /health 200 + stale/foreign descriptor with loopback http_origin elsewhere could yield bearer sent to probed origin
boundary: own chat.rs only; reuse_credential gains probed-origin param, mismatch yields None
tests: cargo check bins (no test edits per lane)
decisions: exact string compare published.http_origin vs probed origin; normalization out of scope (daemon_origin single constructor)
unknowns: none

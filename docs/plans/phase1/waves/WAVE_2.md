# Wave 2 - Ship one real native + web coding journey

Status: DRAFT. Entry: `P1-G1` independently passed.

## Demonstration that closes this wave
Fresh install -> native setup/home -> provider -> approved fixture edit -> tool result -> final answer -> browser co-client -> exit/restart/resume, including deny/cancel.

## Work packages
- **P1-W2-01 - Wire a real terminal input loop with exact restoration**. An actual PTY/console stays responsive while provider work streams; keyboard, paste, resize and teardown act on the same native application state.
- **P1-W2-02 - Connect native home/session layouts to actual daemon state**. The installed default UI is the actual OpenTUI product, not a string snapshot or legacy chat behind a native label.
- **P1-W2-03 - Use the existing multiline composer end to end**. Keyboard-only users compose, edit, send, stop and recover multiline prompts without corrupting text or losing drafts.
- **P1-W2-04 - Add a real protocol-selected compatible provider transport**. Provider protocol, account and upstream model are explicit; vyce/deepseek-v4-flash reaches 9router unchanged instead of being rejected as a non-OpenAI provider.
- **P1-W2-05 - Converge all turn paths on one daemon-owned coding engine**. A prompt performs a real approved coding operation, returns tool output to the model and persists one coherent conversation regardless of client.
- **P1-W2-06 - Make approvals and cancellation actual shared operations**. Only a trusted human decision can authorize the exact pending operation; both clients observe the same outcome.
- **P1-W2-07 - Serve and authenticate the embedded web application**. The release artifact serves a usable authenticated browser app without a Vite process or a separate web-only backend.
- **P1-W2-08 - Run real streaming coding turns from the web UI**. Browser users can create a session, stream a coding turn, grant/deny a tool, cancel and reopen the persisted result.
- **P1-W2-09 - Prove session durability, replay and workspace isolation**. Restart and multi-client replay restore the same history without duplicate effects or cross-workspace leakage.
- **P1-W2-10 - Accept the first installed TUI plus web golden journey**. Finish a small but complete product before declaring the breadth of isolated features complete.

## Gate criteria
- No mock internal engine, policy, storage or UI. Only external service fixtures are substitutes.
- One heavy validation at a time, nonzero executed counts and exact artifact hashes.
- A text append, headless label or renderer refusal is not a passed coding journey.

## Capacity and sequencing
Freeze shared contracts first; then dispatch only non-conflicting one-file candidates. Reserve integration and independent review capacity. Use one heavy validation token across the wave, not one build per worker. A wave may contain multiple bounded dispatch rounds; wave count is not a concurrency or time estimate. Keep repair capacity rather than filling every slot with new breadth.

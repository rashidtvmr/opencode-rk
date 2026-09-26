# BRIDGE-042 app_host

Claim: startup/shutdown order contract mirrors app.tsx run.
Source: packages/tui/src/app.tsx @ a0d9b6c (NOT 95daf90).
Observed: run L186; renderer acquire L191-213 (exitOnCtrlC false L198, release destroyRenderer L209-212); win32DisableProcessedInput L214; keymap L215-219; plugin dispose finalizer L220-228; audio dispose L229; shutdown Deferred+SIGHUP+once destroy L230-236; prewarm getPalette L241; waitForThemeMode(1000) L242; TimeToFirstDraw L1108; index.tsx:1 re-exports run.
Target: crates/opentui-bridge/src/app_host.rs only. Reuse renderer_lifecycle Lifecycle/RendererConfig, no redefine.
Tests (frozen, unrun - no cargo per lane scope): order matches sequence; shutdown reverse; double-start errs; start-destroy ok; deadline breach; input validate.
Decisions: TuiInput minimal {mouse, theme}; FirstDraw default 1000ms; AppHost wraps Lifecycle one-shot start.
Unknowns: lib.rs wiring owned by integrator (lane owns one file only); cargo test not run.

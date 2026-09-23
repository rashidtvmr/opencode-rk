# BRG-FRAME — caller-driven frame loop + timeline clock

Claim: ses_brg_frame. Owned file ONLY: crates/opentui-bridge/src/frame.rs.
lib.rs untouched (standalone `rustc --test` target, no mod wiring needed).

## Source evidence (upstream, read-only)
- renderer.ts setFrameCallback/removeFrameCallback/clear 4182-4194; requestLive/dropLive
  4196-4233; start/internalStart 4235-4264; suspend 4260-4300; resume 4302+;
  stop/internalStop 4374-4390; destroy 4392-4406.
- startRenderLoop 4641-4660 (reset lastTime/frameCount/fps; feed-idle retry keeps
  _isRunning, defers first loop); loop() 4662-4822: backpressure gate BEFORE
  _frameId bump (skipped frames don't increment), animationRequest drain +
  dropLive each, frameCallbacks awaited in order with per-callback try/catch,
  renderStats.frameCallbackTime, fps window >=1000ms.
- renderNative 4824-4860; collectStatSample 4872-4877 (push + shift past
  maxStatSamples=300, renderer.ts:817); getStats 4884+ (avg/min/max over copy).
- Timeline.ts: Timeline{currentTime,isPlaying,isComplete,duration,loop} 285-292;
  update(delta) 457-496 (clamp+complete when !loop; wrap+overshoot when loop);
  play 402, pause 416, rewind currentTime=0 449; TimelineEngine attach/detach
  via setFrameCallback/removeFrameCallback + requestLive/dropLive accounting.

## Target boundary
- Std-only, no threads/timers: caller drives `step_frame(delta_ms)`.
- `FrameLoop`: Idle/Running/Suspended/Stopped/Destroyed; frame_id u64;
  ordered callbacks with id removal; attached timelines advanced per step;
  backpressure flag -> Skipped (no bump, no callbacks); bounded stats ring 300.
- `Timeline`: scalar clock (duration/current/playing/complete/looping).
  ponytail ceiling: no per-property animation items/easing; add when porting
  TimelineAnimationItem/evaluateItem.
- Markers: FrameLoop, FrameState, start/stop/suspend/resume/step_frame, Timeline.

## Tests (frozen after RED)
t01 start->running; t02 double-start err; t03 suspend/resume preserves frame_id;
t04 stop->stopped (+step errs); t05 step increments + callback order;
t06 callback removal; t07 timeline attach/detach; t08 stats ring cap 300;
t09 backpressure skip.

## Decisions
- step_frame allowed in Idle (one-shot, mirrors activateFrame demand path);
  Err in Suspended/Stopped/Destroyed (explicit failure states).
- Callbacks: FnMut(frame_id, delta_ms), upstream per-callback catch has no Rust
  analogue for panics (no catch_unwind: keep simple, document).
- start from Stopped allowed (mirrors internalStart !destroyed gate).

## Unknowns
- None blocking. fps-window emulation omitted (needs clock; caller owns cadence).

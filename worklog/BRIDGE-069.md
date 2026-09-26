# BRIDGE-069 — renderables.rs additive extension

Claim: extend-only `crates/opentui-bridge/src/renderables.rs`, preserve all existing items.
Evidence (TS checkout a0d9b6c): dialog-select.tsx:542 (flexDirection row), :558 (justifyContent space-between), :543 (backgroundColor/fg), dialog-confirm.tsx:62 (text fg + onMouseUp), prompt/index.tsx:1368-1373 (placeholderColor/textColor/cursorColor/minHeight/maxHeight) + :1375-1393 (callbacks stay TS-side), session/index.tsx:1175-1180 (trackOptions bg/fg) + :1181-1182 (stickyStart bottom), component/bg-pulse.tsx:19 (FrameBufferRenderable subclass).
Added: FlexDir, Justify, FlexLayout + validate_flex_layout; BoxStyle + validate_box_style; TextStyle + validate_text_style; InputStyle + validate_input_style + 5 INPUT_* consts; ScrollTrack + validate_scroll_track; StickyStart + validate_sticky_start; FrameBufferCustom struct + validate_framebuffer_custom; RenderableKind::FrameBufferCustom (unit variant, keeps Copy); 2 new RenderableError variants.
Tests: 6 new (13 total). No cargo run (task constraint). Grep verified all symbols present.
Unknowns: none. `width` prop excluded (dimension type lives elsewhere).

# Box appearance and CLI feedback

## Product contract

Box appearance is an explicitly requested OpenCode-RK extension. It applies to
the chat composer, message cards, tool cards, dialogs, sidebars, menus, and other
boxed surfaces. It must preserve permission prompts, focus, text selection,
content clipping, and readable contrast.

### Roundedness

- Provide a global setting and individual box overrides.
- Provide **Sharp**, **Rounded**, **iOS Squircle**, and **Custom** presets.
- Choosing **iOS Squircle — apply to all boxes** applies the preset globally and
  clears per-box corner overrides. Later individual overrides remain possible.
- Custom roundedness has a draggable control from **0 to 50**, with zero meaning
  sharp. The same control supports keyboard adjustment and explicit endpoints.
- Show the selected value and preview the affected box immediately.
- Persist appearance preferences without replacing unrelated UI/model/provider
  preferences. Restart restores both global settings and individual overrides.

### Terminal rendering

A text terminal paints character cells, not arbitrary pixel curves. The CLI
therefore uses a **logical radius** from 0–50 and maps it to supported corner
glyphs. Rounded and squircle presets are terminal approximations; the UI must
not describe them as exact iOS continuous curves or a physical 50-pixel radius.
The control remains draggable, but adjacent values may produce the same glyphs.
Pixel-rendered clients can implement continuous rounded/squircle contours and
pixel radii while retaining the same preference intent.

Roundedness must not alter the text hit map or make decoration/padding
selectable. Border cells are part of layout geometry, not message content.

### Borders and padding

- Every boxed surface has a deliberate border policy; corner changes must not
  leave disconnected edges or consume text cells without updating layout.
- Maintain inner padding between text and borders, including in the composer.
- Clip or adapt decoration on narrow terminals rather than wrapping a border.
- Distinguish composer focus, message roles, tool errors, and permission prompts
  using actual style/context facts rather than parsing message text.
- Keep border/padding preferences bounded so a configuration cannot allocate
  excessive space or make the input area unusable.

### Loading and execution feedback

- Animate a terminal-friendly loader while actual model discovery, a submitted
  turn, or tool execution is in progress.
- Show truthful lifecycle labels, including waiting for permission, execution,
  completion, cancellation, and failure when those facts are available.
- A loader stops on a terminal event even when the assistant's final text is
  empty, as with a provider-free shell command.
- Redraw at a bounded cadence; an idle client does not spin or repaint
  continuously. Respect reduced-motion preferences when supported.
- A spinner or a task card is not evidence that a process or subagent ran.

## Native controls

Open `/appearance` from the chat composer. **Tab** selects All boxes or an
individual box. Use **1** for Sharp, **2** for Rounded, **3** for iOS Squircle on
all boxes, or **4** for Custom. Drag the roundedness track, use **+ / −** for
single-step adjustment, **0** for sharp, or **5** for the maximum logical radius.
Use **b** to cycle border styles, **p** to cycle padding, and **Escape** to close.
Changes are saved in the application's existing `state/kv.json` under
`box_appearance`, preserving unrelated keys.

## Verification

Meaningful checks cover global and per-box persistence, preset application,
drag/keyboard endpoints, narrow-terminal layout, padding exclusion from copied
text, and loaders driven by real backend lifecycle events. Installed PTY
journeys remain necessary in addition to pure style/model tests.

This document specifies the requested feature. Acceptance requires its complete
native input, preference, layout, and paint paths to pass on the integrated
installed build; the presence of this document is not acceptance.

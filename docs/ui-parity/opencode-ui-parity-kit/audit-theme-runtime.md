# OpenCode v2: theme and terminal-rendering contract

**Target:** `450e7b92412c780a7b5a75226bb8d49aaca24e22`, package 2.1.4, OpenTUI 0.6.8. This audit covers the normal full-screen TUI. Its native default theme is `src/theme/assets/v2/opencode.json`; the older `assets/opencode.json` and `src/mini/theme.ts` are different inputs. Values below are derived from the selected source, not sampled from a screenshot. Tests listed below were inspected; execution results belong to the parent audit's validation record.

## 1. The three surfaces that make the layout readable

The default dark conversation is `#0a0a0a`, sidebars are `#141414`, and the composer is `#1e1e1e`. The composer explicitly calls `theme.decrease(theme.background.raised.base)`. Here **decrease means one earlier hue-scale step**, from neutral 700 to neutral 600. It does not mean subtracting RGB brightness. Light mode reverses the visual direction while preserving the semantic operation. [T01], [T02], [T03]

| Role / expression | Dark RGB / hex | Light RGB / hex |
| --- | --- | --- |
| Conversation/root `background.base`, neutral 800 | `(10,10,10)` `#0a0a0a` | `(255,255,255)` `#ffffff` |
| Sidebar and standard raised blocks `background.raised.base`, neutral 700 | `(20,20,20)` `#141414` | `(250,250,250)` `#fafafa` |
| Composer `decrease(raised.base)`, neutral 600 | `(30,30,30)` `#1e1e1e` | `(245,245,245)` `#f5f5f5` |
| Raised-high `background.raised.high`, neutral 600 | `(30,30,30)` `#1e1e1e` | `(245,245,245)` `#f5f5f5` |
| Raised-max `background.raised.max`, neutral 500 | `(76,76,76)` `#4c4c4c` | `(190,190,190)` `#bebebe` |
| Base text, neutral 200 | `(238,238,238)` `#eeeeee` | `(26,26,26)` `#1a1a1a` |
| Muted text, neutral 400 | `(128,128,128)` `#808080` | `(138,138,138)` `#8a8a8a` |
| Faint text (falls back to muted in this theme) | `(128,128,128)` `#808080` | `(138,138,138)` `#8a8a8a` |
| Border | `(72,72,72)` `#484848` | `(184,184,184)` `#b8b8b8` |
| Sidebar scrollbar | `(96,96,96)` `#606060` | `(160,160,160)` `#a0a0a0` |
| Primary action focused background, interactive 200 | `(250,178,131)` `#fab283` | `(59,125,216)` `#3b7dd8` |
| Primary action focused text | `(10,10,10)` `#0a0a0a` | `(255,255,255)` `#ffffff` |
| Accent 200 | `(157,124,216)` `#9d7cd8` | `(214,140,39)` `#d68c27` |

Primary action base and selected backgrounds are **transparent**, not one of these solid grays. Primary hover background is raised-base; dialog primary hover is raised-high. Per-component code can apply additional blending: selected horizontal tabs explicitly blend toward the composer-level surface, whereas normal expanded vertical tabs read the selected-action token. Do not globally paint every “selected” thing with the same color. Transcript scrollbar track uses decreased raised-base and thumb uses border; sidebar scrollbar track uses raised-base and thumb uses scrollbar-base. [T01], [T04]

## 2. Preserve token provenance in Rust

The TypeScript resolver indexes the **identity** of each RGBA swatch with its hue name and step. References preserve that identity. Hue aliases clone their underlying scale so neutral 600 and gray 600 retain distinct provenance even when their RGB values coincide. `increase/decrease` truncate finite step counts, clamp within 100…900, and leave untracked literal colors unchanged. A freshly constructed RGBA with the same RGB as a token is deliberately untracked. [T03]

This is a concrete Rust bridge risk: a flat map of strings to `[u8;4]` loses the ability to implement the source operation faithfully. Store a resolved color together with optional `{hue,step}` provenance, or use stable color handles. Do not recover provenance by reverse-searching RGB values: default light neutral 800 and 900 are both white, and tests explicitly cover equal-valued adjacent swatches. This representation is a port recommendation; the exact observed behavior is established by upstream resolver/component tests. [T03], [T05]

The action state precedence is **disabled → pressed → focused → selected → hovered → base**, using the first true state. Missing states expand to that variant's base. Thus a pressed+hovered action can resolve to transparent if pressed was not given a separate color; inventing a darker pressed shade changes the theme contract. Text feedback roles (error/warning/success/info) are separate from formfield/action roles. [T06]

Theme documents deep-merge base with the requested mode, expand structural state defaults, then resolve token references. Missing/circular references and incomplete native themes fail validation; they are not silently completed from the default theme. A dialog surface is an absolute view, not a recursive “darken again” operation. Its overridden token references resolve within the dialog context. A dialog action override's base supplies omitted states before the outer action's explicit states, an important precedence rule. Surface wrappers remain reactive and retain stable identity during theme changes. [T03], [T05], [T07]

## 3. Syntax, Markdown, and diff colors are independent contracts

Default code syntax uses these exact values; the appearance is not monochrome plus a generic accent:

| Syntax role | Dark | Light |
| --- | --- | --- |
| Comment | `#808080` | `#8a8a8a` |
| Keyword | `#9d7cd8` | `#d68c27` |
| Function | `#fab283` | `#3b7dd8` |
| Variable | `#e06c75` | `#d1383d` |
| String | `#7fd88f` | `#3d9a57` |
| Number | `#f5a742` | `#d68c27` |
| Type | `#e5c07b` | `#b0851f` |
| Operator | `#56b6c2` | `#318795` |
| Punctuation | `#eeeeee` | `#1a1a1a` |

The syntax generator maps parser scopes to colors **and attributes**: comments italic; several keyword families italic; type definitions bold; type keywords bold+italic. Markdown headings are bold, H1 additionally underlined; strong is bold, emphasis/quotes italic, links underlined. Markdown heading follows accent, link follows interactive, inline/raw code follows green, quote/emphasis use the type-like gold, strong uses the number-like amber. Inline code also paints `background.base`. Checked-list scope uses success; unchecked/strikethrough/debug uses muted. Preserve scope precedence, not only the nine named color values. [T01], [T08]

Composer extmarks have their own syntax rules: files warning+bold; agents categorical[0] step 200+bold; skills categorical[1] (fallback categorical[0])+bold; collapsed paste uses focused primary text on warning background+bold. Default categorical order is blue, purple, green, orange, red in dark and purple, orange, green, blue, red in light. Token generation therefore needs the selected mode's categorical ordering. [T01], [T08]

| Diff role | Dark | Light |
| --- | --- | --- |
| Added text | `#4fd6be` | `#1e725c` |
| Removed text | `#c53b53` | `#c53b53` |
| Context/hunk-header text | `#828bb8` | `#7086b5` |
| Added background | `#20303b` | `#d5e5d5` |
| Removed background | `#37222c` | `#f7d8db` |
| Context background | `#141414` | `#fafafa` |
| Added/removed highlight | `#b8db87` / `#e26a75` | `#4db380` / `#f52a65` |
| Line number text | `#8f8f8f` | `#595959` |
| Added/removed line-number background | `#1b2b34` / `#2d1f26` | `#c5d5c5` / `#e7c8cb` |

Diff styling has distinct content, changed span, gutter, and hunk roles. Reusing success/error backgrounds for all of them loses parity. These tables describe `opencode`; other themes must resolve their own values. [T01]

## 4. `opencode2` and “new designs” are opt-in

`opencode2` is excluded from the global built-in list and offered by TUI theme context only while `experimental["new-designs"] === true`. Enabling the experiment does not automatically choose that theme. The experiment also changes dialog layout and controls independently of the selected theme. Test **both dimensions** rather than treating them as one switch. [T09]

For contrast, opencode2 dark root/raised/composer are `#080808`/`#141414`/`#202020`; light equivalents are `#ffffff`/`#f2f2f2`/`#dddddd`. It has separate base/muted/faint text values, shifted interactive/feedback hue steps, and different dialog hover treatment. Native default opencode values must not be replaced with these merely because the project targets OpenCode v2. [T10]

With new-designs disabled, a dialog's active row uses focused action foreground/background and bold title; a current-but-not-active choice uses selected formfield text and a `●` marker. New designs alters row insets, description treatment, current marker to `▪`, and title boldness; active/current are distinct states. Turning off the experiment while opencode2 is selected renders fallback opencode but retains the configured selection name opencode2 for later re-enable. [T09], [T11]

## 5. Theme loading, switching, and system fallback

Default selection is opencode. Mode comes from explicit dark/light lock, otherwise renderer mode, otherwise startup mode. Theme mode and theme name are distinct settings. One-mode themes render their available mode while preserving the user's requested mode preference; selecting a dual-mode theme later reuses that preference. Unknown names render opencode. An invalid non-default theme emits a theme error, activates opencode, and keeps the application running. [T12]

Source priority is built-ins < discovered custom files < generated system theme. Custom `themes/*.json` files are sorted within each directory; later directory discoveries replace same-named themes. V1 objects containing `theme` are migrated; native documents are parsed and validated. Parse caches key on source-object identity, so a replacement source is revalidated. Theme picker movement previews immediately and calls the same configuration-persisting setter as confirmation. Cancellation restores and persists the initial selection; confirmation keeps the chosen selection. [T13]

For system theme, first paint uses cached usable terminal colors or fallback opencode. Palette queries begin only after first paint, request 16 colors, and avoid concurrent probes. Default foreground/background are preferred, with slots 7/0 as data fallback. Mode inference uses the **actual default background only**, weighted luminance `0.299r+0.587g+0.114b >0.5`; slot 0 is not used to infer mode. System background retains alpha 0, while derived raised grays and diff tints use palette colors. Refresh reacts to renderer theme events and recognized terminal notifications; mode locks ignore automatic mode changes. SIGUSR2 refresh waits 250 ms / 1000 ms and reloads custom themes on the latter attempt. [T12], [T14]

Theme changes update renderer background and recreate syntax style. Old native syntax resources are retained until the renderer becomes idle before destruction. A Rust bridge must avoid freeing a style still referenced by a queued frame. This is an existing lifecycle requirement, not an optional visual enhancement. [T12]

## 6. Renderer, cursor, selection, and Unicode

The full TUI requests target 60 fps, Kitty keyboard handling, configured mouse input (default true), no automatic focus, no automatic Ctrl+C exit, and passthrough external output. The target rate is not a guarantee of 60 updates per second. The selected baseline's tab pulse is a live renderable advanced per frame; v2.2.0's later 20 fps pulse optimization is a separate version delta. [T15]

Cursor styles accept block, underline, line, or default (preserve terminal setting). If a cursor configuration object is supplied but fields are absent, its defaults are block and blinking=true; if the entire configuration is absent, OpenTUI receives undefined. Composer cursor is base text color when enabled and raised-base when disabled. The textarea has transparent focused background so the parent composer surface shows through. Its text/placeholder/cursor are separate properties. [T02], [T16]

Action selection is not terminal text selection. The native v2 theme schema has action/formfield selection states but no dedicated full-TUI text-selection color token; OpenTUI owns that rendering unless a component overrides it. Capture its actual selected-cell colors with the pinned renderer. The app adds copy semantics: select-to-copy defaults on non-Windows, manual on Windows; copying does not clear selection, Escape clears, and Ctrl+C handling precedes application shortcuts. Editor paste placeholders expand when copied. [T17]

Use three explicit coordinate domains in Rust: UTF-8/string indexing, grapheme boundaries, and editor/render offsets. The prompt uses OpenTUI `measureEditText` with `renderer.widthMethod`; it does not treat a character count as a cursor offset. PromptOffsetIndex binary-searches grapheme prefix measurements. General Bun display width uses `Bun.stringWidth`; Node has a grapheme/ANSI-aware fallback that corrects text-presentation emoji width unless East Asian wide, emoji presentation, VS16, keycap, or ZWJ rules apply. Match the selected runtime and terminal width mode. Existing tests combine a form-feed, family ZWJ emoji, Tamil `நி`, a tab, and a Chinese mention to expose wrong offset conversions. [T18]

## 7. Acceptance checklist and test entry points

- [ ] **THEME-01** Resolve every default dark/light token from native JSON; compare RGBA and provenance.
- [ ] **THEME-02** Golden surface colors match the table, including prompt's neutral 700→600 operation.
- [ ] **THEME-03** Equal RGB at different steps remains distinct; literal same-RGB colors do not shift.
- [ ] **THEME-04** Step clamping, negative/fractional/nonfinite amounts match resolver behavior.
- [ ] **THEME-05** All simultaneous state combinations follow disabled→pressed→focused→selected→hovered.
- [ ] **THEME-06** Dialog surface is absolute/reactive and resolves references inside its overrides.
- [ ] **THEME-07** Transparent action states preserve underlying cells; compare alpha as well as RGB.
- [ ] **THEME-08** Syntax scopes, font attributes, Markdown styles, extmarks, and categorical ordering match.
- [ ] **THEME-09** Diff content/gutter/span/hunk colors remain distinct in unified and split presentations.
- [ ] **THEME-10** Default/new-designs × opencode/opencode2 combinations are separately covered.
- [ ] **THEME-11** Missing/invalid/custom/V1/one-mode sources take the exact fallback/migration path.
- [ ] **THEME-12** Picker preview/cancel and locked/system mode changes preserve the intended preferences.
- [ ] **THEME-13** System cold-start/cache/palette-reply transitions do not block first paint; alpha 0 remains alpha 0.
- [ ] **THEME-14** Replacing syntax styles during streaming cannot free a resource before renderer idle.
- [ ] **THEME-15** Cursor shape/blink/default/disabled/focus states match, independently of prompt background.
- [ ] **THEME-16** Selection copy/Escape/Ctrl+C behavior and actual selected-cell painting match native renderer.
- [ ] **THEME-17** CJK, combining accents, Tamil, emoji ZWJ/VS16, tabs and control characters preserve cursor/extmark positions.
- [ ] **THEME-18** Geometry, styles, clipping and pointer hit testing use the same measured cell widths.

Useful original suites, from `packages/tui` after installing the pinned dependencies:

```bash
bun test test/theme.test.ts test/theme/v2/resolve.test.ts test/theme/v2/component.test.ts test/theme/v2/select.test.ts
bun test test/context/theme.test.tsx test/cli/tui/theme-mode.test.tsx test/prompt-offset.test.tsx
```

## Source references

All links target the authoritative commit.

- **T01:** [native default palette, semantic mappings, all hue steps, syntax/Markdown/diff](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/theme/assets/v2/opencode.json#L1-L387).
- **T02:** [composer surface](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/prompt/index.tsx#L1674-L1698), [textarea and cursor](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/prompt/index.tsx#L1767-L1854).
- **T03:** [resolver, surface precedence, provenance, aliases](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/theme/src/tui/resolve.ts#L55-L290).
- **T04:** [transcript scrollbar](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/routes/session/index.tsx#L1527-L1537), [sidebar scrollbar](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/routes/session/sidebar.tsx#L50-L70), [horizontal tab blending](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/session-tabs.tsx#L1560-L1573).
- **T05:** [identity and state regression tests](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/test/theme/v2/component.test.ts#L8-L87), [equal-step-color regression](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/test/theme/v2/resolve.test.ts#L203-L222).
- **T06:** [state/schema ordering](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/theme/src/tui/schema.ts#L3-L25), [state fallback expansion](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/theme/src/tui/expand.ts#L26-L101).
- **T07:** [reactive stable surface wrapper](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/theme/component.ts#L5-L48), [mode selection](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/theme/src/tui/select.ts#L8-L24).
- **T08:** [syntax scope and attribute generation](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/theme/src/tui/syntax.ts#L4-L95).
- **T09:** [experiment gating and fallback test](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/test/context/theme.test.tsx#L12-L67), [experiment registry](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/dialog-experiments.tsx#L17-L28).
- **T10:** [experimental opencode2 palette](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/theme/assets/v2/opencode2.json#L1-L391).
- **T11:** [dialog filter and list insets](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/ui/dialog-select.tsx#L660-L769), [active/current row styling](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/ui/dialog-select.tsx#L883-L953).
- **T12:** [theme provider, palette probing, fallback and native lifecycle](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/context/theme.tsx#L113-L428).
- **T13:** [source priority/parsing](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/theme/index.ts#L20-L88), [discovery](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/theme/discovery.ts#L4-L21), [theme picker preview/cancel](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/dialog-theme-list.tsx#L7-L55).
- **T14:** [terminal mode and generated palette](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/theme/system.ts#L4-L144).
- **T15:** [renderer configuration](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/app.tsx#L249-L290), [baseline pulse driver](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/tab-pulse.tsx#L632-L637).
- **T16:** [cursor schema](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/config/index.tsx#L60-L67), [optional cursor defaults](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/config/index.tsx#L317-L323).
- **T17:** [selection key/copy behavior](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/util/selection.ts#L34-L103), [platform copy defaults and priority](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/app.tsx#L584-L609).
- **T18:** [prompt offset conversions](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/prompt/display.ts#L1-L41), [Node width fallback](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/util/string-width.node.ts#L1-L26), [Bun width implementation](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/util/string-width.bun.ts#L1), [Unicode editor regression](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/test/prompt-offset.test.tsx#L11-L58).

[T01]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/theme/assets/v2/opencode.json#L1-L387
[T02]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/prompt/index.tsx#L1674-L1698
[T03]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/theme/src/tui/resolve.ts#L55-L290
[T04]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/routes/session/index.tsx#L1527-L1537
[T05]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/test/theme/v2/component.test.ts#L8-L87
[T06]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/theme/src/tui/schema.ts#L3-L25
[T07]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/theme/component.ts#L5-L48
[T08]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/theme/src/tui/syntax.ts#L4-L95
[T09]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/test/context/theme.test.tsx#L12-L67
[T10]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/theme/assets/v2/opencode2.json#L1-L391
[T11]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/ui/dialog-select.tsx#L660-L769
[T12]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/context/theme.tsx#L113-L428
[T13]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/theme/index.ts#L20-L88
[T14]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/theme/system.ts#L4-L144
[T15]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/app.tsx#L249-L290
[T16]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/config/index.tsx#L60-L67
[T17]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/util/selection.ts#L34-L103
[T18]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/prompt/display.ts#L1-L41

# Assignment 18 — completion menu geometry/spans/styling/hits/caretanchor

## Role
completion menu implementation designer

## Scope (readonly)
- inspect `/Users/mymac/Projects/opencode-rk-recovery-20261005/crates/cli/src/native_completion.rs`
- inspect `/Users/mymac/Projects/opencode-rk-recovery-20261005/crates/cli/src/tui_entry.rs` (paint + dispatch)
- inspect `/Users/mymac/Projects/opencode-rk-recovery-20261005/crates/cli/src/native_ui_style.rs`
- inspect reference `/Users/mymac/Projects/opencode-rk/.git/recovery-evidence/ui-parity-20261027/26-autocomplete.md`
- inspect upstream `/Users/mymac/Projects/tmp/opencode/packages/tui/src/component/prompt/autocomplete.tsx` @ 450e7b9

## Base
/Users/mymac/Projects/opencode-rk-recovery-20261005 @ current main (commit unknown, canonical tui_entry 9292 lines)

## Findings

### Existing infrastructure (already real)
`native_completion.rs` (487 lines) owns bounded completion state:
- `Item { value, description }` - immutable content
- `Completion { commands, mentions, selected, draft, dismissed }`
- `Panel { x, y, width, rows: Vec<(String, bool)>, command_values: Vec<String> }`

### Current `panel_at` gaps vs upstream 450e7b9
1. **Max rows = 6** vs upstream `Math.min(10, count)`. Cap must become 10.
2. **Header row** (" Commands"/" Mentions") + **footer keyboard hint row** exist. Upstream has NO header/footer rows in the popup body; kind labels are inline per-row ("agent"/"skill"/"reference"). The header+footer rows inflate height and shift anchor.y downward.
3. **Selected marker `›`** prepends `› ` to value. Upstream uses NO marker glyph - selection is conveyed purely via full-row background (`theme.background.action.primary.focused`) + focused text fg (`theme.text.action.primary.focused`).
4. **`command_values` populated only in command mode** (`/`), empty in mention mode. The `Panel.command_at` click-hits return `None` for mention panels. Upstream click-selects mention items. But click-selecting mentions inserts a `@name ` token - preserve `command_values` semantics for `/`-commands only; mention clicks should call `accept_value_at` (already wired in native).
5. **Spacing**: native format `" {} {}  {}"` = marker + space + value + TWO spaces + description. Upstream uses `paddingLeft={1} paddingRight={1}` = 1 cell each side, separator `flexGrow` box. The single trailing space before description matches upstream's row gap.
6. **Width**: native panel takes full composer width (`width` from anchor). Upstream: `width: anchor.width` with `paddingLeft{1} paddingRight{1}` - effectively `anchor.width` full. Matches.

### Paint site
`tui_entry.rs:2338-2355`:
```rust
if let Some(completion) = ctx.completion {
    for (index, (line, selected)) in completion.rows.iter().enumerate() {
        let y = u32::from(completion.y) + index as u32;
        let role = if index == 0 { Role::Heading }
            else if *selected { Role::Selected }
            else if index + 1 == completion.rows.len() { Role::Hint }
            else { Role::Item };
        let style = native_ui_style::style(role);
        let bg = style.background.map(|slot| paint_color(ctx.palette, slot)).unwrap_or(panel);
        renderer.fill_rect(u32::from(completion.x), y, u32::from(completion.width), 1, bg)?;
        renderer.draw_text_styled(
            u32::from(completion.x), y,
            &native_view_layout::fit(line, completion.width),
            paint_color(ctx.palette, style.foreground), Some(bg), style.attributes,
        )?;
    }
}
```

### Style roles
`native_ui_style.rs::style`:
- `Selected`: bg=`Primary`, fg=`Background`, BOLD
- `Item`: fg=`Text`, no bg
- `Heading`: fg=`Primary`, BOLD, no bg
- `Hint`: fg=`TextMuted`, no bg

Upstream theme for selected row: bg=`background.action.primary.focused` = `Slot::Primary` mapped; fg=`text.action.primary.focused` = `Slot::Background` mapped (inverted). The current native `Selected` role already inverts to Primary/Background+BOLD. Matches upstream selected styling (content-only, no marker glyph).

### Caret anchor / cursor insertion
- `composer_region(draft)` returns `(x, y, width, height)` - the composer box.
- `panel_at` anchor = `composer_region(&draft)`. Upstream positions popup `top = anchor.y - height()` (above composer). Native does `y: y - rows.len() as u16`. Matches.
- `complete_key_at` already returns `Option<usize>` = new cursor byte offset, preserving insertion semantics. `accept_value_at` returns cursor position for native composer restore. No change needed.

### Dispatch preservation (DO NOT DUPLICATE)
- Keyboard: `tui_entry.rs:5060-5097` - ArrowUp/Down/Escape/Tab/Enter all routed through completion when `panel_at` returns Some. Tab/Enter use `complete_key_at` with `draft_cursor` (cursor-aware).
- Mouse: `tui_entry.rs:5098-5145` - `command_at` hit test, then `accept_value_at` for `/cd`, `replace_draft_with_cursor` + Enter for executable routes.
- Main owns composer wrap/caret (PER CONTRACT L34-35). We only touch completion module + paint role selection.

## Proposed Rust patch

### File: `crates/cli/src/native_completion.rs`

**Change 1**: `panel_at` - raise capacity from 6 to 10, remove header/footer rows, drop `›` marker.

```rust
// REPLACE existing panel_at body (lines 77-92):
pub fn panel_at(&self, draft: &str, cursor: usize, anchor: (u16, u16, u16, u16)) -> Option<Panel> {
    let (x, y, width, _) = anchor;
    let items = self.visible_at(draft, cursor);
    if items.is_empty() || y < 3 || width < 12 { return None; }
    let capacity = usize::from(y.saturating_sub(2)).min(10);
    let selected = self.selected.min(items.len().saturating_sub(1));
    let start = selected.saturating_sub(capacity - 1);
    let command_mode = self.trigger_at(draft, cursor)?.1 == '/';
    let mut rows = Vec::with_capacity(capacity);
    let mut command_values = Vec::with_capacity(capacity);
    for (index, item) in items.iter().enumerate().skip(start).take(capacity) {
        rows.push((format!("{}  {}", if index == selected { item.value.clone() } else { item.value.clone() }, item.description.trim_end()), index == selected));
        if command_mode { command_values.push(item.value.clone()); }
    }
    Some(Panel { x, y: y - rows.len() as u16, width, rows, command_values })
}
```

Wait - upstream uses no marker and relies on bg. But the `selected` bool is still needed for paint to apply `Role::Selected`. The format string should NOT prepend `›`. But we still want value + description with the upstream spacing: `paddingLeft=1` means 1 cell left, content is `value + separator + description`. We use `value + "  " + description` (2 spaces = flexGrow separator box). For empty descriptions (most commands), native currently emits `value + "  "` then trim - keep that. For selected row, still set `true` so paint applies Selected style.

But we must ALSO emit row content for the keyboard hint. Upstream has NO footer hint row. However native tests assert:
```
#[test]
fn panel_is_above_composer_and_selection_scrolls_on_short_viewport() {
    let panel = c.panel((4, 3, 40, 4)).unwrap();
    assert_eq!(panel.rows.len(), 3);
}
```
That asserts 3 rows for 2 items: header(1) + item(1) + hint(1) = 3. Removing header/footer breaks this test. Per AGENTS.md, implementer must NOT weaken semantic tests. The header/footer are native extensions to upstream. The upstream reference has no header/footer. This is a product parity question - whether to drop them.

Per authority order: upstream is specification. The header (" Commands"/ Mentions") is a native addition. To match upstream exactly, remove both. The test must be updated by its independent test-owner to match new parity contract (mechanical test repair allowed for test owner when upstream supersedes).

**Refined Change 1**: Also expose description field for potential per-kind labels (agencies/skill/reference distinction). Add optional `kind` to `Item` for inline label. But this expands scope beyond minimal. Keep `selected: bool` for paint roles. Remove header+footer rows. Cap at 10.

```rust
pub fn panel_at(&self, draft: &str, cursor: usize, anchor: (u16, u16, u16, u16)) -> Option<Panel> {
    let (x, y, width, _) = anchor;
    let items = self.visible_at(draft, cursor);
    if items.is_empty() || y < 3 || width < 12 { return None; }
    let capacity = usize::from(y.saturating_sub(2)).min(10);
    let selected = self.selected.min(items.len().saturating_sub(1));
    let start = selected.saturating_sub(capacity - 1);
    let command_mode = self.trigger_at(draft, cursor)?.1 == '/';
    let mut rows = Vec::with_capacity(capacity);
    let mut command_values = Vec::with_capacity(capacity);
    for (index, item) in items.iter().enumerate().skip(start).take(capacity) {
        let display = if item.description.is_empty() {
            item.value.clone()
        } else {
            format!("{}  {}", item.value, item.description.trim_end())
        };
        rows.push((display, index == selected));
        if command_mode { command_values.push(item.value.clone()); }
    }
    Some(Panel { x, y: y - rows.len() as u16, width, rows, command_values })
}
```

**Change 2**: `Panel.command_at` - already uses actual rendered rectangle + rows. No geometry change needed. The `row == 0 || row + 1 >= rows.len()` guard was for header/footer; now rows are only items. Fix comment and bounds: still keep first/last inert conceptually but with no footer, last row is an item. Actually `command_values.get(row - 1)` with `row - 1` - since `command_values` is 1:1 with `rows` now (both items only, no header), indexing `row - 1` is WRONG. Must be `row`:

Wait - upstream has no header, no footer. `command_values` parallels `rows` 1:1. `command_at` hit row index 0 = first item. So:
```rust
pub fn command_at(&self, x: u16, y: u16) -> Option<&str> {
    if x < self.x || x >= self.x.saturating_add(self.width) || y < self.y { return None; }
    let row = usize::from(y - self.y);
    self.command_values.get(row).map(String::as_str)
}
```
But `command_values` is empty in mention mode (by design). Need mention click support: return item value regardless of mode. But `command_values` only populated for `/`. For `@` mentions, clicks should accept the first `@name` token. However `accept_value_at` exists for that. The existing dispatch code at `tui_entry.rs:5098-5145` uses `command_at` which only works for `/`-mode. To support mention clicks, we need a parallel `item_at` returning the full value. But changing the dispatch is main's lane per contract (L35). We only propose the completion module change; dispatch stays main's domain.

Actually re-reading contract L35: "Main owns composer wrapping/caret implementation; other assignments must not duplicate it." Completion menu itself is OUR assignment. Dispatch wiring is border. We propose completion module returns enough; dispatch caller uses it. We can extend `Panel` with a generic `value_at(x,y)` returning the row's value string from rows directly, keeping `command_at` for `/`-executable semantics.

**Change 3**: Add `Panel::value_at` for click hit on any item (commands AND mentions), preserving `command_at` for executable route dispatch:

```rust
/// Hit-test any completion row by rendered cell coordinates. Returns the
/// item value (e.g. "/models" or "@src/file.rs") for click activation,
/// mirroring the native `command_at` geometry but without the command-only
/// guard, so mention items stay clickable. Non-command panels never produce
/// an executable route, but the value is usable by `accept_value_at`.
pub fn value_at(&self, x: u16, y: u16) -> Option<&str> {
    if x < self.x || x >= self.x.saturating_add(self.width) || y < self.y { return None; }
    let row = usize::from(y - self.y);
    if row >= self.rows.len() { return None; }
    // rows store "value  description" (or just "value"); extract leading token.
    // We retain exact value via command_values when populated (command mode),
    // otherwise re-derive from the trimmed row prefix up to first space gap.
    if let Some(value) = self.command_values.get(row) {
        return Some(value.as_str());
    }
    let text = &self.rows[row].0;
    let trimmed = text.trim_start();
    let token = trimmed.split("  ").next().unwrap_or(trimmed).trim_end();
    if token.is_empty() { return None; }
    Some(token)
}
```

Hmm - this is fragile parsing. Better: store the value alongside the display row. Change `rows: Vec<(String, bool)>` to `Vec<Row>` where `Row { display: String, selected: bool, value: String }`. But that changes the public struct field type - breaking the paint loop in tui_entry which destructures `(line, selected)`. Per contract, minimal change. The paint loop pattern-matches the tuple. We cannot change `Panel` shape without touching tui_entry paint (main's file).

Alternative: keep `rows: Vec<(String, bool)>`, add separate `values: Vec<String>` field parallel to rows for ALL modes (not just command). Then `command_at` uses `command_values` (executable only), `value_at` uses `values` (any item). And `values` always populated. But tui_entry paint constructs `Panel` via `panel_at` and reads `.rows` - that still works. And `command_at` stays. Add `.values`.

**Change 3 (revised)**: Add `values: Vec<String>` to `Panel`, populated for all items. Keep `command_values` for executable `/`-only semantics (dispatch uses it). `command_at` unchanged. New `value_at` for general click:

```rust
pub struct Panel {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub rows: Vec<(String, bool)>,
    command_values: Vec<String>,
    values: Vec<String>,
}

// in panel_at, after building rows:
Some(Panel { x, y: y - rows.len() as u16, width, rows, command_values, values: items.iter().map(|i| i.value.clone()).collect() })

// new method:
pub fn value_at(&self, x: u16, y: u16) -> Option<&str> {
    if x < self.x || x >= self.x.saturating_add(self.width) || y < self.y { return None; }
    let row = usize::from(y - self.y);
    self.values.get(row).map(String::as_str)
}
```

Wait - but `items` is a `Vec<&Item>` and the `skip(start).take(capacity)` only populates partial. `values` must match the rendered `rows` order exactly. Let me restructure: collect into rows AND values together.

Actually the loop already iterates the visible items in order. We can push value during the same loop:

```rust
let mut rows = Vec::with_capacity(capacity);
let mut command_values = Vec::with_capacity(capacity);
let mut values = Vec::with_capacity(capacity);
for (index, item) in items.iter().enumerate().skip(start).take(capacity) {
    let display = ...;
    rows.push((display, index == selected));
    values.push(item.value.clone());
    if command_mode { command_values.push(item.value.clone()); }
}
```

Then `value_at` uses `values` (parallel to rows), `command_at` uses `command_values` (only `/` entries). This is clean, no tuple struct change. `values` is private, accessed via new public `value_at`.

### Paint site: `tui_entry.rs:2338-2355`

Current paint computes Role per row including Header(0)/Hint(last). With header/footer removed, row 0 is the first item. Update role logic:
```rust
let role
### Paint site role logic (main's file - proposal only)
Current: row 0 = Heading, last row = Hint, selected = Selected, else Item.
With no header/footer rows, the role logic should become:
```rust
let role = if *selected { Role::Selected } else { Role::Item };
```
Hint role is no longer generated by native completion (upstream has no footer hint row). This is a main integration decision; native_ui_style::Role::Hint still exists for other overlays.

### Summary of proposed changes to `native_completion.rs` (canonical file):

1. `panel_at`: capacity `.min(6)` -> `.min(10)`, remove header/footer rows, remove `›` marker prefix, keep `selected: bool` for paint.

2. `Panel` struct: add `values: Vec<String>` field (populated for all items in all modes).

3. New `Panel::value_at(x, y) -> Option<&str>`: hit-test any row, return item value (for mention clicks). Keep existing `command_at` for executable `/`-route dispatch (unchanged).

4. Update test `panel_is_above_composer_and_selection_scrolls_on_short_viewport`: with 2 command items and no header/footer, `panel.rows.len()` becomes 2 (not 3). `rows[1]` -> `rows[1]` still holds the selected item (2 items, index 0 and 1). Adjust assertion: `rows.len()==2`, `rows[0].0.contains("/models")`, `rows[1].0.contains("/mcp")` and `rows[1].1` true.

### Geometry facts preserved
- `panel.x` = composer left edge (`composer_region.0`)
- `panel.y` = `composer_top_y - row_count` (above composer, upstream `top = anchor.y - height`)
- `panel.width` = composer width
- `command_at`/`value_at` use actual rendered `self.x, self.y, self.width` (not guessed offsets) - verified tui_entry.rs:5098-5145 uses `panel.command_at(x,y)` with mouse coords.

### Cursor-insertion semantics preserved
`complete_key_at` returns `Option<usize>` new cursor. `accept_value_at` returns `Some(next)` cursor after inserted token. `replace_draft_with_cursor` in native_composer. No changes to these signatures. The `Panel` does not carry cursor state; cursor logic lives in Completion + composer. Caret anchor = composer region y-coordinate, passed as anchor tuple.

### References
- Upstream autocomplete.tsx lines 829-833 (height calc), 858-956 (popup box/SplitBorder/raised.high/noScrollbar/per-row selected bg), 103-132 (position above anchor x/y/width).
- Canonical native_completion.rs lines 73-93 (panel_at), 215-233 (Panel/command_at), 191-208 (complete_key_at/accept_value_at).
- Canonical tui_entry.rs lines 2338-2355 (paint), 5060-5145 (keyboard+mouse dispatch), 1205 (PaintCtx field).

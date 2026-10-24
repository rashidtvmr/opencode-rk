//! Native appearance editor: real keyboard and pointer inputs, no IO.
use crate::native_box_style::{
    BorderStyle, BoxAppearance, BoxId, BoxOverride, FrameMode, RadiusPreset, radius_from_pointer,
};

/// Hit-time cell spans mirrored from the `rows()` renderer: the component
/// label column (`{marker} {label:<18}` = marker + space + an 18-cell field)
/// and the option column after the two separating spaces. Pointer targets
/// may ONLY use spans those rendered rows actually contain (R3).
const LABEL_SPAN_END: u16 = 20;
const OPTION_COL: u16 = 22;

pub struct AppearanceEditor {
    pub style: BoxAppearance,
    pub open: bool,
    original: BoxAppearance,
    target: usize,
    dragging: bool,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AppearanceOutcome { Ignored, Preview, Apply, Cancel }
impl AppearanceEditor {
    pub fn show(&mut self) { self.original=self.style.clone(); self.open=true; self.target=0; self.dragging=false; }
    pub fn apply(&mut self) -> AppearanceOutcome { if !self.open { return AppearanceOutcome::Ignored; } self.original=self.style.clone(); self.open=false; self.dragging=false; AppearanceOutcome::Apply }
    pub fn cancel(&mut self) -> AppearanceOutcome { if !self.open { return AppearanceOutcome::Ignored; } self.style=self.original.clone(); self.open=false; self.dragging=false; AppearanceOutcome::Cancel }
    fn label(id:BoxId)->&'static str {
        match id {BoxId::ChatComposer=>"Composer",BoxId::User=>"Your messages",BoxId::Assistant=>"AI responses",BoxId::System=>"System notes",BoxId::Tool=>"Tool results",BoxId::Dialog=>"Dialogs",BoxId::Sidebar=>"Sidebar",BoxId::Menu=>"Menus"}
    }
    pub fn new(style: BoxAppearance) -> Self {
        let original = style.clone();
        Self {
            style,
            open: false,
            original,
            target: 0,
            dragging: false,
        }
    }
    fn target(&self) -> Option<BoxId> {
        self.target
            .checked_sub(1)
            .and_then(|index| BoxId::ALL.get(index).copied())
    }
    fn radius(&self) -> u8 {
        self.target()
            .map(|id| self.style.effective(id).radius)
            .unwrap_or(self.style.radius)
    }
    fn set_radius(&mut self, radius: u8) {
        if let Some(id) = self.target() {
            let item = self.style.overrides.entry(id).or_default();
            item.radius = Some(radius);
            item.preset = Some(RadiusPreset::Custom);
        } else {
            self.style.radius = radius;
            self.style.preset = RadiusPreset::Custom;
        }
    }
    fn preset(&mut self, preset: RadiusPreset) {
        if let Some(id) = self.target() {
            self.style.overrides.entry(id).or_default().preset = Some(preset);
        } else {
            self.style.preset = preset;
        }
    }
    pub fn key(&mut self, byte: u8) -> bool {
        !matches!(self.key_outcome(byte), AppearanceOutcome::Ignored)
    }
    pub fn key_outcome(&mut self, byte: u8) -> AppearanceOutcome {
        match byte {
            27 => self.cancel(),
            13 => self.apply(),
            9 => {
                self.target = (self.target + 1) % (BoxId::ALL.len() + 1);
                AppearanceOutcome::Preview
            }
            b'1' => {
                self.preset(RadiusPreset::Sharp);
                AppearanceOutcome::Preview
            }
            b'2' => {
                self.preset(RadiusPreset::Rounded);
                AppearanceOutcome::Preview
            }
            b'3' => {
                self.preset(RadiusPreset::IosSquircle);
                AppearanceOutcome::Preview
            }
            b'4' => {
                self.set_radius(50);
                AppearanceOutcome::Preview
            }
            b'r' => { if let Some(id)=self.target() {self.style.overrides.remove(&id); AppearanceOutcome::Preview} else {AppearanceOutcome::Ignored} }
            b'-' => {
                self.set_radius(self.radius().saturating_sub(1));
                AppearanceOutcome::Preview
            }
            b'+' | b'=' => {
                self.set_radius(self.radius().saturating_add(1).min(50));
                AppearanceOutcome::Preview
            }
            b'0' => {
                self.set_radius(0);
                AppearanceOutcome::Preview
            }
            b'5' => {
                self.set_radius(50);
                AppearanceOutcome::Preview
            }
            b'p' => {
                if let Some(id) = self.target() {
                    let value = (self.style.effective(id).padding + 1) % 5;
                    self.style.overrides.entry(id).or_default().padding = Some(value);
                } else {
                    self.style.padding = (self.style.padding + 1) % 5;
                }
                AppearanceOutcome::Preview
            }
            b'b' => {
                let current = self
                    .target()
                    .map(|id| self.style.effective(id).border_style)
                    .unwrap_or(self.style.border_style);
                let next = match current {
                    BorderStyle::Single => BorderStyle::Double,
                    BorderStyle::Double => BorderStyle::Heavy,
                    BorderStyle::Heavy => BorderStyle::None,
                    BorderStyle::None => BorderStyle::Single,
                };
                if let Some(id) = self.target() {
                    self.style
                        .overrides
                        .entry(id)
                        .or_insert_with(BoxOverride::default)
                        .border_style = Some(next);
                } else {
                    self.style.border_style = next;
                }
                AppearanceOutcome::Preview
            }
            b'o' => {
                let mode = self.target()
                    .and_then(|id| self.style.overrides.get(&id).and_then(|over| over.frame_mode))
                    .unwrap_or(self.style.frame_mode);
                let next = match mode {
                    FrameMode::Auto => FrameMode::Outline,
                    FrameMode::Outline => FrameMode::Native,
                    FrameMode::Native => FrameMode::Auto,
                };
                if let Some(id) = self.target() {
                    self.style.overrides.entry(id).or_default().frame_mode = Some(next);
                } else { self.style.frame_mode = next; }
                AppearanceOutcome::Preview
            }
            b'n' => {
                if let Some(id)=self.target() { self.style.overrides.entry(id).or_default().border_style=Some(BorderStyle::None); }
                else { self.style.border_style=BorderStyle::None; }
                AppearanceOutcome::Preview
            }
            _ => AppearanceOutcome::Ignored,
        }
    }
    pub fn move_target(&mut self, down: bool) {
        let count = BoxId::ALL.len() + 1;
        self.target = if down { (self.target + 1) % count } else { (self.target + count - 1) % count };
    }
    pub fn track_width(content_width: u16) -> u16 {
        content_width.saturating_sub(6).min(40)
    }
    /// Painted cell width of the compact option line rendered at `row`:
    /// compact rows draw ONLY the option string starting at column 0, so a
    /// click past that painted text is inert (R3).
    fn compact_option_cells(&self, content_width: u16, row: u16) -> u16 {
        self.rows(content_width)
            .get(usize::from(row))
            .map(|line| u16::try_from(line.chars().count()).unwrap_or(u16::MAX))
            .unwrap_or(0)
    }
    /// Exact control span in the rendered option text.  Wide rows append
    /// inheritance/effective metadata for readability, but that metadata is
    /// never an actionable control.  Returning a cell count here keeps hit
    /// testing tied to the same rendered strings without making the metadata
    /// clickable.
    fn option_control_span(&self, width: u16, row: u16, option_row: u16) -> Option<(u16, u16)> {
        let rendered = self.rows(width);
        let text = rendered.get(usize::from(row)).map(String::as_str).unwrap_or("");
        let option = if width >= 44 {
            text.get(usize::from(OPTION_COL)..).unwrap_or("")
        } else {
            text
        };
        let control = match option_row {
            0 => "1  Sharp corners",
            1 => "2  Rounded corners",
            2 => "3  Soft corners (cell approximation)",
            4 => "0    1    2    3    4",
            5 => option.split(" (effective").next().unwrap_or(option),
            6 => "Reset this component (r)",
            7 => "n  Borderless",
            _ => "",
        };
        let start = option.find(control)?;
        let start = u16::try_from(start).ok()?;
        let end = start.saturating_add(u16::try_from(control.chars().count()).ok()?);
        Some((start, end))
    }
    /// Press dispatch over the RENDERED dialog (R3).
    ///
    /// Hit targets derive from what `rows(content_width)` actually renders:
    /// title rows are inert, row 1 selects the next target, component rows
    /// cover exactly the rendered label span, and option rows map 1:1 to the
    /// rendered option lines. Rows past `visible_rows` (clipped/offscreen in
    /// the caller's viewport) and the two footer hint rows are INERT — a
    /// click can never act on target or option rows that were not painted.
    /// Returns whether an appearance preference changed; navigation returns
    /// false while still consuming the event.
    pub fn pointer_hit(
        &mut self,
        x: u16,
        y: u16,
        kind: u8,
        content_x: u16,
        content_y: u16,
        content_width: u16,
        visible_rows: u16,
    ) -> bool {
        self.pointer_outcome(x, y, kind, content_x, content_y, content_width, visible_rows)
            .is_some_and(|outcome| outcome != AppearanceOutcome::Ignored)
    }
    pub fn pointer_outcome(
        &mut self,
        x: u16,
        y: u16,
        kind: u8,
        content_x: u16,
        content_y: u16,
        content_width: u16,
        visible_rows: u16,
    ) -> Option<AppearanceOutcome> {
        if kind != 0 {
            return None;
        }
        // Screen-absolute pointer mapped strictly into the painted content
        // rect: presses outside its bounds (left/above/below or past the
        // right edge) are INERT — they can never act on an option row (R3).
        let (col, row) = match (x.checked_sub(content_x), y.checked_sub(content_y)) {
            (Some(col), Some(row)) => (col, row),
            _ => return None,
        };
        if col >= content_width {
            return None; // outside the painted rect horizontally
        }
        // Derive every bound from the rendered output: only rows visibly
        // painted here provide targets, plus the exact footer boundary.
        let rendered = self.rows(content_width);
        // Keep this fail-closed even if the renderer is later reduced to a
        // title-only/empty layout: a hit tester must never turn a malformed
        // row count into an underflowed actionable range.
        if row >= rendered.len() as u16 || row >= visible_rows {
            return None;
        }
        let footer_start = (rendered.len() as u16).saturating_sub(2);
        if row >= footer_start {
            let text = rendered.get(usize::from(row))?;
            let chars: Vec<char> = text.chars().collect();
            let control_at = |label: &str| {
                let needle: Vec<char> = label.chars().collect();
                chars.windows(needle.len()).position(|window| window == needle)
                    .filter(|start| usize::from(col) >= *start && usize::from(col) < *start + needle.len())
            };
            let target = if control_at("Enter Apply").is_some() { "Enter Apply" }
                else if control_at("Esc Cancel").is_some() { "Esc Cancel" }
                else { return Some(AppearanceOutcome::Ignored); };
            return Some(if target == "Enter Apply" { self.apply() } else { self.cancel() });
        }
        match row {
            0 => return Some(AppearanceOutcome::Ignored), // title row consumes the press
            1 => {
                self.move_target(true);
                return Some(AppearanceOutcome::Ignored);
            }
            _ => {}
        }
        let option_row = row - 2; // row >= 2 above, no underflow
        let wide = content_width >= 44;
        if wide && col < LABEL_SPAN_END {
            // One target per rendered component label line (rows 2..=9).
            self.target = usize::from(row - 1);
            return Some(AppearanceOutcome::Ignored);
        }
        if wide && col < OPTION_COL {
            return Some(AppearanceOutcome::Ignored); // separator gutter
        }
        if wide {
            // Wide rows paint the option text at OPTION_COL, but do not paint
            // an actionable control across the rest of the dialog width.  Use
            // the actual rendered row (rather than a fixed control width) so
            // every action row has exactly its visible text span as its hit
            // target; trailing cells remain inert.
            let Some((start, end)) = self.option_control_span(content_width, row, option_row) else {
                return Some(AppearanceOutcome::Ignored);
            };
            let relative = col.saturating_sub(OPTION_COL);
            if relative < start || relative >= end {
                return Some(AppearanceOutcome::Ignored);
            }
        }
        if !wide {
            let Some((start, end)) = self.option_control_span(content_width, row, option_row) else {
                return Some(AppearanceOutcome::Ignored);
            };
            if col < start || col >= end {
            return Some(AppearanceOutcome::Ignored); // only rendered text clickable
            }
        }
        match option_row {
            0 => Some(self.key_outcome(b'1')),
            1 => Some(self.key_outcome(b'2')),
            2 => Some(self.key_outcome(b'3')),
            4 => {
                let relative = col.saturating_sub(if wide { OPTION_COL } else { 0 });
                let value = (relative / 5).min(4) as u8;
                if let Some(id) = self.target() {
                    self.style.overrides.entry(id).or_default().padding = Some(value);
                } else {
                    self.style.padding = value;
                }
                Some(AppearanceOutcome::Preview)
            }
            5 => {
                // The border and full-frame selectors share one displayed
                // row. Dispatch by the actual painted character span rather
                // than interpreting a click on 'o Frame' as 'b Border'.
                let text = rendered.get(usize::from(row)).map(String::as_str).unwrap_or("");
                let option = if wide { text.get(usize::from(OPTION_COL)..).unwrap_or("") } else { text };
                let relative = if wide { col.saturating_sub(OPTION_COL) } else { col };
                let choice = option.find("o  Frame:")
                    .is_some_and(|start| usize::from(relative) >= start);
                Some(self.key_outcome(if choice { b'o' } else { b'b' }))
            }
            6 => {
                if let Some(id) = self.target() {
                    self.style.overrides.remove(&id);
                    Some(AppearanceOutcome::Preview)
                } else {
                    Some(AppearanceOutcome::Ignored)
                }
            }
            7 => Some(self.key_outcome(b'n')),
            // Rendered status/hint option lines (3="Shape:", 7=hint) are
            // inert: they are labels, not controls.
            _ => Some(AppearanceOutcome::Ignored),
        }
    }

    /// Legacy viewport-unbounded dispatch; prefer [`pointer_hit`] so clipped
    /// rows stay inert.
    pub fn pointer_controls(&mut self, x: u16, y: u16, kind: u8, content_x: u16, content_y: u16, content_width: u16) -> bool {
        self.pointer_hit(x, y, kind, content_x, content_y, content_width, u16::MAX)
    }
    /// Continuous setting helper retained for callers using a dedicated track.
    pub fn pointer(&mut self,x:u16,y:u16,kind:u8,content_x:u16,content_y:u16,content_width:u16)->bool {
        let left = content_x.saturating_add(3);
        let row = content_y.saturating_add(4);
        let width = Self::track_width(content_width);
        if kind == 0 {
            self.dragging = y == row && x >= left && x < left.saturating_add(width);
        }
        let changed = self.dragging && matches!(kind, 0 | 1 | 2);
        if changed {
            self.set_radius(radius_from_pointer(width, i32::from(x) - i32::from(left)));
        }
        if kind == 2 {
            self.dragging = false;
        }
        changed
    }
    pub fn rows(&self, width: u16) -> Vec<String> {
        let effective = if let Some(id) = self.target() {
            self.style.effective(id)
        } else {
            let mut global = self.style.clone();
            global.overrides.clear();
            global.effective(BoxId::ChatComposer)
        };
        let radius = self.radius();
        let current=self.target().map(Self::label).unwrap_or("All components");
        let inherited = self.target().is_some_and(|id| !self.style.overrides.contains_key(&id));
        let options=vec![
            "1  Sharp corners".to_owned(),
            "2  Rounded corners".into(),
            "3  Soft corners (cell approximation)".into(),
            format!("Shape: {}",if radius == 0 {"Sharp"} else {"Rounded"}),
            format!("0    1    2    3    4   padding [{}]",effective.padding),
            format!("b  Border: {:?} · o  Frame: {:?} (effective{})",effective.border_style,
                self.target().and_then(|id| self.style.overrides.get(&id).and_then(|over| over.frame_mode))
                    .unwrap_or(self.style.frame_mode),
                if inherited {", inherits"} else {""}),
            "Reset this component (r)".into(),
            "n  Borderless · + / - corner setting · p padding".into(),
        ];
        let mut rows=vec!["Box appearance".into(),format!("> Target: {current} · ↑↓ / Tab choose")];
        if width >= 44 {
            for (index,id) in BoxId::ALL.iter().enumerate() {
                let effective = self.style.effective(*id);
                let inherited = !self.style.overrides.contains_key(id);
                let state = if inherited { " · inherits" } else { " · override" };
                let component_option = match index {
                    0 => format!("1  Sharp corners · effective {:?}{state}", effective.preset),
                    1 => format!("2  Rounded corners · effective {:?}{state}", effective.preset),
                    2 => format!("3  Soft corners (cell approximation) · effective {:?}{state}", effective.preset),
                    3 => format!("Shape: {} · effective {:?}{state}", if effective.radius == 0 { "Sharp" } else { "Rounded" }, effective.preset),
                    4 => format!("0    1    2    3    4   padding [{}] · effective{state}", effective.padding),
                    5 => format!("b  Border: {:?} · o  Frame: {:?} (effective{state})",
                        effective.border_style, self.style.overrides.get(id)
                            .and_then(|over| over.frame_mode).unwrap_or(self.style.frame_mode)),
                    6 => format!("Reset this component (r){state}"),
                    _ => format!("n  Borderless · + / - corner setting · p padding{state}"),
                };
                // The left marker is intentionally compact and stays inside
                // the painted label span: `*` means this component has an
                // override, `~` means it inherits global values.  The option
                // text remains the selected target's control, while the
                // component's effective value is reported after that control.
                let marker = if self.target == index + 1 { ">" } else if inherited { "~" } else { "*" };
                rows.push(format!("{} {:<18}  {}", marker, Self::label(*id), component_option));
            }
        } else {
            rows.extend(options);
        }
        rows.push("Enter Apply   Esc Cancel".into());
        rows.push("Preview only: changes are not stored until Apply · ~ inherits · * override".into());
        rows
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usability_open_starts_at_global_target_and_renders_apply_cancel() {
        let mut editor = AppearanceEditor::new(BoxAppearance::default());
        editor.show();
        let rows = editor.rows(30);

        assert!(
            rows.iter().any(|row| row.contains("Target: All components")),
            "opening appearance must visibly select the global target: {rows:?}"
        );
        assert!(
            rows.iter().any(|row| row.contains("Apply")),
            "rendered appearance controls need an explicit Apply action: {rows:?}"
        );
        assert!(
            rows.iter().any(|row| row.contains("Cancel")),
            "rendered appearance controls need an explicit Cancel action: {rows:?}"
        );
    }

    #[test]
    fn usability_rendered_controls_expose_borderless_effective_and_inherited_state() {
        let mut style = BoxAppearance::default();
        style.preset = RadiusPreset::Rounded;
        style.overrides.insert(
            BoxId::User,
            BoxOverride {
                preset: Some(RadiusPreset::Sharp),
                ..BoxOverride::default()
            },
        );
        let editor = AppearanceEditor::new(style);
        let rows = editor.rows(60);

        assert!(
            rows.iter().any(|row| row.contains("Borderless")),
            "borderless must be a visible user-facing choice: {rows:?}"
        );
        assert!(
            rows.iter().any(|row| row.contains("effective")),
            "appearance rows must identify the effective value being previewed: {rows:?}"
        );
        assert!(
            rows.iter().any(|row| row.contains("inherits")),
            "components without overrides must visibly indicate inheritance: {rows:?}"
        );
        assert!(
            rows.iter().any(|row| row.contains("Sharp")),
            "rendered rows must expose the configured/effective shape value: {rows:?}"
        );
    }

    #[test]
    fn usability_global_reset_is_a_noop_and_reports_no_change() {
        let mut editor = AppearanceEditor::new(BoxAppearance::default());
        let rows = editor.rows(30);
        let reset_row = rows
            .iter()
            .position(|row| row.contains("Reset"))
            .expect("rendered editor must expose reset control");
        let reset_col = rows[reset_row]
            .find("Reset")
            .expect("reset control must have a rendered start");
        let before = editor.style.clone();

        assert!(
            !editor.pointer_hit(
                u16::try_from(reset_col).unwrap(),
                u16::try_from(reset_row).unwrap(),
                0,
                0,
                0,
                30,
                u16::MAX,
            ),
            "resetting the global target must report no component reset"
        );
        assert_eq!(
            editor.style, before,
            "global reset must not mutate the global style in the component-reset action"
        );
    }

    #[test]
    fn usability_global_target_is_visible_in_compact_and_wide_renderings() {
        let mut editor = AppearanceEditor::new(BoxAppearance::default());
        editor.show();

        for width in [30, 60] {
            let rows = editor.rows(width);
            assert_eq!(
                rows.get(1).map(String::as_str),
                Some("> Target: All components · ↑↓ / Tab choose"),
                "global target must be explicit at every supported editor width: {rows:?}"
            );
        }
    }

    #[test]
    fn usability_effective_value_follows_selected_component_after_rendered_target_hit() {
        let mut style = BoxAppearance::default();
        style.preset = RadiusPreset::Rounded;
        style.overrides.insert(
            BoxId::User,
            BoxOverride {
                preset: Some(RadiusPreset::Sharp),
                ..BoxOverride::default()
            },
        );
        let mut editor = AppearanceEditor::new(style);
        editor.show();
        let rows = editor.rows(60);
        let user_row = rows
            .iter()
            .position(|row| row.contains("Your messages"))
            .expect("user component must be rendered");
        let user_col = rows[user_row]
            .find("Your messages")
            .expect("user target must have a rendered label");

        assert!(!editor.pointer_hit(
            u16::try_from(user_col).unwrap(),
            u16::try_from(user_row).unwrap(),
            0,
            0,
            0,
            60,
            u16::MAX,
        ), "label click selects a target but does not itself mutate preferences");
        let selected = editor.rows(60);
        assert_eq!(
            selected[1],
            "> Target: Your messages · ↑↓ / Tab choose",
            "rendered target indicator must follow the clicked component label: {selected:?}"
        );
        assert!(
            selected[user_row].contains("Sharp"),
            "selected component row must show its effective configured value: {selected:?}"
        );
        assert!(
            selected[user_row].contains("inherited") || selected[user_row].contains("override"),
            "selected component row must identify why its effective value differs: {selected:?}"
        );
    }

    #[test]
    fn usability_footer_pointer_actions_are_distinct_and_trailing_hint_is_inert() {
        let mut cancel = AppearanceEditor::new(BoxAppearance::default());
        cancel.show();
        assert_eq!(cancel.key_outcome(b'2'), AppearanceOutcome::Preview);
        let cancel_rows = cancel.rows(30);
        let cancel_row = cancel_rows.iter().position(|row| row.contains("Esc Cancel")).expect("Cancel footer");
        let cancel_col = cancel_rows[cancel_row].find("Esc Cancel").unwrap();
        assert!(cancel.pointer_hit(cancel_col as u16, cancel_row as u16, 0, 0, 0, 30, u16::MAX));
        assert_eq!(cancel.style, BoxAppearance::default(), "Cancel restores the draft snapshot");
        assert!(!cancel.open, "Cancel closes the editor");

        let mut cancel_last = AppearanceEditor::new(BoxAppearance::default());
        cancel_last.show();
        assert_eq!(cancel_last.key_outcome(b'2'), AppearanceOutcome::Preview);
        let rows = cancel_last.rows(30);
        let row = rows.iter().position(|line| line.contains("Esc Cancel")).unwrap();
        let start = rows[row].find("Esc Cancel").unwrap();
        let last = start + "Esc Cancel".chars().count() - 1;
        assert!(cancel_last.pointer_hit(last as u16, row as u16, 0, 0, 0, 30, u16::MAX));
        assert_eq!(cancel_last.style, BoxAppearance::default(), "last Cancel cell rolls back");

        let mut apply = AppearanceEditor::new(BoxAppearance::default());
        apply.show();
        assert_eq!(apply.key_outcome(b'2'), AppearanceOutcome::Preview);
        let apply_rows = apply.rows(30);
        let apply_row = apply_rows.iter().position(|row| row.contains("Enter Apply")).expect("Apply footer");
        let apply_start = apply_rows[apply_row].find("Enter Apply").unwrap();
        assert!(!apply.pointer_hit((apply_start + "Enter Apply".len()) as u16, apply_row as u16, 0, 0, 0, 30, u16::MAX));
        assert!(apply.open, "trailing preview hint remains inert");
        assert!(apply.pointer_hit(apply_start as u16, apply_row as u16, 0, 0, 0, 30, u16::MAX));
        assert!(!apply.open, "Apply closes after clicking the first cell");
        assert_eq!(apply.style.preset, RadiusPreset::Rounded, "Apply commits the live preview");
    }
    #[test]
    fn arrows_change_highlighted_target_without_changing_preferences() {
        let mut editor = AppearanceEditor::new(BoxAppearance::default());
        let original = editor.style.clone();
        editor.move_target(true);
        assert_eq!(editor.target, 1);
        assert_eq!(editor.style, original);
        assert!(editor.rows(60)[1].starts_with("> Target:"));
        editor.move_target(false);
        assert_eq!(editor.target, 0);
        assert_eq!(editor.style, original);
    }
    #[test]
    fn soft_corners_preserve_other_components_and_global_preferences() {
        let mut editor = AppearanceEditor::new(BoxAppearance::default());
        editor.target=2; // User
        editor.key(b'+');
        editor.key(b'p');
        let user=editor.style.overrides.get(&BoxId::User).unwrap().clone();
        editor.target=4; // Assistant
        editor.key(b'3');
        assert_eq!(editor.style.overrides.get(&BoxId::User),Some(&user));
        assert_eq!(editor.style.overrides.get(&BoxId::Assistant).unwrap().preset,Some(RadiusPreset::IosSquircle));
        assert_eq!(editor.style.preset,RadiusPreset::Sharp);
        assert_eq!(editor.style.radius,0);
        editor.key(b'2');
        assert_eq!(editor.style.overrides.get(&BoxId::User),Some(&user));
    }
    #[test]
    fn actual_pointer_track_reaches_both_endpoints_and_excludes_padding() {
        let mut editor = AppearanceEditor::new(BoxAppearance::default());
        assert!(!editor.pointer(2, 14, 0, 10, 10, 50));
        assert!(editor.pointer(13, 14, 0, 10, 10, 50));
        assert_eq!(editor.style.radius, 0);
        assert!(editor.pointer(500, 14, 1, 10, 10, 50));
        assert_eq!(editor.style.radius, 50);
        assert!(editor.pointer(52, 14, 2, 10, 10, 50));
        assert_eq!(editor.style.radius, 50);
        assert!(!editor.pointer(13, 14, 1, 10, 10, 50));
    }

    /// R3: hit targets equal the RENDERED rows. Spans mirror the actual
    /// rendered label/option columns; title row, status/hint option lines
    /// and both footer rows consume the press without changing anything.
    #[test]
    fn hits_match_rendered_rows_titles_status_and_footers_inert() {
        let mut editor = AppearanceEditor::new(BoxAppearance::default());
        let rendered = editor.rows(60);
        assert_eq!(
            rendered.len(),
            12,
            "two headers + eight option rows + two footer rows"
        );
        // Title row (rendered index 0): consumes the press, changes nothing.
        assert!(!editor.pointer_hit(4, 2, 0, 2, 2, 60, u16::MAX));
        assert_eq!(editor.target, 0);
        // The rendered label span of row 2 selects the Composer target only
        // where that label is painted; marker/gutter columns differ.
        assert!(!editor.pointer_hit(2 + 20, 4, 0, 2, 2, 60, u16::MAX), "label/gutter gap inert");
        assert!(!editor.pointer_hit(2 + 21, 4, 0, 2, 2, 60, u16::MAX), "gutter inert");
        assert!(!editor.pointer_hit(2 + LABEL_SPAN_END - 1, 4, 0, 2, 2, 60, u16::MAX));
        assert_eq!(editor.target, 1, "last label cell of the first rendered component row selects it");
        assert_eq!(
            rendered[2].contains("Composer"),
            true,
            "selected row is the one actually rendered"
        );
        // Option rows act over the RENDERED option text: row 3 = "2  Rounded
        // corners" and the highlighted target is Composer (per-box override).
        assert!(editor.pointer_hit(2 + OPTION_COL + 4, 5, 0, 2, 2, 60, u16::MAX));
        assert_eq!(
            editor
                .style
                .overrides
                .get(&BoxId::ChatComposer)
                .and_then(|o| o.preset),
            Some(crate::native_box_style::RadiusPreset::Rounded),
            "click on the rendered '2 Rounded' line applies it to the highlighted component"
        );
        assert_eq!(
            editor.style.preset,
            crate::native_box_style::RadiusPreset::Sharp,
            "global preference untouched by a component-scoped click"
        );
        // Status and non-control footer cells remain inert.
        let keep = editor.style.clone();
        let keep_target = editor.target;
        assert!(!editor.pointer_hit(2 + OPTION_COL + 2, 7, 0, 2, 2, 60, u16::MAX), "Shape status row inert");
        assert!(!editor.pointer_hit(2 + OPTION_COL + 2, 12, 0, 2, 2, 60, u16::MAX), "footer row inert");
        assert!(!editor.pointer_hit(2 + OPTION_COL + 2, 13, 0, 2, 2, 60, u16::MAX), "footer row inert");
        assert_eq!(editor.style, keep);
        assert_eq!(editor.target, keep_target);
        let borderless_row = rendered.iter().position(|row| row.contains("n  Borderless")).unwrap();
        let borderless_col = rendered[borderless_row].find("n  Borderless").unwrap();
        let borderless_y = 2 + borderless_row as u16;
        assert!(editor.pointer_hit((2 + borderless_col) as u16, borderless_y, 0, 2, 2, 60, u16::MAX));
        assert_eq!(editor.style.effective(BoxId::ChatComposer).border_style, BorderStyle::None);
        assert_eq!(editor.style.border_style, keep.border_style, "global border remains unchanged");
        assert_eq!(editor.target, keep_target);
        let after_borderless = editor.style.clone();
        let hint_col = borderless_col + "n  Borderless".len();
        assert!(!editor.pointer_hit((2 + hint_col) as u16, borderless_y, 0, 2, 2, 60, u16::MAX), "borderless hint suffix inert");
        assert_eq!(editor.style, after_borderless);
        assert_eq!(editor.target, keep_target);
    }

    /// R3: rows the caller did not paint (clipped dialog) must stay inert even
    /// though their content would map to an option; prefs stay untouched.
    #[test]
    fn clipped_rows_are_inert_across_wide_and_compact_layouts() {
        let mut editor = AppearanceEditor::new(BoxAppearance::default());
        let keep = editor.style.clone();
        // Footer clip: visible_rows 5 paints rows 0..=4 only; any click with
        // row >= 5 is inert (option rows 2..=4 remain actionable below).
        assert!(!editor.pointer_hit(2 + OPTION_COL + 2, 7, 0, 2, 2, 60, 5));
        assert_eq!(editor.style, keep, "clipped option row must not act");
        // Visible rows still act: row 3, "Rounded".
        assert!(editor.pointer_hit(2 + OPTION_COL + 4, 5, 0, 2, 2, 60, 5));
        let keep = editor.style.clone();
        // Compact layout: only the rendered option TEXT is clickable.
        let mut compact = AppearanceEditor::new(BoxAppearance::default());
        compact.show();
        let rendered = compact.rows(30);
        assert_eq!(rendered.len(), 12);
        assert!(compact.pointer_hit(2, 4, 0, 2, 2, 30, u16::MAX), "compact option row acts at col 0");
        assert_eq!(
            compact.style.preset,
            crate::native_box_style::RadiusPreset::Sharp,
            "first compact option is the rendered '1 Sharp corners' line"
        );
        assert!(!compact.pointer_hit(22, 4, 0, 2, 2, 30, u16::MAX), "past rendered text in compact inert");
        let compact_before = compact.style.clone();
        // Reset option rows[6] renders on content row 8 (y=10).
        assert!(!compact.pointer_hit(19, 10, 0, 2, 2, 30, u16::MAX), "global reset is a no-op");
        assert_eq!(
            compact.style,
            compact_before,
            "no target means no override was added by a compact reset press"
        );
    }

    /// Regression control: the same actionable Rounded hit changes style when
    /// its row is visible, but is inert when that exact row is clipped.
    #[test]
    fn clipped_rounded_option_is_inert_but_visible_baseline_mutates() {
        let before = BoxAppearance::default();
        let mut clipped = AppearanceEditor::new(before.clone());
        let mut visible = AppearanceEditor::new(before.clone());
        clipped.show();
        visible.show();
        clipped.move_target(true); // Composer
        visible.move_target(true); // Composer
        let (x, y, content_x, content_y, width) =
            (2 + OPTION_COL + 4, 5, 2, 2, 60);

        assert!(
            !clipped.pointer_hit(x, y, 0, content_x, content_y, width, 3),
            "row 3 is clipped when only rows 0..=2 are visible"
        );
        assert_eq!(clipped.style, before, "clipped hit leaves style unchanged");

        assert!(
            visible.pointer_hit(x, y, 0, content_x, content_y, width, 4),
            "same row 3 hit acts when rows 0..=3 are visible"
        );
        assert_ne!(visible.style, before, "visible baseline must mutate style");
        assert_eq!(
            visible
                .style
                .overrides
                .get(&BoxId::ChatComposer)
                .and_then(|item| item.preset),
            Some(RadiusPreset::Rounded)
        );
    }

    /// Actual-caller boundary: tui_entry passes SCREEN-absolute presses plus
    /// the painted height, so only presses CONTAINED in the painted content
    /// rect act. Outside left/right/above/below, zero width and cropped
    /// height stay inert without panicking or changing any preference.
    #[test]
    fn screen_absolute_presses_find_painted_compact_cell_without_panic() {
        let mut compact = AppearanceEditor::new(BoxAppearance::default());
        assert!(
            compact.pointer_hit(2, 4, 0, 2, 2, 30, u16::MAX),
            "first painted compact option cell (col 0) acts"
        );
        assert_eq!(
            compact.style.preset,
            crate::native_box_style::RadiusPreset::Sharp
        );
        let keep = compact.style.clone();
        // Outside left (x < content_x): inert.
        assert!(!compact.pointer_hit(1, 5, 0, 2, 2, 30, u16::MAX), "left of rect inert");
        // Outside above (y < content_y): inert.
        assert!(!compact.pointer_hit(10, 1, 0, 2, 2, 30, u16::MAX), "above rect inert");
        // Outside below cropped height (actual caller passes painted rows).
        assert!(
            !compact.pointer_hit(10, 12, 0, 2, 2, 30, 10),
            "row past painted height inert"
        );
        // Zero content width paints nothing: every press inert.
        assert!(!compact.pointer_hit(4, 5, 0, 2, 2, 0, u16::MAX), "zero width inert");
        // Right edge containment: col == content_width is outside.
        assert!(
            !compact.pointer_hit(32, 5, 0, 2, 2, 30, u16::MAX),
            "past right edge inert"
        );
        assert_eq!(compact.style, keep, "outside presses change no preference");
        // Wide layout regressed for unbounded columns: col == content_width
        // must not reach the wide option dispatch either.
        let mut wide = AppearanceEditor::new(BoxAppearance::default());
        assert!(
            !wide.pointer_hit(62, 5, 0, 2, 2, 60, u16::MAX),
            "unbounded wide column is outside the painted rect"
        );
        assert_eq!(wide.style, BoxAppearance::default(), "wide right edge inert");
    }

    /// Component preferences stay isolated across pointer navigation (the
    /// mouse never edits other components' overrides implicitly).
    #[test]
    fn mouse_navigation_preserves_per_component_preferences() {
        let mut editor = AppearanceEditor::new(BoxAppearance::default());
        editor.target = 2; // User
        editor.key(b'+'); // ...sets Custom radius 1 on User
        let user = editor.style.overrides.get(&BoxId::User).cloned();
        assert!(user.is_some());
        // Pointer navigation to another component must not mutate User.
        editor.pointer_hit(4, 7, 0, 2, 2, 60, u16::MAX); // row 5 → target 4
        assert_eq!(
            editor.style.overrides.get(&BoxId::User),
            user.as_ref(),
            "pointer selection preserves other components' prefs"
        );
        // Reset row 8 (options[6]) removes ONLY the highlight component.
        assert!(editor.pointer_hit(2 + OPTION_COL + 2, 10, 0, 2, 2, 60, u16::MAX));
        assert!(
            !editor.style.overrides.contains_key(&BoxId::Assistant),
            "reset applies to the highlighted component only"
        );
        assert_eq!(
            editor.style.overrides.get(&BoxId::User),
            user.as_ref(),
            "User override untouched"
        );
    }

    /// Wide rows must expose only the painted reset label as a reset target.
    /// In particular, the blank cells to its right are still inside the dialog
    /// rect but are not part of the rendered control.
    #[test]
    fn wide_reset_hit_uses_exact_rendered_option_span() {
        const CONTENT_X: u16 = 3;
        const CONTENT_Y: u16 = 4;
        const WIDTH: u16 = 80;
        const RESET: &str = "Reset this component (r)";

        let configured = || {
            let mut editor = AppearanceEditor::new(BoxAppearance::default());
            editor.show();
            editor.move_target(true); // Composer is the selected target.
            editor.key(b'2');
            editor
        };

        let mut probe = configured();
        let rows = probe.rows(WIDTH);
        let reset_row = rows
            .iter()
            .position(|row| row.contains(RESET))
            .expect("wide rendering contains the reset label");
        let reset_start = rows[reset_row]
            .find(RESET)
            .expect("reset label has a rendered start");
        let reset_end = reset_start + RESET.chars().count();
        assert_eq!(reset_start, usize::from(OPTION_COL));
        assert!(reset_end <= rows[reset_row].chars().count());

        // The first and last painted cells of the reset label are controls.
        let first_col = u16::try_from(reset_start).unwrap();
        let last_col = u16::try_from(reset_end - 1).unwrap();
        assert!(probe.pointer_hit(
            CONTENT_X + first_col,
            CONTENT_Y + u16::try_from(reset_row).unwrap(),
            0,
            CONTENT_X,
            CONTENT_Y,
            WIDTH,
            u16::MAX,
        ));
        assert!(probe.style.overrides.is_empty());

        let mut last = configured();
        assert!(last.pointer_hit(
            CONTENT_X + last_col,
            CONTENT_Y + u16::try_from(reset_row).unwrap(),
            0,
            CONTENT_X,
            CONTENT_Y,
            WIDTH,
            u16::MAX,
        ));
        assert!(last.style.overrides.is_empty());

        // Reconstruct the exact pre-reset state and prove the cell immediately
        // after the painted label is inert, despite being inside the wide rect.
        let mut blank = configured();
        let before_blank = blank.style.clone();
        assert!(!blank.pointer_hit(
            CONTENT_X + u16::try_from(reset_end).unwrap(),
            CONTENT_Y + u16::try_from(reset_row).unwrap(),
            0,
            CONTENT_X,
            CONTENT_Y,
            WIDTH,
            u16::MAX,
        ));
        assert_eq!(blank.style, before_blank);

        // The cell immediately before the rendered option is the separator
        // gutter and must not become a reset hit either.
        let mut before = configured();
        assert!(!before.pointer_hit(
            CONTENT_X + first_col - 1,
            CONTENT_Y + u16::try_from(reset_row).unwrap(),
            0,
            CONTENT_X,
            CONTENT_Y,
            WIDTH,
            u16::MAX,
        ));
        assert_eq!(before.style, before_blank);
    }
}

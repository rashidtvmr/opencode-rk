//! Native appearance editor: real keyboard and pointer inputs, no IO.
use crate::native_box_style::{
    BorderStyle, BoxAppearance, BoxId, BoxOverride, RadiusPreset, radius_from_pointer,
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
    target: usize,
    dragging: bool,
}
impl AppearanceEditor {
    pub fn show(&mut self) { self.open=true; self.target=1; self.dragging=false; }
    fn label(id:BoxId)->&'static str {
        match id {BoxId::ChatComposer=>"Composer",BoxId::User=>"Your messages",BoxId::Assistant=>"AI responses",BoxId::System=>"System notes",BoxId::Tool=>"Tool results",BoxId::Dialog=>"Dialogs",BoxId::Sidebar=>"Sidebar",BoxId::Menu=>"Menus"}
    }
    pub fn new(style: BoxAppearance) -> Self {
        Self {
            style,
            open: false,
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
        match byte {
            27 => {
                self.open = false;
                self.dragging = false;
                false
            }
            9 => {
                self.target = (self.target + 1) % (BoxId::ALL.len() + 1);
                false
            }
            b'1' => {
                self.preset(RadiusPreset::Sharp);
                true
            }
            b'2' => {
                self.preset(RadiusPreset::Rounded);
                true
            }
            b'3' => {
                self.preset(RadiusPreset::IosSquircle);
                true
            }
            b'4' => {
                self.set_radius(50);
                true
            }
            b'r' => { if let Some(id)=self.target() {self.style.overrides.remove(&id);} true }
            b'-' => {
                self.set_radius(self.radius().saturating_sub(1));
                true
            }
            b'+' | b'=' => {
                self.set_radius(self.radius().saturating_add(1).min(50));
                true
            }
            b'0' => {
                self.set_radius(0);
                true
            }
            b'5' => {
                self.set_radius(50);
                true
            }
            b'p' => {
                if let Some(id) = self.target() {
                    let value = (self.style.effective(id).padding + 1) % 5;
                    self.style.overrides.entry(id).or_default().padding = Some(value);
                } else {
                    self.style.padding = (self.style.padding + 1) % 5;
                }
                true
            }
            b'b' => {
                let current = self
                    .target()
                    .map(|id| self.style.effective(id).border_style)
                    .unwrap_or(self.style.border_style);
                let next = match current {
                    BorderStyle::Single => BorderStyle::Double,
                    BorderStyle::Double => BorderStyle::Heavy,
                    BorderStyle::Heavy => BorderStyle::Single,
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
                true
            }
            _ => false,
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
        if kind != 0 {
            return false;
        }
        // Screen-absolute pointer mapped strictly into the painted content
        // rect: presses outside its bounds (left/above/below or past the
        // right edge) are INERT — they can never act on an option row (R3).
        let (col, row) = match (x.checked_sub(content_x), y.checked_sub(content_y)) {
            (Some(col), Some(row)) => (col, row),
            _ => return false,
        };
        if col >= content_width {
            return false; // outside the painted rect horizontally
        }
        // Derive every bound from the rendered output: only rows visibly
        // painted here provide targets, plus the exact footer boundary.
        let rendered = self.rows(content_width);
        // Keep this fail-closed even if the renderer is later reduced to a
        // title-only/empty layout: a hit tester must never turn a malformed
        // row count into an underflowed actionable range.
        let content_rows = (rendered.len() as u16).saturating_sub(2); // trailing hint rows are inert
        if row >= content_rows || row >= visible_rows {
            return false;
        }
        match row {
            0 => return false, // title row consumes the press, changes nothing
            1 => {
                self.move_target(true);
                return false;
            }
            _ => {}
        }
        let option_row = row - 2; // row >= 2 above, no underflow
        let wide = content_width >= 44;
        if wide && col < LABEL_SPAN_END {
            // One target per rendered component label line (rows 2..=9).
            self.target = usize::from(row - 1);
            return false;
        }
        if wide && col < OPTION_COL {
            return false; // separator gutter between label and option columns
        }
        if !wide && col >= self.compact_option_cells(content_width, row) {
            return false; // only the RENDERED compact option text is clickable
        }
        match option_row {
            0 => self.key(b'1'),
            1 => self.key(b'2'),
            2 => self.key(b'3'),
            4 => {
                let relative = col.saturating_sub(if wide { OPTION_COL } else { 0 });
                let value = (relative / 5).min(4) as u8;
                if let Some(id) = self.target() {
                    self.style.overrides.entry(id).or_default().padding = Some(value);
                } else {
                    self.style.padding = value;
                }
                true
            }
            5 => self.key(b'b'),
            6 => {
                if let Some(id) = self.target() {
                    self.style.overrides.remove(&id);
                }
                true
            }
            // Rendered status/hint option lines (3="Shape:", 7=hint) are
            // inert: they are labels, not controls.
            _ => false,
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
        let options=vec![
            "1  Sharp corners".to_owned(),
            "2  Rounded corners".into(),
            "3  Soft corners (cell approximation)".into(),
            format!("Shape: {}",if radius == 0 {"Sharp"} else {"Rounded"}),
            format!("0    1    2    3    4   padding [{}]",effective.padding),
            format!("b  Border weight: {:?}",effective.border_style),
            "Reset this component (r)".into(),
            "+ / - corner setting · p padding".into(),
        ];
        let mut rows=vec!["Box appearance".into(),format!("> Target: {current} · ↑↓ / Tab choose")];
        if width >= 44 {
            for (index,id) in BoxId::ALL.iter().enumerate() {
                rows.push(format!("{} {:<18}  {}",if self.target==index+1 {">"} else {" "},Self::label(*id),options[index]));
            }
        } else {
            rows.extend(options);
        }
        rows.push("Click a component or setting · Esc close".into());
        rows.push("Corners preserve each component's own surface.".into());
        rows
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        // Status line (row 5 = options[3] "Shape: …"), rendered help row 9
        // and both footer hint rows (10, 11) are labels, not controls.
        let keep = editor.style.clone();
        let keep_target = editor.target;
        assert!(!editor.pointer_hit(2 + OPTION_COL + 2, 7, 0, 2, 2, 60, u16::MAX), "Shape status row inert");
        assert!(!editor.pointer_hit(2 + OPTION_COL + 2, 11, 0, 2, 2, 60, u16::MAX), "help option row inert");
        assert!(!editor.pointer_hit(2 + OPTION_COL + 2, 12, 0, 2, 2, 60, u16::MAX), "footer row inert");
        assert!(!editor.pointer_hit(2 + OPTION_COL + 2, 13, 0, 2, 2, 60, u16::MAX), "footer row inert");
        assert_eq!(editor.style, keep);
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
        assert!(
            compact.pointer_hit(19, 10, 0, 2, 2, 30, u16::MAX),
            "reset line still clickable inside its rendered span"
        );
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
}

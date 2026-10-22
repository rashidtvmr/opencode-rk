//! Native appearance editor: real keyboard and pointer inputs, no IO.
use crate::native_box_style::{
    BorderStyle, BoxAppearance, BoxId, BoxOverride, RadiusPreset, radius_from_pointer,
};

pub struct AppearanceEditor {
    pub style: BoxAppearance,
    pub open: bool,
    target: usize,
    dragging: bool,
}
impl AppearanceEditor {
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
        if preset == RadiusPreset::IosSquircle {
            self.target = 0;
            self.style.preset = preset;
            self.style.radius = 50;
            self.style.overrides.clear();
        } else if let Some(id) = self.target() {
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
                self.preset(RadiusPreset::Custom);
                true
            }
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
    pub fn pointer(
        &mut self,
        x: u16,
        y: u16,
        kind: u8,
        content_x: u16,
        content_y: u16,
        content_width: u16,
    ) -> bool {
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
        let target = self
            .target()
            .map(|id| {
                serde_json::to_string(&id)
                    .unwrap_or_default()
                    .trim_matches('"')
                    .to_owned()
            })
            .unwrap_or_else(|| "All boxes".into());
        let effective = if let Some(id) = self.target() {
            self.style.effective(id)
        } else {
            let mut global = self.style.clone();
            global.overrides.clear();
            global.effective(BoxId::ChatComposer)
        };
        let radius = self.radius();
        let track_width = Self::track_width(width);
        let cursor = if track_width > 1 {
            (u32::from(radius) * u32::from(track_width - 1) / 50) as usize
        } else {
            0
        };
        let track: String = (0..usize::from(track_width))
            .map(|index| if index == cursor { '●' } else { '─' })
            .collect();
        vec![
            "Box appearance".into(),
            format!("> Target: {target} · ↑/↓ or Tab: next box"),
            format!("Preset: {:?}", effective.preset),
            format!("Roundedness: {radius}/50 logical"),
            format!("  [{track}]"),
            format!(
                "Border: {:?} · Padding: {}",
                effective.border_style, effective.padding
            ),
            "1 sharp · 2 rounded · 3 iOS squircle ALL · 4 custom".into(),
            "Drag slider · +/- adjust · 0 sharp · 5 maximum".into(),
            "b border · p padding · Escape close".into(),
            "Squircle: terminal approximation, not pixel curves".into(),
        ]
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
    fn squircle_all_clears_individual_override() {
        let mut editor = AppearanceEditor::new(BoxAppearance::default());
        editor.key(9);
        editor.key(b'+');
        assert!(!editor.style.overrides.is_empty());
        editor.key(b'3');
        assert!(editor.style.overrides.is_empty());
        assert_eq!(editor.style.preset, RadiusPreset::IosSquircle);
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
}

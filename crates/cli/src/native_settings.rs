#![forbid(unsafe_code)]
//! Focused native parity slice for upstream DialogConfig: Tabs/Layout.

use crate::native_navigation::TabOrientation;
pub const MAX_ROWS: usize = 1;
pub const MAX_QUERY_CHARS: usize = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SettingsAction { Apply(TabOrientation), Close }
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettingsRow { pub id: &'static str, pub category: &'static str, pub title: &'static str, pub description: &'static str, pub key: &'static str }
#[derive(Clone, Debug)]
pub struct SettingsDialog { open: bool, selected: usize, current: TabOrientation, query: String }
impl Default for SettingsDialog { fn default() -> Self { Self { open: false, selected: 0, current: TabOrientation::Horizontal, query: String::new() } } }
impl SettingsDialog {
    pub fn open(&mut self, current: TabOrientation) { self.open = true; self.current = current; self.selected = 0; self.query.clear(); }
    pub fn close(&mut self) { self.open = false; self.query.clear(); }
    pub const fn is_open(&self) -> bool { self.open }
    pub const fn current(&self) -> TabOrientation { self.current }
    pub fn query(&self) -> &str { &self.query }
    pub fn has_visible_row(&self) -> bool {
        let query = self.query.trim().to_lowercase();
        query.is_empty() || ["layout", "tabs", "tabs.layout", "top", "side", "orientation"].iter().any(|value| value.contains(&query) || query.contains(value))
    }
    pub const fn rows() -> [SettingsRow; MAX_ROWS] { [SettingsRow { id: "tabs.layout", category: "Tabs", title: "Layout", description: "Whether tabs run along the top or down the side", key: "tabs.layout" }] }
    pub fn push_query(&mut self, ch: char) -> bool { if !self.open || ch.is_control() || self.query.chars().count() >= MAX_QUERY_CHARS { return false; } self.query.push(ch); true }
    pub fn backspace(&mut self) -> bool { self.open && self.query.pop().is_some() }
    pub fn select(&mut self, index: usize) -> bool { if !self.open || index >= MAX_ROWS { return false; } self.selected = index; true }
    pub fn next(&mut self) { let _ = self.select(0); }
    pub fn previous(&mut self) { let _ = self.select(0); }
    pub fn click(&mut self, id: &str) -> Option<SettingsAction> { if id != "tabs.layout" { return None; } self.selected = 0; self.right() }
    pub fn left(&mut self) -> Option<SettingsAction> { self.cycle(-1) }
    pub fn right(&mut self) -> Option<SettingsAction> { self.cycle(1) }
    fn cycle(&mut self, _direction: i8) -> Option<SettingsAction> { if !self.open || !self.has_visible_row() { return None; } let next = match self.current { TabOrientation::Horizontal => TabOrientation::Vertical, TabOrientation::Vertical => TabOrientation::Horizontal }; Some(SettingsAction::Apply(next)) }
    /// Commit the value only after its configuration write succeeds.
    pub fn applied(&mut self, orientation: TabOrientation) { self.current = orientation; }
    pub fn activate(&self) -> Option<SettingsAction> { self.open.then(|| SettingsAction::Apply(self.current)) }
    /// Hit only the visible Layout row, in modal-content coordinates.
    pub fn hit_layout(&self, x: u16, y: u16, width: u16, height: usize) -> bool {
        if y != 3 { return false; }
        self.render(width, height).get(3).is_some_and(|line| {
            let cells = crate::native_view_layout::visit_unpadded_cells(line.trim_end(), usize::from(width), |_, _| {});
            usize::from(x) < cells
        })
    }
    pub fn render(&self, width: u16, height: usize) -> Vec<String> {
        if !self.open || width == 0 || height == 0 { return Vec::new(); }
        if !self.has_visible_row() { return vec!["Settings".into(), format!("Search settings: {}", self.query), "No matching settings".into()].into_iter().take(height).map(|line| crate::native_view_layout::fit(&line, width)).collect(); }
        let value = match self.current { TabOrientation::Horizontal => "horizontal", TabOrientation::Vertical => "vertical" };
        let left = "> Layout"; let right = format!("{value}  "); let gap = usize::from(width).saturating_sub(left.len() + right.len());
        let mut out = vec!["Settings".into(), format!("Search settings: {}", self.query), "Tabs".into(), format!("{left}{}{right}", " ".repeat(gap)), "Whether tabs run along the top or down the side".into()];
        out.truncate(height); out.into_iter().map(|line| crate::native_view_layout::fit(&line, width)).collect()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn mouse_requires_a_visible_rendered_layout_row() {
        let mut d = SettingsDialog::default(); d.open(TabOrientation::Horizontal);
        assert!(!d.hit_layout(0, 3, 60, 3));
        assert!(d.hit_layout(0, 3, 60, 4));
        assert!(!d.hit_layout(59, 3, 60, 4));
        assert!(!d.hit_layout(0, 2, 60, 4));
        d.push_query('z'); assert!(!d.hit_layout(0, 3, 60, 4));
    }
    #[test] fn current_layout_is_focused_and_arrows_cycle() { let mut d=SettingsDialog::default(); d.open(TabOrientation::Vertical); assert_eq!(d.activate(),Some(SettingsAction::Apply(TabOrientation::Vertical))); assert_eq!(d.left(),Some(SettingsAction::Apply(TabOrientation::Horizontal))); }
    #[test] fn grouped_layout_row_and_bounded_render() { assert_eq!(SettingsDialog::rows()[0].category,"Tabs"); assert_eq!(SettingsDialog::rows()[0].title,"Layout"); let mut d=SettingsDialog::default(); d.open(TabOrientation::Horizontal); assert!(d.render(12,4).iter().all(|x|x.chars().count()<=12)); }
    #[test] fn failed_apply_does_not_close_model() { let mut d=SettingsDialog::default(); d.open(TabOrientation::Horizontal); let _=d.activate(); assert!(d.is_open()); }
}

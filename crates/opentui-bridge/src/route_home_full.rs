#![forbid(unsafe_code)]
//! Home route tab (`packages/tui/src/routes/home.tsx` `Home`).
pub const MAX_TAB_LEN: usize = 32;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HomeRoute {
    pub tab: String,
}
impl HomeRoute {
    pub fn set_tab(&mut self, tab: &str) {
        if tab.is_empty() {
            return;
        }
        let mut end = tab.len().min(MAX_TAB_LEN);
        while !tab.is_char_boundary(end) {
            end -= 1;
        }
        self.tab = tab[..end].to_string();
    }
    #[must_use]
    pub fn tab_of(&self) -> &str {
        &self.tab
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn route() -> HomeRoute {
        HomeRoute { tab: "home".into() }
    }
    #[test]
    fn set_and_get() {
        let mut r = route();
        r.set_tab("sessions");
        assert_eq!(r.tab_of(), "sessions");
    }
    #[test]
    fn empty_ignored() {
        let mut r = route();
        r.set_tab("");
        assert_eq!(r.tab_of(), "home");
    }
    #[test]
    fn truncates() {
        let mut r = route();
        r.set_tab(&"a".repeat(MAX_TAB_LEN + 5));
        assert_eq!(r.tab.len(), MAX_TAB_LEN);
    }
}

//! Session route open state + sidebar width. TS: sidebar.tsx overlay/Show + width 42.
#![forbid(unsafe_code)]

pub struct SessionRoute {
    pub open: bool,
    pub width: u16,
}

impl SessionRoute {
    pub fn new() -> Self {
        Self {
            open: false,
            width: 42,
        }
    }
    pub fn toggle(&mut self) {
        self.open = !self.open;
    }
    pub fn set_width(&mut self, w: u16) {
        self.width = w.clamp(20, 80);
    }
    pub fn render_hint(&self) -> String {
        let s = format!(
            "{}:{}",
            if self.open { "open" } else { "closed" },
            self.width
        );
        s.chars().take(64).collect()
    }
}

impl Default for SessionRoute {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn default_closed_42() {
        let r = SessionRoute::new();
        assert!(!r.open && r.width == 42);
    }
    #[test]
    fn toggle_flips() {
        let mut r = SessionRoute::new();
        r.toggle();
        assert!(r.open);
        r.toggle();
        assert!(!r.open);
    }
    #[test]
    fn clamp_bounds() {
        let mut r = SessionRoute::new();
        r.set_width(10);
        assert_eq!(r.width, 20);
        r.set_width(200);
        assert_eq!(r.width, 80);
        r.set_width(50);
        assert_eq!(r.width, 50);
    }
    #[test]
    fn hint_format_capped() {
        let mut r = SessionRoute::new();
        assert_eq!(r.render_hint(), "closed:42");
        r.toggle();
        r.set_width(80);
        assert_eq!(r.render_hint(), "open:80");
        assert!(r.render_hint().len() <= 64);
    }
}

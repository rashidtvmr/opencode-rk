#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct HelpOverlay {
    pub visible: bool,
}

impl HelpOverlay {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    pub fn show(&mut self) {
        self.visible = true;
    }
    pub fn hide(&mut self) {
        self.visible = false;
    }
    pub fn toggle(&mut self) -> bool {
        self.visible = !self.visible;
        self.visible
    }
    #[must_use]
    pub fn is_visible(&self) -> bool {
        self.visible
    }
}

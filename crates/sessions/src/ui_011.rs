#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TrustToggle {
    pub trusted: bool,
}

impl TrustToggle {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    pub fn set(&mut self, value: bool) -> String {
        self.trusted = value;
        if value {
            "trusted:on".to_owned()
        } else {
            "trusted:off".to_owned()
        }
    }
    #[must_use]
    pub fn is_trusted(&self) -> bool {
        self.trusted
    }
}

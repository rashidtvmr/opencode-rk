use thiserror::Error;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SteerAction {
    Interrupt,
    Resume,
    Retry,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SteerState {
    pub busy: bool,
    pub last_action: Option<String>,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum SteerError {
    #[error("nothing is running to interrupt")]
    NotBusy,
    #[error("a turn is already running")]
    AlreadyBusy,
}

impl SteerState {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    pub fn request(&mut self, action: SteerAction) -> Result<String, SteerError> {
        match action {
            SteerAction::Interrupt => {
                if !self.busy {
                    return Err(SteerError::NotBusy);
                }
                self.busy = false;
                self.last_action = Some("interrupted".to_owned());
                Ok("interrupted".to_owned())
            }
            SteerAction::Resume => {
                if self.busy {
                    return Err(SteerError::AlreadyBusy);
                }
                self.busy = true;
                self.last_action = Some("resumed".to_owned());
                Ok("resumed".to_owned())
            }
            SteerAction::Retry => {
                if self.busy {
                    return Err(SteerError::AlreadyBusy);
                }
                self.busy = true;
                self.last_action = Some("retried".to_owned());
                Ok("retried".to_owned())
            }
        }
    }
}

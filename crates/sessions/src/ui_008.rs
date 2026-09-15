use thiserror::Error;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Approval {
    Allow,
    Deny,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApprovalCard {
    pub tool: String,
    pub pending: bool,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum ApprovalError {
    #[error("tool name must not be empty")]
    EmptyTool,
}

impl ApprovalCard {
    pub fn new(tool: &str) -> Result<Self, ApprovalError> {
        if tool.is_empty() {
            return Err(ApprovalError::EmptyTool);
        }
        Ok(Self {
            tool: tool.to_owned(),
            pending: true,
        })
    }
    pub fn decide(&mut self, approval: Approval) -> String {
        self.pending = false;
        match approval {
            Approval::Allow => format!("allowed:{}", self.tool),
            Approval::Deny => format!("denied:{}", self.tool),
        }
    }
    #[must_use]
    pub fn is_pending(&self) -> bool {
        self.pending
    }
}

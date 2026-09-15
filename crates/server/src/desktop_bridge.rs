//! Desktop-bridge channel slice (opencode.desktop-client; WEB-003 half).
//! Pure logic: parse channel names and build channel-scoped topics.

use std::error::Error;
use std::fmt::{self, Display};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    Stable,
    Beta,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BridgeError {
    UnknownChannel { name: String },
    EmptyTopic,
}

impl Display for BridgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownChannel { name } => write!(f, "unknown channel: {name}"),
            Self::EmptyTopic => write!(f, "empty topic"),
        }
    }
}

impl Error for BridgeError {}

pub fn parse_channel(n: &str) -> Result<Channel, BridgeError> {
    match n.trim().to_ascii_lowercase().as_str() {
        "stable" => Ok(Channel::Stable),
        "beta" => Ok(Channel::Beta),
        _ => Err(BridgeError::UnknownChannel {
            name: n.to_string(),
        }),
    }
}

pub fn topic_for(ch: &Channel, topic: &str) -> Result<String, BridgeError> {
    let t = topic.trim();
    if t.is_empty() {
        return Err(BridgeError::EmptyTopic);
    }
    let prefix = match ch {
        Channel::Stable => "stable",
        Channel::Beta => "beta",
    };
    Ok(format!("{prefix}:{t}"))
}

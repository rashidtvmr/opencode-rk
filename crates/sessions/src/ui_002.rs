use thiserror::Error;

pub const MAX_THREAD_MSGS: usize = 1000;
pub const MAX_MSG_CHARS: usize = 32768;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ThreadMsg {
    pub id: String,
    pub role: String,
    pub text: String,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum ThreadError {
    #[error("message id must not be empty")]
    EmptyId,
    #[error("message role must be system, user, assistant, or tool")]
    EmptyRole,
    #[error("message text too long: max {max}, actual {actual}")]
    TextTooLong { max: usize, actual: usize },
    #[error("too many messages: max {max}, actual {actual}")]
    TooManyMessages { max: usize, actual: usize },
}

pub fn append_msg(buf: &mut Vec<ThreadMsg>, m: ThreadMsg) -> Result<(), ThreadError> {
    if m.id.is_empty() {
        return Err(ThreadError::EmptyId);
    }
    match m.role.as_str() {
        "system" | "user" | "assistant" | "tool" => {}
        _ => return Err(ThreadError::EmptyRole),
    }
    let len = m.text.chars().count();
    if len > MAX_MSG_CHARS {
        return Err(ThreadError::TextTooLong {
            max: MAX_MSG_CHARS,
            actual: len,
        });
    }
    if buf.len() >= MAX_THREAD_MSGS {
        return Err(ThreadError::TooManyMessages {
            max: MAX_THREAD_MSGS,
            actual: buf.len(),
        });
    }
    buf.push(m);
    Ok(())
}

#[must_use]
pub fn latest_n(buf: &[ThreadMsg], n: usize) -> &[ThreadMsg] {
    if n == 0 || buf.is_empty() {
        return &[];
    }
    let start = buf.len().saturating_sub(n);
    &buf[start..]
}

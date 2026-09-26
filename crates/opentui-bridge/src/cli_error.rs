#![forbid(unsafe_code)]
//! CLI error message extraction (mirrors `packages/tui/src/util/error.ts:5-17`).
//!
//! `TaggedError` is the typed counterpart of the TS `isRecord` check
//! (`packages/tui/src/util/record.ts:1-3`); `cause` mirrors `cause.body`
//! recursion (`error.ts:6-9`). TS checkout `a0d9b6c`, not pinned `95daf90`.
//!
//! Divergence: TS sets `process.exitCode` as a side effect (`error.ts:12`).
//! This lib never calls `process::exit`; it returns the code for the caller.

/// Known `_tag` values handled by `cli_error_message`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    Cli,
    AccountService,
    AccountTransport,
    Unknown,
}

/// Classify a `_tag` string. Fail-closed: anything else is `Unknown`.
#[must_use]
pub fn classify(tag: &str) -> ErrorKind {
    match tag {
        "CliError" => ErrorKind::Cli,
        "AccountServiceError" => ErrorKind::AccountService,
        "AccountTransportError" => ErrorKind::AccountTransport,
        _ => ErrorKind::Unknown,
    }
}

/// Typed error node: `_tag` + `message` + optional `exitCode` + `cause.body` chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaggedError {
    pub tag: String,
    pub message: String,
    pub exit_code: Option<i32>,
    pub cause: Option<Box<TaggedError>>,
}

impl TaggedError {
    pub fn new(tag: &str, message: &str) -> Self {
        Self {
            tag: tag.to_string(),
            message: message.to_string(),
            exit_code: None,
            cause: None,
        }
    }

    pub fn with_exit_code(mut self, code: i32) -> Self {
        self.exit_code = Some(code);
        self
    }

    pub fn with_cause(mut self, cause: TaggedError) -> Self {
        self.cause = Some(Box::new(cause));
        self
    }
}

/// Displayable failure: message plus exit code for the caller to apply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliFailure {
    pub message: String,
    pub exit_code: Option<i32>,
}

/// Innermost `cause` message wins (TS early-return); tag-agnostic helper.
/// Iterative walk: terminates for any finite chain, no stack growth.
#[must_use]
pub fn message_of(err: &TaggedError) -> String {
    let mut cur = err;
    while let Some(next) = cur.cause.as_deref() {
        cur = next;
    }
    cur.message.clone()
}

/// Formatted message for known tags, else `None`. Nested `cause.body` wins
/// even when the outer tag is unknown (mirrors `error.ts:6-9` precedence).
#[must_use]
pub fn cli_error_message(err: &TaggedError) -> Option<String> {
    let mut chain: Vec<&TaggedError> = Vec::new();
    let mut cur = err;
    loop {
        chain.push(cur);
        match cur.cause.as_deref() {
            Some(next) => cur = next,
            None => break,
        }
    }
    for node in chain.iter().rev() {
        if classify(&node.tag) != ErrorKind::Unknown {
            return Some(node.message.clone());
        }
    }
    None
}

/// Exit code of the error node that produced the message (`CliError` only,
/// mirroring `error.ts:11-13` minus the `process.exitCode` side effect).
#[must_use]
pub fn exit_code_of(err: &TaggedError) -> Option<i32> {
    let mut chain: Vec<&TaggedError> = Vec::new();
    let mut cur = err;
    loop {
        chain.push(cur);
        match cur.cause.as_deref() {
            Some(next) => cur = next,
            None => break,
        }
    }
    for node in chain.iter().rev() {
        match classify(&node.tag) {
            ErrorKind::Cli => return node.exit_code,
            ErrorKind::AccountService | ErrorKind::AccountTransport => return None,
            ErrorKind::Unknown => {}
        }
    }
    None
}

/// Message + exit code for known tags, else `None`.
#[must_use]
pub fn cli_failure(err: &TaggedError) -> Option<CliFailure> {
    cli_error_message(err).map(|message| CliFailure {
        message,
        exit_code: exit_code_of(err),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_error_message_returns_message() {
        let err = TaggedError::new("CliError", "boom");
        assert_eq!(classify("CliError"), ErrorKind::Cli);
        assert_eq!(cli_error_message(&err), Some("boom".to_string()));
    }

    #[test]
    fn nested_cause_body_wins() {
        let err = TaggedError::new("CliError", "outer")
            .with_cause(TaggedError::new("AccountServiceError", "inner"));
        assert_eq!(cli_error_message(&err), Some("inner".to_string()));
        assert_eq!(message_of(&err), "inner".to_string());
    }

    #[test]
    fn unknown_tag_none() {
        let err = TaggedError::new("Nope", "x");
        assert_eq!(classify("Nope"), ErrorKind::Unknown);
        assert_eq!(cli_error_message(&err), None);
        assert_eq!(cli_failure(&err), None);
    }

    #[test]
    fn exit_code_carried() {
        let err = TaggedError::new("CliError", "boom").with_exit_code(3);
        assert_eq!(exit_code_of(&err), Some(3));
        assert_eq!(
            cli_failure(&err),
            Some(CliFailure {
                message: "boom".to_string(),
                exit_code: Some(3),
            })
        );
        let acct = TaggedError::new("AccountTransportError", "m").with_exit_code(9);
        assert_eq!(exit_code_of(&acct), None);
    }

    #[test]
    fn deep_nesting_terminates() {
        let mut err = TaggedError::new("CliError", "leaf");
        for i in 0..5000 {
            err = TaggedError::new("CliError", format!("wrap-{i}").as_str()).with_cause(err);
        }
        assert_eq!(cli_error_message(&err), Some("leaf".to_string()));
        assert_eq!(message_of(&err), "leaf".to_string());
    }

    #[test]
    fn account_kinds_mapped() {
        assert_eq!(classify("AccountServiceError"), ErrorKind::AccountService);
        assert_eq!(classify("AccountTransportError"), ErrorKind::AccountTransport);
        let err = TaggedError::new("AccountServiceError", "auth broke");
        assert_eq!(cli_error_message(&err), Some("auth broke".to_string()));
    }
}

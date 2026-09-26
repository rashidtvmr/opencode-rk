//! Prompt part split (TS: `prompt/part.ts`).
#![forbid(unsafe_code)]

pub const MAX_TEXT_CHARS: usize = 4096;
pub const MAX_SLASH_CHARS: usize = 128;
pub const MAX_MENTION_CHARS: usize = 512;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PartKind2 {
    Text(String),
    Slash(String),
    Mention(String),
}

fn take(s: &str, cap: usize) -> String {
    s.chars().take(cap).collect()
}

pub fn split_part(text: &str) -> PartKind2 {
    if let Some(cmd) = text.strip_prefix('/') {
        let cmd = cmd.split_whitespace().next().unwrap_or("");
        return PartKind2::Slash(take(cmd, MAX_SLASH_CHARS));
    }
    if let Some(at) = text.find('@') {
        let token = text[at + 1..].split_whitespace().next().unwrap_or("");
        let token =
            token.trim_end_matches(|c: char| matches!(c, ',' | '.' | ';' | ':' | '!' | '?' | ')'));
        return PartKind2::Mention(take(token, MAX_MENTION_CHARS));
    }
    PartKind2::Text(take(text, MAX_TEXT_CHARS))
}

pub fn render(part: &PartKind2) -> String {
    match part {
        PartKind2::Text(s) => s.clone(),
        PartKind2::Slash(cmd) => format!("/{cmd}"),
        PartKind2::Mention(name) => format!("@{name}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slash_split() {
        assert_eq!(
            split_part("/commit foo"),
            PartKind2::Slash("commit".to_string())
        );
    }

    #[test]
    fn mention_split() {
        assert_eq!(
            split_part("hi @user yo"),
            PartKind2::Mention("user".to_string())
        );
    }

    #[test]
    fn text_passthrough() {
        assert_eq!(
            split_part("hello world"),
            PartKind2::Text("hello world".to_string())
        );
    }

    #[test]
    fn empty_text() {
        assert_eq!(split_part(""), PartKind2::Text(String::new()));
    }

    #[test]
    fn render_roundtrip() {
        assert_eq!(render(&split_part("/commit foo")), "/commit");
        assert_eq!(render(&split_part("hi @user yo")), "@user");
        assert_eq!(render(&split_part("hello")), "hello");
    }
}

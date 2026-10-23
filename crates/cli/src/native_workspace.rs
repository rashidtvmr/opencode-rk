#![forbid(unsafe_code)]
//! Safe native workspace command parsing and resolution.

use std::path::{Path, PathBuf};

pub fn cd_argument(input: &str) -> Result<String, &'static str> {
    let rest = input.strip_prefix("/cd").ok_or("not a cd command")?;
    if rest.chars().next().is_some_and(|ch| !ch.is_whitespace()) {
        return Err("/cd requires a directory");
    }
    let mut out = String::new();
    let mut chars = rest.trim_start().chars().peekable();
    if chars.peek().is_none() { return Err("/cd requires a directory"); }
    let mut quote: Option<char> = None;
    let mut escaped = false;
    while let Some(ch) = chars.next() {
        if escaped {
            out.push(ch);
            escaped = false;
        } else if ch == '\\' && quote != Some('\'') {
            escaped = true;
        } else {
            match quote {
                Some(active) if ch == active => quote = None,
                Some(_) => out.push(ch),
                None if ch == '\'' || ch == '"' => quote = Some(ch),
                None if ch.is_whitespace() => {
                    if chars.any(|ch| !ch.is_whitespace()) || out.is_empty() {
                        return Err("/cd accepts exactly one directory");
                    }
                    break;
                }
                None => out.push(ch),
            }
        }
    }
    if escaped || quote.is_some() { return Err("unterminated /cd argument"); }
    if out.is_empty() { Err("/cd requires a directory") } else { Ok(out) }
}

pub fn resolve_directory(raw: &str, base: &Path) -> Result<PathBuf, String> {
    if raw.is_empty() || raw.len() > 4096 || raw.contains('\0') { return Err("invalid working directory".into()); }
    let expanded = if raw == "~" {
        std::env::var_os("HOME").map(PathBuf::from).ok_or("HOME is unavailable")?
    } else if let Some(rest) = raw.strip_prefix("~/") {
        std::env::var_os("HOME").map(PathBuf::from).ok_or("HOME is unavailable")?.join(rest)
    } else { PathBuf::from(raw) };
    let candidate = if expanded.is_absolute() { expanded } else { base.join(expanded) };
    let resolved = candidate.canonicalize().map_err(|_| "working directory does not exist".to_owned())?;
    if !resolved.is_dir() { return Err("working directory is not a directory".into()); }
    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cd_requires_one_argument() {
        assert_eq!(cd_argument("/cd /tmp"), Ok("/tmp".into()));
        assert_eq!(cd_argument("/cd \"dir with spaces\""), Ok("dir with spaces".into()));
        assert_eq!(cd_argument("/cd dir\\ with\\ spaces"), Ok("dir with spaces".into()));
        assert!(cd_argument("/cd").is_err());
        assert!(cd_argument("/cd /tmp && echo nope").is_err());
        assert!(cd_argument("/cd foo\"").is_err());
        assert!(cd_argument("/cd foo'bar").is_err());
        assert_eq!(cd_argument("/cd foo\\\"bar"), Ok("foo\"bar".into()));
        // A backslash is literal inside single quotes; this sequence closes
        // the quote and starts another one, so the final quote is unmatched.
        assert!(cd_argument("/cd 'foo\\'bar'").is_err());
        assert_eq!(cd_argument("/cd 'foo\\bar'"), Ok("foo\\bar".into()));
        assert_eq!(cd_argument("/cd \"foo\\\"bar\""), Ok("foo\"bar".into()));
        assert!(cd_argument("/cd foo\"bar").is_err());
        assert!(cd_argument("/cd foo'bar").is_err());
    }
}

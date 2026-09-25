#![forbid(unsafe_code)]

//! Context screen rows over [`CtxBundle`].
//!
//! TS truth: `crates/cli/src/tui_entry.rs:537-551` Context arm (title +
//! snapshot kv lines, width-clip + height-truncate at `:569-575`).

use crate::ctx_bundle::CtxBundle;

fn fit(s: &str, width: usize) -> String {
    let mut t: String = s.chars().take(width).collect();
    while t.chars().count() < width {
        t.push(' ');
    }
    t
}

/// Title + summary + kv `key: value` rows, padded/clipped to `width`,
/// capped at `height` rows.
pub fn context_lines(bundle: &CtxBundle, width: usize, height: usize) -> Vec<String> {
    if width == 0 || height == 0 {
        return Vec::new();
    }
    let mut out = vec![fit("Context", width)];
    if out.len() >= height {
        out.truncate(height);
        return out;
    }
    out.push(fit(&bundle.summary(), width));
    for k in bundle.kv.keys() {
        if out.len() >= height {
            break;
        }
        let v = bundle.kv.get(&k).unwrap_or("");
        out.push(fit(&format!("{k}: {v}"), width));
    }
    out.truncate(height);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_and_summary_first() {
        let b = CtxBundle::new("/tmp");
        let l = context_lines(&b, 40, 10);
        assert!(l[0].starts_with("Context"));
        assert!(l[1].starts_with("route=home"));
        assert!(l.iter().any(|r| r.starts_with("cwd: /tmp")));
    }

    #[test]
    fn rows_padded_to_width() {
        let b = CtxBundle::new("/tmp");
        for r in context_lines(&b, 30, 10) {
            assert_eq!(r.chars().count(), 30);
        }
    }

    #[test]
    fn width_clips_and_height_caps() {
        let mut b = CtxBundle::new("/tmp");
        b.set("a", "1");
        let l = context_lines(&b, 5, 2);
        assert_eq!(l.len(), 2);
        assert!(l.iter().all(|r| r.chars().count() == 5));
        assert!(context_lines(&b, 0, 5).is_empty());
        assert!(context_lines(&b, 10, 0).is_empty());
    }

    #[test]
    fn kv_capped_by_body() {
        let mut b = CtxBundle::new("/tmp");
        b.set("a", "1");
        b.set("c", "3");
        let l = context_lines(&b, 40, 3);
        assert_eq!(l.len(), 3);
        assert!(l[2].starts_with("cwd:") || l[2].starts_with("a:"));
    }

    #[test]
    fn empty_dims_yield_empty() {
        let b = CtxBundle::new("/tmp");
        assert!(context_lines(&b, 0, 0).is_empty());
    }
}

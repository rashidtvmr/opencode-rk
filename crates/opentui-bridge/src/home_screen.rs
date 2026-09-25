#![forbid(unsafe_code)]
//! Home screen frame: title + cwd + recent + footer tip.
//! TS truth `packages/tui/src/routes/home.tsx` (prompt-first layout).

use crate::home_footer::HomeFooter;
use crate::home_route::HomeRoute;

fn clip(s: &str, width: usize) -> String {
    if s.chars().count() <= width {
        s.to_string()
    } else {
        s.chars().take(width).collect()
    }
}

/// Frame home screen to exactly `height` rows, each char-safe clipped to `width`.
/// Layout: title, `cwd <dir>`, recent list (or `recent: none`), blank pad,
/// footer current tip always last (except height 0 coerced to 1).
pub fn home_lines(
    route: &HomeRoute,
    footer: &HomeFooter,
    width: usize,
    height: usize,
) -> Vec<String> {
    let width = width.max(1);
    let height = height.max(1);
    let foot = clip(footer.current().unwrap_or(""), width);
    if height == 1 {
        return vec![foot];
    }
    let mut body = vec![route.title(), format!("cwd {}", route.cwd)];
    if route.recent.is_empty() {
        body.push("recent: none".to_string());
    } else {
        body.push("recent:".to_string());
        body.extend(route.recent.iter().map(|d| format!("  {d}")));
    }
    let mut lines: Vec<String> = body.into_iter().map(|l| clip(&l, width)).collect();
    lines.truncate(height - 1);
    while lines.len() < height - 1 {
        lines.push(String::new());
    }
    lines.push(foot);
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn route() -> HomeRoute {
        let mut r = HomeRoute::new("/x/proj");
        r.add_recent("/a".into());
        r.add_recent("/b".into());
        r
    }

    fn footer() -> HomeFooter {
        HomeFooter::new(vec!["tip one".into()])
    }

    #[test]
    fn exact_height_title_first() {
        let out = home_lines(&route(), &footer(), 40, 8);
        assert_eq!(out.len(), 8);
        assert_eq!(out[0], route().title());
    }

    #[test]
    fn cwd_second_recent_listed() {
        let out = home_lines(&route(), &footer(), 40, 8);
        assert_eq!(out[1], "cwd /x/proj");
        assert!(out.contains(&"  /b".to_string()));
        assert!(out.contains(&"  /a".to_string()));
    }

    #[test]
    fn empty_recent_placeholder() {
        let r = HomeRoute::new("/x");
        let out = home_lines(&r, &footer(), 40, 6);
        assert!(out.contains(&"recent: none".to_string()));
    }

    #[test]
    fn footer_always_last() {
        let out = home_lines(&route(), &footer(), 40, 6);
        assert_eq!(out.last().unwrap(), "tip one");
        let tiny = home_lines(&route(), &footer(), 40, 1);
        assert_eq!(tiny, vec!["tip one".to_string()]);
    }

    #[test]
    fn truncates_body_keeps_footer() {
        let out = home_lines(&route(), &footer(), 40, 3);
        assert_eq!(out.len(), 3);
        assert_eq!(out[2], "tip one");
    }

    #[test]
    fn width_clip_char_safe() {
        let mut r = HomeRoute::new("/héllo✓wörld-long-path");
        r.add_recent("/éééééééééééé".into());
        let f = HomeFooter::new(vec!["✓✓✓✓✓✓✓✓✓✓✓✓".into()]);
        let out = home_lines(&r, &f, 5, 8);
        for l in &out {
            assert!(l.chars().count() <= 5, "overflow: {l:?}");
        }
    }
}

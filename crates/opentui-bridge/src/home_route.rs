#![forbid(unsafe_code)]
//! Home route state: cwd + recent dirs + session destination.
//! TS truth `packages/tui/src/routes/home.tsx:22` (`Home`, prompt-first layout).

/// Max stored path length; longer paths truncate on a char boundary.
pub const MAX_PATH_LEN: usize = 512;
/// Max recent entries.
pub const MAX_RECENT: usize = 16;
/// Max destination id length; mirrors `home_destination::MAX_ID_LEN`.
pub const MAX_DEST_LEN: usize = 64;

fn trunc(s: &str, cap: usize) -> String {
    if s.len() <= cap {
        return s.to_string();
    }
    let mut end = cap;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].to_string()
}

fn abbrev(cwd: &str) -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    if home.is_empty() {
        return cwd.to_string();
    }
    if cwd == home {
        return "~".to_string();
    }
    let pref = format!("{home}/");
    if let Some(rest) = cwd.strip_prefix(&pref) {
        return format!("~/{rest}");
    }
    cwd.to_string()
}

/// Home route state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HomeRoute {
    pub cwd: String,
    pub recent: Vec<String>,
    pub dest: Option<String>,
}

impl HomeRoute {
    /// New route; empty cwd falls back to `"."`.
    pub fn new(cwd: &str) -> Self {
        let mut r = Self {
            cwd: String::new(),
            recent: Vec::new(),
            dest: None,
        };
        if !r.set_cwd(cwd) {
            r.cwd = ".".to_string();
        }
        r
    }

    /// Set cwd; empty returns false, longer truncates to [`MAX_PATH_LEN`].
    pub fn set_cwd(&mut self, cwd: &str) -> bool {
        if cwd.is_empty() {
            return false;
        }
        self.cwd = trunc(cwd, MAX_PATH_LEN);
        true
    }

    /// Push dir to front; dup moves to front, oldest evicted past [`MAX_RECENT`].
    pub fn add_recent(&mut self, dir: String) -> bool {
        if dir.is_empty() {
            return false;
        }
        let dir = trunc(&dir, MAX_PATH_LEN);
        self.recent.retain(|d| d != &dir);
        self.recent.insert(0, dir);
        self.recent.truncate(MAX_RECENT);
        true
    }

    /// Select destination; `None`/empty clears, longer truncates to [`MAX_DEST_LEN`].
    pub fn select_dest(&mut self, dest: Option<&str>) {
        match dest {
            None => self.dest = None,
            Some(s) if s.is_empty() => self.dest = None,
            Some(s) => self.dest = Some(trunc(s, MAX_DEST_LEN)),
        }
    }

    /// `"home <cwd>"` with `$HOME` abbreviated to `~`.
    pub fn title(&self) -> String {
        format!("home {}", abbrev(&self.cwd))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_cwd_false() {
        let mut r = HomeRoute::new("/x");
        assert!(!r.set_cwd(""));
        assert_eq!(r.cwd, "/x");
    }

    #[test]
    fn cwd_truncates() {
        let mut r = HomeRoute::new("/x");
        assert!(r.set_cwd(&"a".repeat(MAX_PATH_LEN + 10)));
        assert_eq!(r.cwd.len(), MAX_PATH_LEN);
    }

    #[test]
    fn dup_moves_front() {
        let mut r = HomeRoute::new("/x");
        r.add_recent("/a".into());
        r.add_recent("/b".into());
        assert!(r.add_recent("/a".into()));
        assert_eq!(r.recent, vec!["/a".to_string(), "/b".to_string()]);
    }

    #[test]
    fn evicts_oldest() {
        let mut r = HomeRoute::new("/x");
        for i in 0..MAX_RECENT + 3 {
            r.add_recent(format!("/d{i}"));
        }
        assert_eq!(r.recent.len(), MAX_RECENT);
        assert_eq!(r.recent[0], format!("/d{}", MAX_RECENT + 2));
        assert!(!r.recent.contains(&"/d0".to_string()));
    }

    #[test]
    fn dest_select_clear() {
        let mut r = HomeRoute::new("/x");
        r.select_dest(Some("ses_1"));
        assert_eq!(r.dest.as_deref(), Some("ses_1"));
        r.select_dest(None);
        assert_eq!(r.dest, None);
        r.select_dest(Some("ses_2"));
        r.select_dest(Some(""));
        assert_eq!(r.dest, None);
    }

    #[test]
    fn title_tilde() {
        let home = std::env::var("HOME").unwrap_or_default();
        if home.is_empty() {
            return;
        }
        let r = HomeRoute::new(&format!("{home}/proj"));
        assert_eq!(r.title(), "home ~/proj");
        let root = HomeRoute::new(&home);
        assert_eq!(root.title(), "home ~");
    }
}

#![forbid(unsafe_code)]
//! Full question with single/multi select (port of `question.shared.ts` toggle/pick).

/// Max chars for id.
pub const ID_CAP: usize = 64;
/// Max chars for header.
pub const HEADER_CAP: usize = 256;
/// Max options held.
pub const OPT_CAP: usize = 8;
/// Max chars per option.
pub const LABEL_CAP: usize = 256;
/// Max picked indices.
pub const PICK_CAP: usize = 8;

fn trunc(s: &str, cap: usize) -> String {
    s.chars().take(cap).collect()
}

/// Capped question with single or multi selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestionFull {
    id: String,
    header: String,
    opts: Vec<String>,
    multi: bool,
    picked: Vec<usize>,
}

impl QuestionFull {
    /// Build with id/header/label caps; opts truncated to 8.
    pub fn new(id: String, header: String, opts: Vec<String>, multi: bool) -> Self {
        let mut o: Vec<String> = opts.into_iter().map(|s| trunc(&s, LABEL_CAP)).collect();
        o.truncate(OPT_CAP);
        Self {
            id: trunc(&id, ID_CAP),
            header: trunc(&header, HEADER_CAP),
            opts: o,
            multi,
            picked: Vec::new(),
        }
    }
    /// Toggle index. OOB false. Single-mode replaces. Multi adds/removes, cap 8.
    pub fn toggle(&mut self, i: usize) -> bool {
        if i >= self.opts.len() {
            return false;
        }
        if !self.multi {
            self.picked = vec![i];
            return true;
        }
        if let Some(p) = self.picked.iter().position(|&x| x == i) {
            self.picked.remove(p);
            true
        } else if self.picked.len() >= PICK_CAP {
            false
        } else {
            self.picked.push(i);
            true
        }
    }
    /// Picked labels in pick order.
    pub fn done(&self) -> Vec<String> {
        self.picked
            .iter()
            .filter_map(|i| self.opts.get(*i).cloned())
            .collect()
    }
    /// Clear picks.
    pub fn reset(&mut self) {
        self.picked.clear();
    }
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn header(&self) -> &str {
        &self.header
    }
    pub fn opts(&self) -> &[String] {
        &self.opts
    }
    pub fn multi(&self) -> bool {
        self.multi
    }
    pub fn picked(&self) -> &[usize] {
        &self.picked
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn q(multi: bool) -> QuestionFull {
        QuestionFull::new(
            "q".into(),
            "h".into(),
            vec!["a".into(), "b".into(), "c".into()],
            multi,
        )
    }
    #[test]
    fn toggle_on_adds_pick() {
        let mut x = q(true);
        assert!(x.toggle(0));
        assert_eq!(x.picked(), &[0]);
    }
    #[test]
    fn toggle_off_removes_pick() {
        let mut x = q(true);
        x.toggle(0);
        x.toggle(1);
        assert!(x.toggle(0));
        assert_eq!(x.picked(), &[1]);
    }
    #[test]
    fn single_replaces_pick() {
        let mut x = q(false);
        x.toggle(0);
        assert!(x.toggle(2));
        assert_eq!(x.picked(), &[2]);
    }
    #[test]
    fn bounds_false_keeps_pick() {
        let mut x = q(true);
        x.toggle(1);
        assert!(!x.toggle(9));
        assert_eq!(x.picked(), &[1]);
    }
    #[test]
    fn done_returns_labels() {
        let mut x = q(true);
        x.toggle(2);
        x.toggle(0);
        assert_eq!(x.done(), vec!["c".to_string(), "a".to_string()]);
    }
    #[test]
    fn reset_clears_picks() {
        let mut x = q(true);
        x.toggle(0);
        x.reset();
        assert!(x.picked().is_empty() && x.done().is_empty());
    }
    #[test]
    fn caps_truncate() {
        let x = QuestionFull::new(
            "x".repeat(100),
            "y".repeat(300),
            (0..12).map(|i| format!("o{i}")).collect(),
            true,
        );
        assert_eq!(x.id().len(), ID_CAP);
        assert_eq!(x.header().len(), HEADER_CAP);
        assert_eq!(x.opts().len(), OPT_CAP);
    }
}

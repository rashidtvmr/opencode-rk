#![forbid(unsafe_code)]
//! Shared single-question option card.
//!
//! Minimal port of `question.shared.ts` option-pick idea: a capped title,
//! a capped option list, and an index pick with OOB rejection.

/// Maximum options held by [`QuestionCard`]; extras dropped at construction.
pub const OPTION_CAP: usize = 8;
/// Maximum chars kept for [`QuestionOption::id`].
pub const ID_CAP: usize = 64;
/// Maximum chars kept for [`QuestionOption::label`].
pub const LABEL_CAP: usize = 256;
/// Maximum chars kept for [`QuestionCard`] title.
pub const TITLE_CAP: usize = 256;

fn trunc(s: &str, cap: usize) -> String {
    s.chars().take(cap).collect()
}

/// One selectable question option with capped id and label.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestionOption {
    pub id: String,
    pub label: String,
}

impl QuestionOption {
    /// Build with id capped at 64 chars and label at 256 chars.
    pub fn new(id: String, label: String) -> Self {
        Self {
            id: trunc(&id, ID_CAP),
            label: trunc(&label, LABEL_CAP),
        }
    }
}

/// Titled option list with at most 8 options and one optional pick.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestionCard {
    title: String,
    options: Vec<QuestionOption>,
    picked: Option<usize>,
}

impl QuestionCard {
    /// Build with title capped at 256 chars; options truncated to 8.
    pub fn new(title: String, mut options: Vec<QuestionOption>) -> Self {
        options.truncate(OPTION_CAP);
        Self {
            title: trunc(&title, TITLE_CAP),
            options,
            picked: None,
        }
    }

    /// Pick by index; OOB returns false and keeps prior pick.
    pub fn pick(&mut self, idx: usize) -> bool {
        if idx < self.options.len() {
            self.picked = Some(idx);
            true
        } else {
            false
        }
    }

    /// Borrow the picked option, if any.
    pub fn picked_option(&self) -> Option<&QuestionOption> {
        self.picked.and_then(|i| self.options.get(i))
    }

    /// Card title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Option list.
    pub fn options(&self) -> &[QuestionOption] {
        &self.options
    }

    /// Picked index, if any.
    pub fn picked(&self) -> Option<usize> {
        self.picked
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(n: usize) -> QuestionCard {
        QuestionCard::new(
            "t".to_string(),
            (0..n)
                .map(|i| QuestionOption::new(format!("id{i}"), format!("label{i}")))
                .collect(),
        )
    }

    #[test]
    fn pick_ok_sets_picked() {
        let mut c = card(3);
        assert!(c.pick(1));
        assert_eq!(c.picked(), Some(1));
    }

    #[test]
    fn pick_oob_returns_false() {
        let mut c = card(2);
        assert!(!c.pick(5));
        assert_eq!(c.picked(), None);
    }

    #[test]
    fn picked_none_by_default() {
        assert_eq!(card(2).picked(), None);
        assert_eq!(card(2).picked_option(), None);
    }

    #[test]
    fn picked_option_returns_label() {
        let mut c = card(3);
        c.pick(2);
        assert_eq!(c.picked_option().unwrap().label, "label2");
    }

    #[test]
    fn options_capped_at_8() {
        let c = card(12);
        assert_eq!(c.options().len(), OPTION_CAP);
    }

    #[test]
    fn caps_truncate_id_label_title() {
        let o = QuestionOption::new("x".repeat(100), "y".repeat(300));
        assert_eq!(o.id.len(), ID_CAP);
        assert_eq!(o.label.len(), LABEL_CAP);
        let c = QuestionCard::new("z".repeat(300), vec![]);
        assert_eq!(c.title().len(), TITLE_CAP);
    }
}

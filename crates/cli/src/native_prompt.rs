#![forbid(unsafe_code)]
//! Live prompt ownership and the three CP-33 / CP-34 / CP-35 submit/clear
//! fixes.
//!
//! Upstream reference: pin `450e7b92412c780a7b5a75226bb8d49aaca24e22`,
//! `packages/tui/src/component/prompt/index.tsx` (submit / queue / clear /
//! `syncExtmarksWithPromptParts`) and `prompt/mention.ts`.
//!
//! # Ownership model (CP-35)
//!
//! [`TrackedDraft`] (native_paste.rs) is documented as *the owning state the
//! production edit pipeline must migrate to*: it holds `text`, `cursor` and a
//! [`TrackedPrompt`] carrying **every** tracked part (folded pastes, files,
//! agents and skills). The live loop still has to speak the small wire trio
//! (`draft: String`, `draft_cursor: usize`, `draft_parts: Vec<PastedText>`)
//! because the surrounding helper functions (`native_composer`,
//! `native_completion`, ...) take those types directly and cannot be changed
//! from this lane. [`LiveDraft`] bridges that without losing ownership: it
//! keeps a single persistent `TrackedDraft` as the source of truth, adopts the
//! wire trio on entry, runs `realign_after_edit()` (which calls
//! `TrackedPrompt::realign()`), and writes **all** tracked fields back. Every
//! live mutation site calls [`LiveDraft::realign`] so mention anchors follow
//! the edit instead of going stale — the old code only re-anchored folded
//! pastes via the free `reconcile_parts` helper and silently dropped the
//! files/agents/skills the owner had derived.
//!
//! Pasted parts *are* populated in production (`native_composer::
//! insert_paste_at_cursor` pushes them into `draft_parts`); the owner adopts
//! them before realigning, so their ranges are genuinely re-derived after a
//! live edit. Files/agents/skills are owned and realigned by the same owner;
//! the current live composer has no mention-insertion UI that creates them,
//! which is recorded as a limitation rather than claimed as observable here.

use crate::native_paste::{
    should_retain_cleared_draft, DraftEntry, HistoryEntry, PastedText, PromptHistory, PromptMode,
    TrackedDraft, TrackedPrompt,
};

/// The authoritative live owner of the composer prompt (CP-35).
#[derive(Clone, Debug, Default)]
pub(crate) struct LiveDraft {
    owner: TrackedDraft,
}

impl LiveDraft {
    #[must_use]
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Adopt the wire trio into the owner. Files/agents/skills live only in
    /// the owner and are deliberately left untouched here, so they survive a
    /// transport refresh instead of being silently dropped.
    fn adopt(&mut self, text: &str, cursor: usize, pasted: Vec<PastedText>) {
        self.owner.text = text.to_owned();
        self.owner.parts.text = text.to_owned();
        self.owner.cursor = cursor;
        self.owner.parts.pasted = pasted;
    }

    /// Mirror the owner back to the wire trio.
    fn sync_out(&self, text: &mut String, cursor: &mut usize, pasted: &mut Vec<PastedText>) {
        *text = self.owner.text.clone();
        *cursor = self.owner.cursor;
        *pasted = self.owner.parts.pasted.clone();
    }

    /// Realign the authoritative owner after a live text edit and write every
    /// tracked field back. This is the single seam every live mutation site
    /// calls so `TrackedPrompt::realign()` runs on the real owner.
    pub(crate) fn realign(
        &mut self,
        text: &mut String,
        cursor: &mut usize,
        pasted: &mut Vec<PastedText>,
    ) {
        self.adopt(text, *cursor, std::mem::take(pasted));
        self.owner.realign_after_edit();
        self.sync_out(text, cursor, pasted);
    }

    /// Restore a prompt-history entry into the live draft: the restored
    /// prompt owns exactly its own parts, so stale tracked extras are cleared
    /// first and then realigned against the restored text.
    pub(crate) fn restore(
        &mut self,
        text: &mut String,
        cursor: &mut usize,
        pasted: &mut Vec<PastedText>,
        entry: DraftEntry,
    ) {
        self.owner.parts.files.clear();
        self.owner.parts.agents.clear();
        self.owner.parts.skills.clear();
        self.adopt(&entry.text, entry.cursor, entry.parts);
        self.owner.realign_after_edit();
        self.sync_out(text, cursor, pasted);
    }

    /// Clear the live draft and every tracked part it owns.
    pub(crate) fn clear(
        &mut self,
        text: &mut String,
        cursor: &mut usize,
        pasted: &mut Vec<PastedText>,
    ) {
        self.owner = TrackedDraft::default();
        self.sync_out(text, cursor, pasted);
    }

    /// CP-34: retain the cleared draft in history (when upstream would) and
    /// then clear the live owner. Returns whether an entry was appended.
    pub(crate) fn clear_with_retain(
        &mut self,
        history: &mut PromptHistory,
        text: &mut String,
        cursor: &mut usize,
        pasted: &mut Vec<PastedText>,
        mode: PromptMode,
    ) -> bool {
        let retained = retain_cleared_draft(history, text, *cursor, mode, pasted, &self.owner.parts);
        self.clear(text, cursor, pasted);
        retained
    }

    /// The tracked owner's parts, used by history retention and diagnostics.
    #[must_use]
    pub(crate) fn tracked(&self) -> &TrackedPrompt {
        &self.owner.parts
    }
}

/// CP-33(a): the mode carried into `SubmitContext` is derived from a leading
/// `!` so a queued shell command is refused (`QueueRejection::ShellMode`)
/// instead of being admitted as a normal prompt.
#[must_use]
pub(crate) fn mode_for_draft(draft: &str) -> PromptMode {
    if draft.starts_with('!') {
        PromptMode::Shell
    } else {
        PromptMode::Normal
    }
}

/// CP-34: the history entry retained when a live draft is cleared
/// (`index.tsx:1526-1538`). Upstream appends only when the trimmed text is
/// long enough or tracked parts exist.
pub(crate) fn retain_cleared_draft(
    history: &mut PromptHistory,
    draft: &str,
    cursor: usize,
    mode: PromptMode,
    pasted: &[PastedText],
    parts: &TrackedPrompt,
) -> bool {
    let retain = should_retain_cleared_draft(
        draft,
        pasted.len(),
        parts.files.len(),
        parts.agents.len(),
    );
    if !retain {
        return false;
    }
    history.append(HistoryEntry {
        prompt: DraftEntry {
            text: draft.to_owned(),
            cursor,
            mode,
            parts: pasted.to_vec(),
        },
    });
    true
}

/// CP-34: whether an in-focus Ctrl+C with a non-empty draft clears the
/// composer (retaining the draft) instead of falling through to the global
/// byte-exit. `overlay_owns_key` is the OR of every overlay that keeps
/// priority over the bare composer for byte 3.
#[must_use]
pub(crate) fn ctrl_c_clears_with_retain(draft_non_empty: bool, overlay_owns_key: bool) -> bool {
    draft_non_empty && !overlay_owns_key
}

/// CP-33(b): a blank-Enter promotion must commit the staged agent/model
/// before promoting the first queued prompt (`index.tsx:1365-1373`). A plain
/// empty Enter with nothing queued does not commit.
#[must_use]
pub(crate) fn commit_selection_for_promotion(
    queued: bool,
    commit_selection: bool,
    has_session: bool,
) -> bool {
    queued && commit_selection && has_session
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_paste::{FilePart, LabelPart, Mention};

    fn folded() -> PastedText {
        PastedText {
            text: "the full pasted text".to_owned(),
            placeholder: "[Pasted ~3 lines]".to_owned(),
            start: 7,
            end: 7 + "[Pasted ~3 lines]".len(),
        }
    }

    /// CP-35: the same `LiveDraft::realign` the live loop calls keeps every
    /// tracked part kind after a text edit, with mention ranges re-derived from
    /// the edited text and folded pastes re-anchored to their placeholder.
    #[test]
    fn cp35_realign_keeps_all_part_kinds_and_reanchors_them() {
        let placeholder = "[Pasted ~3 lines]";
        let mut draft = format!("@alice {placeholder} @build");
        let mut cursor = draft.len();
        let mut pasted = vec![folded()];
        let mut live = LiveDraft::new();
        assert!(live.tracked().files.is_empty());
        // Seed tracked extras the way a mention-bearing composer would.
        live.owner.parts.files.push(FilePart {
            uri: "src/main.rs".to_owned(),
            name: None,
            mention: None,
        });
        live.owner.parts.agents.push(LabelPart {
            id: "alice".to_owned(),
            mention: Some(Mention {
                text: "@alice".to_owned(),
                start: 0,
                end: 6,
            }),
        });
        live.owner.parts.skills.push(LabelPart {
            id: "build".to_owned(),
            mention: Some(Mention {
                text: "@build".to_owned(),
                start: 0,
                end: 0,
            }),
        });

        // The live sites mutate the trio before calling realign().
        draft.insert_str(0, "hey ");
        cursor += 4;
        live.realign(&mut draft, &mut cursor, &mut pasted);

        assert!(draft.starts_with("hey @alice"));
        assert_eq!(cursor, draft.len());
        // files without a mention are preserved verbatim.
        assert_eq!(live.tracked().files.len(), 1);
        assert_eq!(live.tracked().files[0].uri, "src/main.rs");
        // agent/skill mentions are re-derived against the edited text.
        assert_eq!(live.tracked().agents.len(), 1);
        assert_eq!(
            live.tracked().agents[0].mention.as_ref().map(|m| m.start),
            Some(draft.find("@alice").unwrap())
        );
        assert_eq!(live.tracked().skills.len(), 1);
        assert_eq!(
            live.tracked().skills[0].mention.as_ref().map(|m| m.start),
            Some(draft.find("@build").unwrap())
        );
        // the folded paste survives through the transport and is re-anchored.
        assert_eq!(pasted.len(), 1);
        assert_eq!(pasted[0].start, draft.find(placeholder).unwrap());
    }

    /// CP-35: a folded paste whose placeholder was edited away is dropped
    /// rather than left with a stale range.
    #[test]
    fn cp35_realign_drops_parts_that_no_longer_exist() {
        let mut draft = "[Pasted ~3 lines] done".to_owned();
        let mut cursor = draft.len();
        let mut pasted = vec![folded()];
        let mut live = LiveDraft::new();

        draft.clear();
        cursor = 0;
        live.realign(&mut draft, &mut cursor, &mut pasted);

        assert!(pasted.is_empty());
        assert!(draft.is_empty());
    }

    /// CP-34: `clear_with_retain` (the function the Ctrl+C and ESC paths call)
    /// appends a meaningful draft to real history and clears the owner.
    #[test]
    fn cp34_clear_with_retain_records_text_cursor_mode_and_parts() {
        let mut history = PromptHistory::new();
        let mut draft = "short".to_owned();
        let mut cursor = 5;
        let mut pasted = vec![folded()];
        let mut live = LiveDraft::new();

        let retained = live.clear_with_retain(
            &mut history,
            &mut draft,
            &mut cursor,
            &mut pasted,
            PromptMode::Shell,
        );

        assert!(retained);
        assert!(draft.is_empty());
        assert_eq!(cursor, 0);
        assert!(pasted.is_empty());
        assert_eq!(history.entries().len(), 1);
        let entry = &history.entries()[0].prompt;
        assert_eq!(entry.text, "short");
        assert_eq!(entry.cursor, 5);
        assert_eq!(entry.mode, PromptMode::Shell);
        assert_eq!(entry.parts, vec![folded()]);
    }

    /// CP-34: a short, plain draft is dropped from history but still cleared.
    #[test]
    fn cp34_clear_with_retain_drops_short_plain_draft() {
        let mut history = PromptHistory::new();
        let mut draft = "x".to_owned();
        let mut cursor = 1;
        let mut pasted = Vec::new();
        let mut live = LiveDraft::new();

        assert!(!live.clear_with_retain(
            &mut history,
            &mut draft,
            &mut cursor,
            &mut pasted,
            PromptMode::Normal,
        ));
        assert!(history.entries().is_empty());
        assert!(draft.is_empty());
    }

    /// CP-34: Ctrl+C clears with retention only for a non-empty draft with no
    /// overlay owner; otherwise it keeps exit priority.
    #[test]
    fn cp34_ctrl_c_clear_respects_overlay_priority() {
        assert!(ctrl_c_clears_with_retain(true, false));
        assert!(!ctrl_c_clears_with_retain(false, false));
        assert!(!ctrl_c_clears_with_retain(true, true));
        assert!(!ctrl_c_clears_with_retain(false, true));
    }

    /// CP-33(a): leading `!` maps to shell mode, everything else to normal.
    #[test]
    fn cp33_mode_for_draft_uses_leading_bang() {
        assert_eq!(mode_for_draft("!ls -la"), PromptMode::Shell);
        assert_eq!(mode_for_draft("ls -la"), PromptMode::Normal);
        assert_eq!(mode_for_draft(""), PromptMode::Normal);
    }

    /// CP-33(b): commit happens only for a real queued promotion in a session.
    #[test]
    fn cp33_commit_selection_only_for_queued_session_promotion() {
        assert!(commit_selection_for_promotion(true, true, true));
        assert!(!commit_selection_for_promotion(true, false, true));
        assert!(!commit_selection_for_promotion(false, true, true));
        assert!(!commit_selection_for_promotion(true, true, false));
    }

    /// CP-35: a history restore drops stale tracked extras and re-derives the
    /// restored parts' anchors through the same owner path.
    #[test]
    fn cp35_restore_realigns_restored_parts_and_drops_stale_extras() {
        let mut draft = "current".to_owned();
        let mut cursor = 7;
        let mut pasted = Vec::new();
        let mut live = LiveDraft::new();
        live.owner.parts.files.push(FilePart {
            uri: "stale.rs".to_owned(),
            name: None,
            mention: None,
        });

        live.restore(
            &mut draft,
            &mut cursor,
            &mut pasted,
            DraftEntry {
                text: "[Pasted ~3 lines]".to_owned(),
                cursor: 4,
                mode: PromptMode::Normal,
                parts: vec![folded()],
            },
        );

        assert_eq!(draft, "[Pasted ~3 lines]");
        assert!(live.tracked().files.is_empty(), "stale extras dropped");
        assert_eq!(pasted.len(), 1);
        assert_eq!(pasted[0].start, 0);
    }
}

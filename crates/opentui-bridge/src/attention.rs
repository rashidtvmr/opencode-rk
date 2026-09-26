#![forbid(unsafe_code)]
//! Focus + notification policy (mirrors `packages/tui/src/attention.ts`).
//!
//! TS checkout at /home/rashid/projects/opencode commit a0d9b6c (NOT the
//! pinned 95daf90; paths/lines cited below are against a0d9b6c).
//! Evidence: attention.ts:24 (`FocusState` unknown/focused/blurred),
//! plugin/src/tui.ts:233 (`TuiAttentionWhen` always/focused/blurred),
//! attention.ts:107-112 (`focusSkip` gate), attention.ts:179-181
//! (notification defaults to `when: "blurred"`), attention.ts:196-200
//! (sound defaults to `when: "always"`, `false` when muted),
//! attention.ts:202-205 (skip surfacing), attention.ts:22
//! (`subagent_done` uses `yup-01.mp3`).
//!
//! `Never` is NOT an evidenced `TuiAttentionWhen`; muting is expressed via
//! `enabled: false` / `notification: false` / `sound: false` at the call
//! site, modeled here as [`NotifySkip::Muted`].

/// Renderer focus tracking (mirrors `FocusState`, attention.ts:24,120-131).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusState {
    Unknown,
    Focused,
    Blurred,
}

/// Delivery gate (mirrors `TuiAttentionWhen`, plugin/src/tui.ts:233).
/// Only these three are evidenced; there is no `never`/`on-blur` variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttentionWhen {
    Always,
    Focused,
    Blurred,
}

/// Why a notify/sound was skipped. `Focused`/`Blurred`/`FocusUnknown`
/// mirror `focusSkip` (attention.ts:107-112); `Muted` mirrors the muted
/// paths (config `sound` off / `sound: false`, attention.ts:83-89,195);
/// `NoSound` mirrors a failed load/play (attention.ts:151-167).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotifySkip {
    Focused,
    Blurred,
    FocusUnknown,
    Muted,
    NoSound,
}

/// Terminal bell policy alongside rich notification/sound.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BellPolicy {
    Always,
    WhenSilent,
    Never,
}

/// Pure `focusSkip` gate (attention.ts:107-112): `None` means deliver.
#[must_use]
pub const fn focus_skip(when: AttentionWhen, focus: FocusState) -> Option<NotifySkip> {
    match when {
        AttentionWhen::Always => None,
        AttentionWhen::Blurred => match focus {
            FocusState::Focused => Some(NotifySkip::Focused),
            FocusState::Unknown => Some(NotifySkip::FocusUnknown),
            FocusState::Blurred => None,
        },
        AttentionWhen::Focused => match focus {
            FocusState::Blurred => Some(NotifySkip::Blurred),
            FocusState::Unknown => Some(NotifySkip::FocusUnknown),
            FocusState::Focused => None,
        },
    }
}

/// Notification gate: notification requested, default `when: "blurred"`
/// (attention.ts:179-181).
#[must_use]
pub const fn should_notify(when: AttentionWhen, focus: FocusState) -> bool {
    focus_skip(when, focus).is_none()
}

/// Sound gate: default `when: "always"`; `muted` covers config-off /
/// `sound: false` (attention.ts:83-89,195-197).
#[must_use]
pub const fn should_play_sound(when: AttentionWhen, focus: FocusState, muted: bool) -> Result<(), NotifySkip> {
    if muted {
        return Err(NotifySkip::Muted);
    }
    match focus_skip(when, focus) {
        None => Ok(()),
        Some(skip) => Err(skip),
    }
}

/// Bell rings per policy; `WhenSilent` only when neither notification nor
/// sound fired.
#[must_use]
pub const fn should_ring(policy: BellPolicy, notified: bool, sound: bool) -> bool {
    match policy {
        BellPolicy::Always => true,
        BellPolicy::Never => false,
        BellPolicy::WhenSilent => !notified && !sound,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn always_delivers_regardless_of_focus() {
        assert!(should_notify(AttentionWhen::Always, FocusState::Focused));
        assert!(should_notify(AttentionWhen::Always, FocusState::Blurred));
        assert!(should_notify(AttentionWhen::Always, FocusState::Unknown));
    }

    #[test]
    fn blurred_default_skips_when_focused() {
        assert!(!should_notify(AttentionWhen::Blurred, FocusState::Focused));
        assert!(should_notify(AttentionWhen::Blurred, FocusState::Blurred));
        assert_eq!(
            focus_skip(AttentionWhen::Blurred, FocusState::Focused),
            Some(NotifySkip::Focused)
        );
    }

    #[test]
    fn focused_skips_when_blurred_and_unknown_skips_gated() {
        assert!(!should_notify(AttentionWhen::Focused, FocusState::Blurred));
        assert!(should_notify(AttentionWhen::Focused, FocusState::Focused));
        assert_eq!(
            focus_skip(AttentionWhen::Focused, FocusState::Unknown),
            Some(NotifySkip::FocusUnknown)
        );
        assert_eq!(
            focus_skip(AttentionWhen::Blurred, FocusState::Unknown),
            Some(NotifySkip::FocusUnknown)
        );
    }

    #[test]
    fn sound_gate_muted_and_focus() {
        assert_eq!(
            should_play_sound(AttentionWhen::Always, FocusState::Blurred, true),
            Err(NotifySkip::Muted)
        );
        assert_eq!(
            should_play_sound(AttentionWhen::Always, FocusState::Blurred, false),
            Ok(())
        );
        assert_eq!(
            should_play_sound(AttentionWhen::Blurred, FocusState::Focused, false),
            Err(NotifySkip::Focused)
        );
    }

    #[test]
    fn bell_policy() {
        assert!(should_ring(BellPolicy::Always, true, true));
        assert!(!should_ring(BellPolicy::Never, false, false));
        assert!(should_ring(BellPolicy::WhenSilent, false, false));
        assert!(!should_ring(BellPolicy::WhenSilent, true, false));
        assert!(!should_ring(BellPolicy::WhenSilent, false, true));
    }
}

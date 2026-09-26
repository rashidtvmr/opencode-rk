#![forbid(unsafe_code)]
//! Sound registry + playback guard (mirrors `packages/tui/src/audio.ts`).
//!
//! TS checkout at /home/rashid/projects/opencode commit a0d9b6c (NOT the
//! pinned 95daf90; paths/lines cited below are against a0d9b6c).
//! Evidence: audio.ts:7-21 (`Audio.create({ autoStart: false })`, error
//! listener, null on failure), audio.ts:23-36 (`loadSoundFile` cached per
//! file in `sounds` map, null on load failure), audio.ts:38-43 (`play`
//! starts the context when stopped, null when start fails),
//! audio.ts:45-47 (`stopVoice`), audio.ts:49-52 (`dispose` clears sounds).
//! Sound stems evidenced in attention.ts:17-22.

use std::collections::HashMap;
use std::fmt;

/// Bounded registry size (DoS cap on cached sounds; mirrors the unbounded
/// TS `sounds` map with an explicit ceiling).
pub const MAX_SOUNDS: usize = 16;

/// Named attention sounds (mirrors `TuiAttentionSoundName`,
/// plugin/src/tui.ts:235; file stems from attention.ts:17-22).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SoundName {
    Default,
    Question,
    Permission,
    Error,
    Done,
    SubagentDone,
}

impl SoundName {
    /// File stem; `Done` reuses the default stem (attention.ts:21 assigns
    /// `doneSoundPath` the same `bip-bop-01.mp3` as default).
    #[must_use]
    pub const fn stem(self) -> &'static str {
        match self {
            Self::Default => "bip-bop-01",
            Self::Question => "bip-bop-03",
            Self::Permission => "staplebops-06",
            Self::Error => "nope-03",
            Self::Done => "bip-bop-01",
            Self::SubagentDone => "yup-01",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SoundError {
    OverCapacity,
    NotLoaded,
    Stopped,
}

impl fmt::Display for SoundError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OverCapacity => write!(f, "sound registry full"),
            Self::NotLoaded => write!(f, "sound not loaded"),
            Self::Stopped => write!(f, "audio not started"),
        }
    }
}

impl std::error::Error for SoundError {}

/// Bounded file-path cache keyed by [`SoundName`] (mirrors the TS `sounds`
/// map, audio.ts:5,23-36, with an explicit [`MAX_SOUNDS`] ceiling).
#[derive(Debug, Default)]
pub struct SoundRegistry {
    entries: HashMap<SoundName, String>,
}

impl SoundRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Cache a sound file path; fail-closed at [`MAX_SOUNDS`] (re-register
    /// of a known name refreshes in place and never counts against the cap).
    pub fn register(&mut self, name: SoundName, file: String) -> Result<(), SoundError> {
        if !self.entries.contains_key(&name) && self.entries.len() >= MAX_SOUNDS {
            return Err(SoundError::OverCapacity);
        }
        self.entries.insert(name, file);
        Ok(())
    }

    #[must_use]
    pub fn get(&self, name: SoundName) -> Option<&str> {
        self.entries.get(&name).map(String::as_str)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

/// Playback lifecycle (mirrors `Audio.create({ autoStart: false })` +
/// lazy `start()` in `play`, audio.ts:10,38-43).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioState {
    Started,
    Stopped,
}

#[derive(Debug)]
pub struct Audio {
    state: AudioState,
    registry: SoundRegistry,
}

impl Audio {
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: AudioState::Stopped,
            registry: SoundRegistry::new(),
        }
    }

    /// Start the context (mirrors `current.start()`, audio.ts:41).
    pub fn start(&mut self) -> bool {
        self.state = AudioState::Started;
        true
    }

    /// Dispose the context and drop cached sounds (audio.ts:49-52).
    pub fn stop(&mut self) {
        self.state = AudioState::Stopped;
        self.registry.clear();
    }

    #[must_use]
    pub fn is_started(&self) -> bool {
        self.state == AudioState::Started
    }

    #[must_use]
    pub fn registry(&self) -> &SoundRegistry {
        &self.registry
    }

    pub fn registry_mut(&mut self) -> &mut SoundRegistry {
        &mut self.registry
    }

    /// Guarded play: false unless started AND the sound is cached
    /// (mirrors the null-guards in `play`, audio.ts:38-43).
    #[must_use]
    pub fn play(&self, name: SoundName) -> bool {
        self.is_started() && self.registry.get(name).is_some()
    }
}

impl Default for Audio {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stems_match_attention_sources() {
        assert_eq!(SoundName::Default.stem(), "bip-bop-01");
        assert_eq!(SoundName::Question.stem(), "bip-bop-03");
        assert_eq!(SoundName::Permission.stem(), "staplebops-06");
        assert_eq!(SoundName::Error.stem(), "nope-03");
        assert_eq!(SoundName::Done.stem(), "bip-bop-01");
        assert_eq!(SoundName::SubagentDone.stem(), "yup-01");
    }

    #[test]
    fn registry_register_get_clear() {
        let mut registry = SoundRegistry::new();
        assert!(registry.is_empty());
        registry.register(SoundName::Default, "bip-bop-01.mp3".to_string()).unwrap();
        assert_eq!(registry.get(SoundName::Default), Some("bip-bop-01.mp3"));
        assert_eq!(registry.len(), 1);
        registry.clear();
        assert!(registry.is_empty());
        assert_eq!(registry.get(SoundName::Default), None);
    }

    #[test]
    fn registry_bound_enforced() {
        // ponytail: only 6 SoundName variants evidenced; cap unreachable via
        // public API, so assert re-register stays flat and len <= MAX_SOUNDS.
        let mut registry = SoundRegistry::new();
        let names = [
            SoundName::Default,
            SoundName::Question,
            SoundName::Permission,
            SoundName::Error,
            SoundName::Done,
            SoundName::SubagentDone,
        ];
        for name in names {
            registry.register(name, format!("{}.mp3", name.stem())).unwrap();
        }
        assert_eq!(registry.len(), names.len());
        assert!(registry.len() <= MAX_SOUNDS);
        registry.register(SoundName::Default, "other.mp3".to_string()).unwrap();
        assert_eq!(registry.len(), names.len());
        assert_eq!(registry.get(SoundName::Default), Some("other.mp3"));
    }

    #[test]
    fn play_guarded_by_start_and_cache() {
        let mut audio = Audio::new();
        assert!(!audio.is_started());
        assert!(!audio.play(SoundName::Default));
        audio.start();
        assert!(!audio.play(SoundName::Default));
        audio
            .registry_mut()
            .register(SoundName::Default, "bip-bop-01.mp3".to_string())
            .unwrap();
        assert!(audio.play(SoundName::Default));
        audio.stop();
        assert!(!audio.is_started());
        assert!(!audio.play(SoundName::Default));
    }

    #[test]
    fn stop_clears_cache_like_dispose() {
        let mut audio = Audio::new();
        audio.start();
        audio
            .registry_mut()
            .register(SoundName::Error, "nope-03.mp3".to_string())
            .unwrap();
        audio.stop();
        assert!(audio.registry().is_empty());
    }
}

//! WEB-016: bounded voice-capture session contract (server half).
//!
//! Native state machine for dictation/voice capture owned by the server
//! boundary. Browser audio plumbing in `web/` is owned elsewhere; this module
//! tracks permission/adapter gating, explicit microphone state, an editable
//! text transcript, and bounded lifecycle resources. Text chat stays usable in
//! every state: voice is never the only interaction path.
//!
//! Bounds: utterance count and transcript chars are capped by [`VoiceConfig`];
//! capture duration is capped by `max_capture_secs`; reconnects are capped by
//! `max_reconnects`. Reconnect never duplicates utterances (segment ids are
//! retained across reconnects). Mute/stop/release drop live media tracks
//! promptly (`live_tracks()` falls to zero) while the transcript persists.
//! Errors carry fixed redacted templates and never echo transcript bytes.

/// Lifecycle bounds for one voice session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VoiceConfig {
    /// Maximum retained utterances per session.
    pub max_utterances: usize,
    /// Maximum retained transcript chars across all utterances.
    pub max_transcript_chars: usize,
    /// Maximum capture duration in seconds before forced stop.
    pub max_capture_secs: u64,
    /// Maximum reconnects before [`VoiceError::TooManyReconnects`].
    pub max_reconnects: u32,
}

impl Default for VoiceConfig {
    fn default() -> Self {
        Self {
            max_utterances: 256,
            max_transcript_chars: 65_536,
            max_capture_secs: 3_600,
            max_reconnects: 3,
        }
    }
}

impl VoiceConfig {
    /// Hard ceiling on retained transcript bytes (worst case, normally small).
    #[must_use]
    pub fn byte_cap(&self) -> usize {
        self.max_utterances.saturating_mul(256) + self.max_transcript_chars
    }
}

/// Microphone permission/device grant presented by the browser layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MicGrant {
    /// Microphone permission granted and a device is present.
    Granted,
    /// Microphone permission denied by the user.
    Denied,
    /// Permission granted but no capture device exists.
    NoDevice,
}

/// Explicit microphone state. Always paired with a text label for non-audio UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MicState {
    /// Capture configured but not running.
    Idle,
    /// Live capture; one media track is held.
    Capturing,
    /// Muted by keyboard/control; tracks released, session retained.
    Muted,
    /// Stopped; tracks released, transcript retained.
    Stopped,
}

/// Voice-session failures. Fixed templates only; never echo transcript bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceError {
    /// Microphone permission denied; nothing was captured.
    PermissionDenied,
    /// No capture device present.
    NoDevice,
    /// No native audio/realtime adapter installed.
    NoAdapter,
    /// Capture is muted; speech frames are refused, text chat still works.
    Muted,
    /// Capture is not running (idle or stopped).
    NotCapturing,
    /// Utterance or transcript bound exceeded.
    TooLarge,
    /// Capture ran past `max_capture_secs`; tracks released.
    CaptureTooLong,
    /// Reconnect budget exhausted.
    TooManyReconnects,
    /// Segment id already retained; reconnect must not duplicate it.
    Duplicate,
    /// Segment id unknown to this session.
    UnknownSegment,
}

impl std::fmt::Display for VoiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PermissionDenied => write!(f, "microphone permission denied"),
            Self::NoDevice => write!(f, "no microphone device present"),
            Self::NoAdapter => write!(f, "no native audio adapter installed"),
            Self::Muted => write!(f, "capture is muted"),
            Self::NotCapturing => write!(f, "capture is not running"),
            Self::TooLarge => write!(f, "voice buffer bound exceeded"),
            Self::CaptureTooLong => write!(f, "capture duration bound exceeded"),
            Self::TooManyReconnects => write!(f, "reconnect bound exceeded"),
            Self::Duplicate => write!(f, "duplicate voice segment refused"),
            Self::UnknownSegment => write!(f, "unknown voice segment"),
        }
    }
}

impl std::error::Error for VoiceError {}

/// Bounded voice-capture session: explicit mic state, editable transcript.
#[derive(Debug)]
pub struct VoiceSession {
    config: VoiceConfig,
    grant: MicGrant,
    adapter: bool,
    state: MicState,
    segments: Vec<(u64, String)>,
    transcript_chars: usize,
    elapsed_secs: u64,
    reconnects: u32,
    released: bool,
    ever_started: bool,
}

impl VoiceSession {
    /// Create an idle session. `adapter` reports whether a real native
    /// audio/realtime adapter exists; without one capture stays explicit.
    #[must_use]
    pub fn new(config: VoiceConfig, grant: MicGrant, adapter: bool) -> Self {
        Self {
            config,
            grant,
            adapter,
            state: MicState::Idle,
            segments: Vec::new(),
            transcript_chars: 0,
            elapsed_secs: 0,
            reconnects: 0,
            released: true,
            ever_started: false,
        }
    }

    /// Current explicit microphone state.
    #[must_use]
    pub fn state(&self) -> MicState {
        self.state
    }

    /// Non-audio text label for the current state (screen-reader UI).
    #[must_use]
    pub fn state_label(&self) -> &'static str {
        match self.state {
            MicState::Idle => "microphone idle",
            MicState::Capturing => "capturing audio",
            MicState::Muted => "microphone muted",
            MicState::Stopped => "voice stopped",
        }
    }

    /// Non-audio text label for the microphone control.
    #[must_use]
    pub fn mic_label(&self) -> String {
        format!("Microphone: {}", self.state_label())
    }

    /// Text labels for the start/stop/mute controls (all distinct).
    #[must_use]
    pub fn control_labels(&self) -> [&'static str; 3] {
        ["Start dictation", "Stop dictation", "Mute microphone"]
    }

    /// Voice is never the only path: text chat stays usable in every state.
    #[must_use]
    pub fn text_chat_usable(&self) -> bool {
        true
    }

    /// Live media tracks currently held (1 while capturing, else 0).
    #[must_use]
    pub fn live_tracks(&self) -> usize {
        if self.state == MicState::Capturing {
            1
        } else {
            0
        }
    }

    /// Whether capture resources are currently released.
    #[must_use]
    pub fn released(&self) -> bool {
        self.released
    }

    /// Retained utterance count.
    #[must_use]
    pub fn utterance_count(&self) -> usize {
        self.segments.len()
    }

    /// Visible editable transcript (utterances joined by newline).
    #[must_use]
    pub fn transcript(&self) -> String {
        self.segments
            .iter()
            .map(|(_, text)| text.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Bounded retention view: last `max_chars` of the transcript.
    #[must_use]
    pub fn retained_transcript(&self, max_chars: usize) -> String {
        let full = self.transcript();
        let chars: Vec<char> = full.chars().collect();
        if chars.len() <= max_chars {
            return full;
        }
        chars[chars.len() - max_chars..].iter().collect()
    }

    /// Start (or restart) capture. Permission/adapter failures stay explicit
    /// and retain no transcript side effects.
    pub fn start(&mut self) -> Result<(), VoiceError> {
        match self.grant {
            MicGrant::Denied => return Err(VoiceError::PermissionDenied),
            MicGrant::NoDevice => return Err(VoiceError::NoDevice),
            MicGrant::Granted => {}
        }
        if !self.adapter {
            return Err(VoiceError::NoAdapter);
        }
        self.state = MicState::Capturing;
        self.released = false;
        self.ever_started = true;
        Ok(())
    }

    /// Keyboard mute: pauses capture and releases live tracks immediately.
    pub fn mute(&mut self) -> Result<(), VoiceError> {
        if self.state != MicState::Capturing {
            return Err(VoiceError::NotCapturing);
        }
        self.state = MicState::Muted;
        Ok(())
    }

    /// Keyboard unmute: resumes capture when permission/adapter still hold.
    pub fn unmute(&mut self) -> Result<(), VoiceError> {
        if self.state != MicState::Muted {
            return Err(VoiceError::NotCapturing);
        }
        match self.grant {
            MicGrant::Denied => return Err(VoiceError::PermissionDenied),
            MicGrant::NoDevice => return Err(VoiceError::NoDevice),
            MicGrant::Granted => {}
        }
        if !self.adapter {
            return Err(VoiceError::NoAdapter);
        }
        self.state = MicState::Capturing;
        Ok(())
    }

    /// Stop capture and release live tracks promptly; transcript persists.
    pub fn stop(&mut self) {
        self.state = MicState::Stopped;
        self.released = true;
    }

    /// Unmount path: identical to stop (release tracks, keep transcript).
    pub fn release(&mut self) {
        self.stop();
    }

    /// Accept one transcribed segment. Duplicates (reconnect replays) are
    /// refused without growing the buffer; oversize input is refused.
    pub fn push_utterance(&mut self, id: u64, text: &str) -> Result<(), VoiceError> {
        match self.grant {
            MicGrant::Denied => return Err(VoiceError::PermissionDenied),
            MicGrant::NoDevice => return Err(VoiceError::NoDevice),
            MicGrant::Granted => {}
        }
        if !self.adapter {
            return Err(VoiceError::NoAdapter);
        }
        if self.state == MicState::Muted {
            return Err(VoiceError::Muted);
        }
        if self.state != MicState::Capturing {
            return Err(VoiceError::NotCapturing);
        }
        if self.segments.iter().any(|(sid, _)| *sid == id) {
            return Err(VoiceError::Duplicate);
        }
        let chars = text.chars().count();
        if text.is_empty()
            || self.segments.len() >= self.config.max_utterances.max(1)
            || self.transcript_chars.saturating_add(chars) > self.config.max_transcript_chars
        {
            return Err(VoiceError::TooLarge);
        }
        self.transcript_chars = self.transcript_chars.saturating_add(chars);
        self.segments.push((id, text.to_string()));
        Ok(())
    }

    /// Edit a retained segment in place (dictation correction); the original
    /// assistant message is never rewritten here, only this transcript copy.
    pub fn edit_transcript(&mut self, id: u64, text: &str) -> Result<(), VoiceError> {
        let slot = self
            .segments
            .iter_mut()
            .find(|(sid, _)| *sid == id)
            .ok_or(VoiceError::UnknownSegment)?;
        let next = text.chars().count();
        let prev = slot.1.chars().count();
        let total = self
            .transcript_chars
            .saturating_sub(prev)
            .saturating_add(next);
        if text.is_empty() || total > self.config.max_transcript_chars {
            return Err(VoiceError::TooLarge);
        }
        slot.1 = text.to_string();
        self.transcript_chars = total;
        Ok(())
    }

    /// Advance the capture clock. Past `max_capture_secs` the session stops
    /// and releases tracks, reporting [`VoiceError::CaptureTooLong`].
    pub fn advance_secs(&mut self, secs: u64) -> Result<(), VoiceError> {
        self.elapsed_secs = self.elapsed_secs.saturating_add(secs);
        if self.elapsed_secs > self.config.max_capture_secs {
            self.stop();
            return Err(VoiceError::CaptureTooLong);
        }
        Ok(())
    }

    /// Reconnect within the bounded budget. Retained segments survive, so a
    /// replayed segment id reports [`VoiceError::Duplicate`] instead of
    /// duplicating the transcript.
    pub fn reconnect(&mut self) -> Result<(), VoiceError> {
        if self.reconnects >= self.config.max_reconnects {
            return Err(VoiceError::TooManyReconnects);
        }
        self.reconnects += 1;
        self.elapsed_secs = 0;
        self.state = MicState::Idle;
        self.released = true;
        Ok(())
    }

    /// Whether this session ever entered capture (lifecycle witness).
    #[must_use]
    pub fn ever_started(&self) -> bool {
        self.ever_started
    }
}

//! Remote turn controls for phone-originated coding turns.
//!
//! Phone envelopes carry the same prompt plus an idempotency key
//! (`submission_id`) and a model/effort/agent [`Selection`]. The controller
//! maps every envelope to one [`EngineRequest`] independent of origin, dedups
//! retries by `submission_id`, routes interrupts to the intended turn, and
//! fails unauthorized/offline devices loudly instead of queueing invisibly.
//!
//! Bounds: [`MAX_TURNS`] live turns, [`MAX_PROMPT_BYTES`] per prompt,
//! [`MAX_HISTORY_ENTRIES`] transcript lines per turn. Std only.
#![forbid(unsafe_code)]

use std::collections::{HashMap, HashSet};
use std::fmt;

/// Max prompt bytes per envelope.
pub const MAX_PROMPT_BYTES: usize = 32_768;
/// Max live turns retained.
pub const MAX_TURNS: usize = 256;
/// Max bytes for submission and device ids.
pub const MAX_ID_LEN: usize = 128;
/// Max bytes per model/effort/agent field.
pub const MAX_SELECTION_FIELD_LEN: usize = 64;
/// Max transcript entries kept per turn (prompt + interrupt markers).
pub const MAX_HISTORY_ENTRIES: usize = 64;

/// Selectable models (closed allowlist).
pub const ALLOWED_MODELS: &[&str] = &["default", "mini", "pro"];
/// Selectable effort levels (closed allowlist).
pub const ALLOWED_EFFORTS: &[&str] = &["low", "medium", "high"];
/// Selectable subagents (closed allowlist).
pub const ALLOWED_AGENTS: &[&str] = &["general", "coder", "reviewer"];

/// Where a prompt came from. Phone variants always carry the device id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Origin {
    Local,
    Phone { device_id: String },
}

impl Origin {
    /// Device key used for auth, online state, and selection persistence.
    pub fn device_key(&self) -> &str {
        match self {
            Self::Local => "local",
            Self::Phone { device_id } => device_id,
        }
    }

    pub fn is_phone(&self) -> bool {
        matches!(self, Self::Phone { .. })
    }
}

impl fmt::Display for Origin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Local => write!(f, "local"),
            Self::Phone { device_id } => write!(f, "phone:{device_id}"),
        }
    }
}

/// Model/effort/agent selection. Stored per device as the persisting marker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    pub model: String,
    pub effort: String,
    pub agent: String,
}

impl Default for Selection {
    fn default() -> Self {
        Self {
            model: "default".to_string(),
            effort: "medium".to_string(),
            agent: "general".to_string(),
        }
    }
}

impl Selection {
    pub fn validate(&self) -> Result<(), TurnError> {
        if self.model.len() > MAX_SELECTION_FIELD_LEN
            || !ALLOWED_MODELS.contains(&self.model.as_str())
        {
            return Err(TurnError::UnknownModel {
                value: self.model.clone(),
            });
        }
        if self.effort.len() > MAX_SELECTION_FIELD_LEN
            || !ALLOWED_EFFORTS.contains(&self.effort.as_str())
        {
            return Err(TurnError::UnknownEffort {
                value: self.effort.clone(),
            });
        }
        if self.agent.len() > MAX_SELECTION_FIELD_LEN
            || !ALLOWED_AGENTS.contains(&self.agent.as_str())
        {
            return Err(TurnError::UnknownAgent {
                value: self.agent.clone(),
            });
        }
        Ok(())
    }
}

/// Inbound prompt envelope. `submission_id` is the idempotency key: resending
/// the same id returns the original receipt and never creates a second turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptEnvelope {
    pub origin: Origin,
    pub submission_id: String,
    pub prompt: String,
    pub selection: Selection,
}

impl PromptEnvelope {
    pub fn validate(&self) -> Result<(), TurnError> {
        if !valid_id(&self.submission_id) {
            return Err(TurnError::BadSubmissionId);
        }
        if self.prompt.is_empty() {
            return Err(TurnError::EmptyPrompt);
        }
        if self.prompt.len() > MAX_PROMPT_BYTES {
            return Err(TurnError::PromptTooLarge {
                bytes: self.prompt.len(),
                max: MAX_PROMPT_BYTES,
            });
        }
        if let Origin::Phone { device_id } = &self.origin {
            if !valid_id(device_id) {
                return Err(TurnError::BadDeviceId {
                    value: device_id.clone(),
                });
            }
        }
        self.selection.validate()
    }

    /// Origin-stripped engine input. Phone and local envelopes with equal
    /// prompt/selection map to equal requests: one engine, one transcript.
    pub fn engine_request(&self) -> EngineRequest {
        EngineRequest {
            prompt: self.prompt.clone(),
            model: self.selection.model.clone(),
            effort: self.selection.effort.clone(),
            agent: self.selection.agent.clone(),
        }
    }
}

/// What the coding engine actually runs. No origin, no device, no keys.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineRequest {
    pub prompt: String,
    pub model: String,
    pub effort: String,
    pub agent: String,
}

/// Live turn lifecycle. Interrupted turns keep history for recovery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnStatus {
    Running,
    Interrupted,
    Done,
}

/// One accepted turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Turn {
    pub id: u64,
    pub submission_id: String,
    pub origin: Origin,
    pub prompt: String,
    pub selection: Selection,
    pub status: TurnStatus,
    pub history: Vec<String>,
}

/// Receipt for [`RemoteTurns::submit`]. `deduped` marks idempotent replays.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmitReceipt {
    pub turn_id: u64,
    pub deduped: bool,
    pub selection: Selection,
}

/// Receipt for [`RemoteTurns::interrupt`]. Proves which turn was stopped and
/// how much transcript survived.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterruptReceipt {
    pub turn_id: u64,
    pub preserved_history_len: usize,
}

/// Typed failures. Messages name the device/turn so callers can show them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TurnError {
    Unauthorized { device: String },
    Offline { device: String },
    EmptyPrompt,
    PromptTooLarge { bytes: usize, max: usize },
    BadSubmissionId,
    BadDeviceId { value: String },
    UnknownModel { value: String },
    UnknownEffort { value: String },
    UnknownAgent { value: String },
    UnknownTurn { id: u64 },
    TooManyTurns { max: usize },
    HistoryFull { max: usize },
}

impl fmt::Display for TurnError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unauthorized { device } => {
                write!(f, "unauthorized device: {device}")
            }
            Self::Offline { device } => write!(
                f,
                "device offline: {device}; prompt rejected, nothing queued"
            ),
            Self::EmptyPrompt => write!(f, "prompt is empty"),
            Self::PromptTooLarge { bytes, max } => {
                write!(f, "prompt too large: {bytes} bytes, max {max}")
            }
            Self::BadSubmissionId => write!(f, "bad submission id"),
            Self::BadDeviceId { value } => write!(f, "bad device id: {value}"),
            Self::UnknownModel { value } => write!(f, "unknown model: {value}"),
            Self::UnknownEffort { value } => write!(f, "unknown effort: {value}"),
            Self::UnknownAgent { value } => write!(f, "unknown agent: {value}"),
            Self::UnknownTurn { id } => write!(f, "unknown turn: {id}"),
            Self::TooManyTurns { max } => write!(f, "too many turns: max {max}"),
            Self::HistoryFull { max } => write!(f, "turn history full: max {max}"),
        }
    }
}

impl std::error::Error for TurnError {}

fn valid_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= MAX_ID_LEN
        && s.bytes().all(|b| {
            b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.' || b == b':'
        })
}

/// Owns turns, idempotency keys, auth, online state, and per-device
/// selections. Synchronous, no threads, no I/O, bounded memory.
#[derive(Debug, Default)]
pub struct RemoteTurns {
    turns: Vec<Turn>,
    by_submission: HashMap<String, u64>,
    authorized: HashSet<String>,
    online: HashMap<String, bool>,
    last_selection: HashMap<String, Selection>,
    next_id: u64,
}

impl RemoteTurns {
    pub fn new() -> Self {
        Self {
            turns: Vec::new(),
            by_submission: HashMap::new(),
            authorized: HashSet::new(),
            online: HashMap::new(),
            last_selection: HashMap::new(),
            next_id: 1,
        }
    }

    /// Authorize a phone device. Newly authorized devices start online.
    pub fn authorize(&mut self, device_id: &str) -> Result<(), TurnError> {
        if !valid_id(device_id) {
            return Err(TurnError::BadDeviceId {
                value: device_id.to_string(),
            });
        }
        self.authorized.insert(device_id.to_string());
        self.online.insert(device_id.to_string(), true);
        Ok(())
    }

    /// Flip per-device connectivity. Returns false for unknown devices.
    pub fn set_device_online(&mut self, device_id: &str, online: bool) -> bool {
        if !self.authorized.contains(device_id) {
            return false;
        }
        self.online.insert(device_id.to_string(), online);
        true
    }

    pub fn is_authorized(&self, device_id: &str) -> bool {
        self.authorized.contains(device_id)
    }

    pub fn is_online(&self, device_id: &str) -> bool {
        self.online.get(device_id).copied().unwrap_or(false)
    }

    fn check_phone(&self, device_id: &str) -> Result<(), TurnError> {
        if !self.authorized.contains(device_id) {
            return Err(TurnError::Unauthorized {
                device: device_id.to_string(),
            });
        }
        Ok(())
    }

    /// Accept a prompt envelope. Offline phones get [`TurnError::Offline`]
    /// and nothing is stored or queued. A repeated `submission_id` returns
    /// the original turn's receipt with `deduped: true`.
    pub fn submit(&mut self, env: PromptEnvelope) -> Result<SubmitReceipt, TurnError> {
        env.validate()?;
        if let Origin::Phone { device_id } = &env.origin {
            self.check_phone(device_id)?;
            if !self.is_online(device_id) {
                return Err(TurnError::Offline {
                    device: device_id.clone(),
                });
            }
        }
        if let Some(&id) = self.by_submission.get(&env.submission_id) {
            let turn = self
                .turns
                .iter()
                .find(|t| t.id == id)
                .expect("submission index always points at a live turn");
            return Ok(SubmitReceipt {
                turn_id: id,
                deduped: true,
                selection: turn.selection.clone(),
            });
        }
        if self.turns.len() >= MAX_TURNS {
            return Err(TurnError::TooManyTurns { max: MAX_TURNS });
        }
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1).max(1);
        self.last_selection
            .insert(env.origin.device_key().to_string(), env.selection.clone());
        self.turns.push(Turn {
            id,
            submission_id: env.submission_id.clone(),
            origin: env.origin.clone(),
            prompt: env.prompt.clone(),
            selection: env.selection.clone(),
            status: TurnStatus::Running,
            history: vec![format!("prompt: {}", env.prompt)],
        });
        self.by_submission.insert(env.submission_id.clone(), id);
        Ok(SubmitReceipt {
            turn_id: id,
            deduped: false,
            selection: env.selection,
        })
    }

    /// Stop the intended turn. Unauthorized callers get
    /// [`TurnError::Unauthorized`] with zero side effects. Interrupt is
    /// idempotent: repeating it returns the same receipt.
    pub fn interrupt(
        &mut self,
        origin: &Origin,
        turn_id: u64,
    ) -> Result<InterruptReceipt, TurnError> {
        if let Origin::Phone { device_id } = origin {
            self.check_phone(device_id)?;
        }
        let turn = self
            .turns
            .iter_mut()
            .find(|t| t.id == turn_id)
            .ok_or(TurnError::UnknownTurn { id: turn_id })?;
        if turn.status != TurnStatus::Interrupted {
            if turn.history.len() >= MAX_HISTORY_ENTRIES {
                return Err(TurnError::HistoryFull {
                    max: MAX_HISTORY_ENTRIES,
                });
            }
            turn.history
                .push(format!("interrupt: requested by {origin}"));
            turn.status = TurnStatus::Interrupted;
        }
        Ok(InterruptReceipt {
            turn_id,
            preserved_history_len: turn.history.len(),
        })
    }

    /// Last accepted selection for a device key (`Origin::device_key`).
    /// This is the persisting marker: it survives across turns.
    pub fn selection_for(&self, device_key: &str) -> Option<&Selection> {
        self.last_selection.get(device_key)
    }

    pub fn turn(&self, id: u64) -> Option<&Turn> {
        self.turns.iter().find(|t| t.id == id)
    }

    pub fn history(&self, id: u64) -> Option<&[String]> {
        self.turn(id).map(|t| t.history.as_slice())
    }

    pub fn turn_count(&self) -> usize {
        self.turns.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEV: &str = "phone-1";

    fn controller() -> RemoteTurns {
        let mut c = RemoteTurns::new();
        c.authorize(DEV).expect("fixture device authorizes");
        c
    }

    fn phone_env(sub: &str, prompt: &str) -> PromptEnvelope {
        PromptEnvelope {
            origin: Origin::Phone {
                device_id: DEV.to_string(),
            },
            submission_id: sub.to_string(),
            prompt: prompt.to_string(),
            selection: Selection::default(),
        }
    }

    fn local_env(sub: &str, prompt: &str) -> PromptEnvelope {
        PromptEnvelope {
            origin: Origin::Local,
            submission_id: sub.to_string(),
            prompt: prompt.to_string(),
            selection: Selection::default(),
        }
    }

    #[test]
    fn phone_and_local_share_engine_input() {
        let phone = phone_env("sub-1", "fix login bug");
        let local = local_env("sub-2", "fix login bug");
        assert_eq!(phone.engine_request(), local.engine_request());
        let c = controller();
        let r1 = c
            .turns
            .len()
            .checked_add(0)
            .expect("len math cannot overflow");
        assert_eq!(r1, 0);
        let mut c = controller();
        let p = c.submit(phone).expect("phone submit works");
        let l = c.submit(local).expect("local submit works");
        assert_ne!(p.turn_id, l.turn_id);
        assert_eq!(c.turn(p.turn_id).unwrap().prompt, c.turn(l.turn_id).unwrap().prompt);
    }

    #[test]
    fn selection_persists_and_shapes_engine_request() {
        let mut c = controller();
        let sel = Selection {
            model: "pro".to_string(),
            effort: "high".to_string(),
            agent: "coder".to_string(),
        };
        let mut env = phone_env("sub-sel", "refactor router");
        env.selection = sel.clone();
        let receipt = c.submit(env.clone()).expect("submit works");
        assert_eq!(receipt.selection, sel);
        assert_eq!(c.selection_for(DEV), Some(&sel));
        assert_eq!(env.engine_request().model, "pro");
        assert_eq!(env.engine_request().effort, "high");
        assert_eq!(env.engine_request().agent, "coder");
        // Marker survives a second turn with default selection overwritten.
        let env2 = phone_env("sub-sel-2", "next task");
        c.submit(env2).expect("second submit works");
        assert_eq!(
            c.selection_for(DEV),
            Some(&Selection::default()),
            "latest accepted selection is the persisted marker"
        );
    }

    #[test]
    fn duplicate_submission_id_creates_one_turn() {
        let mut c = controller();
        let first = c.submit(phone_env("dup-1", "do work")).expect("first works");
        assert!(!first.deduped);
        let second = c.submit(phone_env("dup-1", "do work")).expect("replay works");
        assert!(second.deduped);
        assert_eq!(first.turn_id, second.turn_id);
        assert_eq!(c.turn_count(), 1);
    }

    #[test]
    fn duplicate_id_with_different_prompt_still_one_turn() {
        let mut c = controller();
        let first = c.submit(phone_env("dup-2", "original")).expect("first works");
        let replay = c
            .submit(phone_env("dup-2", "attacker-changed-prompt"))
            .expect("replay returns receipt, never a new turn");
        assert!(replay.deduped);
        assert_eq!(replay.turn_id, first.turn_id);
        assert_eq!(c.turn_count(), 1);
        assert_eq!(c.turn(first.turn_id).unwrap().prompt, "original");
    }

    #[test]
    fn interrupt_hits_intended_turn_and_preserves_history() {
        let mut c = controller();
        let a = c.submit(phone_env("t-a", "task a")).expect("a works").turn_id;
        let b = c.submit(phone_env("t-b", "task b")).expect("b works").turn_id;
        let origin = Origin::Phone {
            device_id: DEV.to_string(),
        };
        let receipt = c.interrupt(&origin, a).expect("interrupt works");
        assert_eq!(receipt.turn_id, a);
        assert!(receipt.preserved_history_len >= 2);
        let ha = c.history(a).unwrap();
        assert!(ha[0].contains("task a"), "original prompt entry survives");
        assert_eq!(c.turn(a).unwrap().status, TurnStatus::Interrupted);
        assert_eq!(c.turn(b).unwrap().status, TurnStatus::Running);
        // Idempotent repeat.
        let again = c.interrupt(&origin, a).expect("repeat works");
        assert_eq!(again.turn_id, a);
        assert_eq!(c.turn(b).unwrap().status, TurnStatus::Running);
    }

    #[test]
    fn unauthorized_phone_submit_errors_without_side_effect() {
        let mut c = controller();
        let env = PromptEnvelope {
            origin: Origin::Phone {
                device_id: "intruder".to_string(),
            },
            submission_id: "evil-1".to_string(),
            prompt: "rm -rf /".to_string(),
            selection: Selection::default(),
        };
        let err = c.submit(env).expect_err("intruder must fail");
        assert_eq!(
            err,
            TurnError::Unauthorized {
                device: "intruder".to_string()
            }
        );
        assert!(format!("{err}").contains("unauthorized"));
        assert_eq!(c.turn_count(), 0, "denied submit stores nothing");
        assert!(c.selection_for("intruder").is_none());
    }

    #[test]
    fn unauthorized_interrupt_errors_without_side_effect() {
        let mut c = controller();
        let id = c.submit(phone_env("t-victim", "victim task")).expect("works").turn_id;
        let evil = Origin::Phone {
            device_id: "intruder".to_string(),
        };
        let err = c.interrupt(&evil, id).expect_err("intruder interrupt fails");
        assert!(format!("{err}").contains("unauthorized"));
        assert_eq!(c.turn(id).unwrap().status, TurnStatus::Running);
        assert_eq!(c.history(id).unwrap().len(), 1);
    }

    #[test]
    fn offline_submit_errors_and_never_queues() {
        let mut c = controller();
        assert!(c.set_device_online(DEV, false));
        let err = c
            .submit(phone_env("off-1", "queued invisibly?"))
            .expect_err("offline submit must fail");
        assert_eq!(
            err,
            TurnError::Offline {
                device: DEV.to_string()
            }
        );
        assert!(format!("{err}").contains("offline"));
        assert!(format!("{err}").contains("nothing queued"));
        assert_eq!(c.turn_count(), 0, "offline prompt leaves no turn");
        assert!(c.selection_for(DEV).is_none(), "offline prompt persists nothing");
        // Back online: same id succeeds exactly once.
        assert!(c.set_device_online(DEV, true));
        let r = c.submit(phone_env("off-1", "queued invisibly?")).expect("works");
        assert!(!r.deduped);
        assert_eq!(c.turn_count(), 1);
    }

    #[test]
    fn unknown_turn_and_bad_selection_rejected() {
        let mut c = controller();
        let origin = Origin::Phone {
            device_id: DEV.to_string(),
        };
        assert_eq!(
            c.interrupt(&origin, 999).expect_err("missing turn fails"),
            TurnError::UnknownTurn { id: 999 }
        );
        let mut bad = phone_env("bad-model", "x");
        bad.selection.model = "godmode".to_string();
        assert_eq!(
            c.submit(bad).expect_err("bad model fails"),
            TurnError::UnknownModel {
                value: "godmode".to_string()
            }
        );
        assert_eq!(c.turn_count(), 0);
    }
}

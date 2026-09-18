//! PAR-008: web turn adapter capability + gating contract.
//!
//! Source evidence (repo 5af7884):
//! - `crates/server/src/lib.rs:139-199` (`web_capabilities`): tools,
//!   plugins, approvals report `available_for_web_turn: false`; attachments
//!   report `draft_ingest: true` but `available_for_web_turn: false`; search /
//!   deep_research / voice report `available: false`; artifacts report
//!   `available: true, editing_available: true, run/apply: false`.
//! - `crates/server/src/web_attachments.rs`: bounded draft ingest store
//!   (ingest path), separate from any provider transmit path.
//! - `crates/server/src/web_artifact.rs:400-411` (`authorize_run`):
//!   run/apply need executor AND explicit grant; `web_tool_chooser.rs`
//!   approvals gate tool use.
//! - `crates/server/src/voice_capture.rs:230-243` (`VoiceSession::start`):
//!   capture without adapter/grant fails explicitly.
//!
//! This module mirrors those flags through caller-supplied availability
//! instead of hardcoded success, and separates ingest (store bytes) from
//! transmit (forward to provider only when authorized), edit/version
//! (durable, store-gated) from run/apply (executor + grant gated), and
//! marks voice/search/research explicitly unavailable until real adapters
//! land. std only, no I/O, no clock, no threads.
#![forbid(unsafe_code)]

/// Max bytes of turn text accepted for transmit planning.
pub const MAX_TURN_TEXT_BYTES: usize = 64 * 1024;
/// Max attachment bytes forwarded per authorized transmit.
pub const MAX_TRANSMIT_BYTES: usize = 8 * 1024 * 1024;
/// Max attachments forwarded per turn.
pub const MAX_TRANSMIT_ATTACHMENTS: usize = 8;
/// Max artifact bytes accepted for an edit/version.
pub const MAX_ADAPTER_ARTIFACT_BYTES: usize = 256 * 1024;

/// Real availability inputs. The caller (server boundary) supplies these;
/// the report derives from them so a missing adapter can never read green.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WebTurnAvailability {
    /// Native tool executor reachable from the web turn path.
    pub tool_adapter: bool,
    /// Plugin host owned by the web daemon.
    pub plugin_host: bool,
    /// Execution-owned approval flow on the web turn path.
    pub approval_flow: bool,
    /// Draft ingest store (persist files pre-turn).
    pub draft_ingest: bool,
    /// Provider attachment adapter (blob -> Responses input).
    pub attachment_provider_adapter: bool,
    /// Native transcription / realtime audio adapter.
    pub transcription_adapter: bool,
    /// Web-search adapter (local grep does not count).
    pub web_search_adapter: bool,
    /// Research execution + citation adapter.
    pub research_adapter: bool,
    /// Durable artifact store with versioning.
    pub artifact_store: bool,
    /// Safe native executor for artifact run/apply.
    pub artifact_executor: bool,
}

impl WebTurnAvailability {
    /// Current server reality per `lib.rs:139-199`: only draft ingest and
    /// durable artifact editing exist; every execution/transmit path is down.
    #[must_use]
    pub fn current() -> Self {
        Self {
            tool_adapter: false,
            plugin_host: false,
            approval_flow: false,
            draft_ingest: true,
            attachment_provider_adapter: false,
            transcription_adapter: false,
            web_search_adapter: false,
            research_adapter: false,
            artifact_store: true,
            artifact_executor: false,
        }
    }
}

/// Honest per-feature status: available, or unavailable with the reason the
/// client must show instead of a fake success.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeatureStatus {
    Available,
    Unavailable { reason: &'static str },
}

impl FeatureStatus {
    #[must_use]
    pub fn available(&self) -> bool {
        matches!(self, Self::Available)
    }

    #[must_use]
    pub fn reason(&self) -> Option<&'static str> {
        match self {
            Self::Available => None,
            Self::Unavailable { reason } => Some(reason),
        }
    }
}

/// Capability report derived from [`WebTurnAvailability`]. Field names mirror
/// the `web_capabilities` JSON keys so drift is visible in tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapabilityReport {
    pub tools_for_web_turn: FeatureStatus,
    pub plugins_for_web_turn: FeatureStatus,
    pub approvals_for_web_turn: FeatureStatus,
    /// Draft files persisted pre-turn (ingest path).
    pub draft_ingest: FeatureStatus,
    /// Draft bytes forwarded into provider input (transmit path).
    pub attachments_for_web_turn: FeatureStatus,
    pub search: FeatureStatus,
    pub deep_research: FeatureStatus,
    pub voice: FeatureStatus,
    pub artifacts: FeatureStatus,
    pub artifact_editing: FeatureStatus,
    pub artifact_run: FeatureStatus,
    pub artifact_apply: FeatureStatus,
}

/// Build the report from real flags. No literal success: every `Available`
/// requires its corresponding input flag.
#[must_use]
pub fn capability_report(flags: WebTurnAvailability) -> CapabilityReport {
    CapabilityReport {
        tools_for_web_turn: gate(
            flags.tool_adapter,
            "the current web Responses turn adapter does not execute native tools",
        ),
        plugins_for_web_turn: gate(
            flags.plugin_host,
            "the web daemon does not yet own a plugin host or turn adapter",
        ),
        approvals_for_web_turn: gate(
            flags.approval_flow,
            "approval records exist natively but the web turn path has no execution-owned approval flow",
        ),
        draft_ingest: gate(flags.draft_ingest, "draft ingest store is not configured"),
        attachments_for_web_turn: gate(
            flags.draft_ingest && flags.attachment_provider_adapter,
            "draft files are persisted but the provider attachment adapter is unavailable",
        ),
        search: gate(
            flags.web_search_adapter,
            "local grep is not a web-search adapter",
        ),
        deep_research: gate(
            flags.research_adapter,
            "no native research execution and citation adapter is installed",
        ),
        voice: gate(
            flags.transcription_adapter,
            "no native transcription or realtime audio adapter is installed",
        ),
        artifacts: gate(flags.artifact_store, "no durable artifact store is installed"),
        artifact_editing: gate(
            flags.artifact_store,
            "durable editing and versioning are unavailable without the artifact store",
        ),
        artifact_run: gate(
            flags.artifact_store && flags.artifact_executor,
            "durable editing and versioning are available; run/apply require a safe native execution and approval bridge",
        ),
        artifact_apply: gate(
            flags.artifact_store && flags.artifact_executor,
            "durable editing and versioning are available; run/apply require a safe native execution and approval bridge",
        ),
    }
}

fn gate(present: bool, reason: &'static str) -> FeatureStatus {
    if present {
        FeatureStatus::Available
    } else {
        FeatureStatus::Unavailable { reason }
    }
}

/// Attachment pipeline failures. Denial leaves the transmit sink untouched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttachmentGateError {
    EmptyTurn,
    TurnTooLarge { max: usize },
    NotIngested,
    NoProviderAdapter,
    Denied,
    Oversize { len: usize, max: usize },
    TooMany,
}

impl core::fmt::Display for AttachmentGateError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::EmptyTurn => write!(f, "turn text must not be empty"),
            Self::TurnTooLarge { max } => write!(f, "turn text exceeds {max} bytes"),
            Self::NotIngested => write!(f, "attachment was never ingested into the draft store"),
            Self::NoProviderAdapter => write!(
                f,
                "draft files are persisted but the provider attachment adapter is unavailable"
            ),
            Self::Denied => write!(f, "attachment transmit denied: explicit authorization required"),
            Self::Oversize { len, max } => {
                write!(f, "attachment too large: {len} bytes exceeds {max}")
            }
            Self::TooMany => write!(f, "too many attachments for one turn"),
        }
    }
}

impl std::error::Error for AttachmentGateError {}

/// Permit proving the ingest->transmit gate passed. Carries only counts, no
/// bytes or handles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransmitPermit {
    pub attachment_count: usize,
}

/// Ingest path: validate draft bytes for staging. Pure check, stores nothing.
pub fn validate_ingest(name: &str, len: usize) -> Result<(), AttachmentGateError> {
    let t = name.trim();
    if t.is_empty()
        || t.len() > 255
        || t.contains('\0')
        || t.contains('/')
        || t.contains('\\')
        || t.contains(':')
        || t.contains("..")
        || t.starts_with('.')
        || t.starts_with('-')
    {
        return Err(AttachmentGateError::NotIngested);
    }
    if len == 0 || len > MAX_TRANSMIT_BYTES {
        return Err(AttachmentGateError::Oversize {
            len,
            max: MAX_TRANSMIT_BYTES,
        });
    }
    Ok(())
}

/// Transmit path: attachment bytes reach the provider adapter only when the
/// draft was ingested, the provider adapter exists, and the turn is
/// explicitly authorized. Any failure returns before touching `sink`.
pub fn authorize_transmit(
    turn_text: &str,
    ingested_count: usize,
    flags: WebTurnAvailability,
    explicitly_authorized: bool,
) -> Result<TransmitPermit, AttachmentGateError> {
    if turn_text.trim().is_empty() {
        return Err(AttachmentGateError::EmptyTurn);
    }
    if turn_text.len() > MAX_TURN_TEXT_BYTES {
        return Err(AttachmentGateError::TurnTooLarge {
            max: MAX_TURN_TEXT_BYTES,
        });
    }
    if ingested_count > MAX_TRANSMIT_ATTACHMENTS {
        return Err(AttachmentGateError::TooMany);
    }
    if ingested_count == 0 {
        return Err(AttachmentGateError::NotIngested);
    }
    if !flags.attachment_provider_adapter {
        return Err(AttachmentGateError::NoProviderAdapter);
    }
    if !explicitly_authorized {
        return Err(AttachmentGateError::Denied);
    }
    Ok(TransmitPermit {
        attachment_count: ingested_count,
    })
}

/// Forward authorized attachment digests into the provider sink. The sink
/// grows only on the authorized path; denied calls never reach here because
/// they hold no [`TransmitPermit`].
pub fn transmit_attachments(
    permit: TransmitPermit,
    digests: &[String],
    sink: &mut Vec<String>,
) -> Result<usize, AttachmentGateError> {
    if digests.len() != permit.attachment_count || digests.len() > MAX_TRANSMIT_ATTACHMENTS {
        return Err(AttachmentGateError::TooMany);
    }
    sink.extend(digests.iter().cloned());
    Ok(sink.len())
}

/// Artifact operation failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactGateError {
    NoStore,
    DocumentEmpty,
    DocumentTooLarge,
    NoExecutor,
    Denied,
}

impl core::fmt::Display for ArtifactGateError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NoStore => write!(f, "no durable artifact store is installed"),
            Self::DocumentEmpty => write!(f, "artifact document must not be empty"),
            Self::DocumentTooLarge => write!(f, "artifact document exceeds the byte bound"),
            Self::NoExecutor => write!(
                f,
                "code run/apply disabled: no safe native executor is installed"
            ),
            Self::Denied => write!(f, "code run/apply denied: explicit authorization is required"),
        }
    }
}

impl std::error::Error for ArtifactGateError {}

/// Permit for an edit/version write. Editing needs the durable store only;
/// it never executes code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EditPermit;
/// Permit for run/apply. Needs executor AND explicit grant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunPermit;

/// Edit/version gate: allowed whenever the durable store exists and the
/// document is non-empty and bounded. Never touches execution.
pub fn authorize_edit(
    flags: WebTurnAvailability,
    document_len: usize,
) -> Result<EditPermit, ArtifactGateError> {
    if !flags.artifact_store {
        return Err(ArtifactGateError::NoStore);
    }
    if document_len == 0 {
        return Err(ArtifactGateError::DocumentEmpty);
    }
    if document_len > MAX_ADAPTER_ARTIFACT_BYTES {
        return Err(ArtifactGateError::DocumentTooLarge);
    }
    Ok(EditPermit)
}

/// Run/apply gate: disabled without a safe executor, denied without an
/// explicit grant, permitted only when both hold.
pub fn authorize_run_apply(
    flags: WebTurnAvailability,
    explicitly_granted: bool,
) -> Result<RunPermit, ArtifactGateError> {
    if !flags.artifact_store {
        return Err(ArtifactGateError::NoStore);
    }
    if !flags.artifact_executor {
        return Err(ArtifactGateError::NoExecutor);
    }
    if !explicitly_granted {
        return Err(ArtifactGateError::Denied);
    }
    Ok(RunPermit)
}

/// Voice journey: no adapter installed, so every start is an honest refusal.
/// Text chat remains the usable path; this function never fabricates audio.
pub fn voice_start_status(flags: WebTurnAvailability) -> Result<(), FeatureStatus> {
    match gate(
        flags.transcription_adapter,
        "no native transcription or realtime audio adapter is installed",
    ) {
        FeatureStatus::Available => Ok(()),
        s => Err(s),
    }
}

/// Web-search journey: local grep is not a web adapter; honest refusal.
pub fn search_status(flags: WebTurnAvailability) -> Result<(), FeatureStatus> {
    match gate(
        flags.web_search_adapter,
        "local grep is not a web-search adapter",
    ) {
        FeatureStatus::Available => Ok(()),
        s => Err(s),
    }
}

/// Deep-research journey: no execution/citation adapter; honest refusal.
pub fn research_status(flags: WebTurnAvailability) -> Result<(), FeatureStatus> {
    match gate(
        flags.research_adapter,
        "no native research execution and citation adapter is installed",
    ) {
        FeatureStatus::Available => Ok(()),
        s => Err(s),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_up() -> WebTurnAvailability {
        WebTurnAvailability {
            tool_adapter: true,
            plugin_host: true,
            approval_flow: true,
            draft_ingest: true,
            attachment_provider_adapter: true,
            transcription_adapter: true,
            web_search_adapter: true,
            research_adapter: true,
            artifact_store: true,
            artifact_executor: true,
        }
    }

    #[test]
    fn capability_matches_current_server_reality() {
        // Mirrors lib.rs:139-199 web_capabilities on HEAD 5af7884.
        let r = capability_report(WebTurnAvailability::current());
        assert!(!r.tools_for_web_turn.available());
        assert!(!r.plugins_for_web_turn.available());
        assert!(!r.approvals_for_web_turn.available());
        assert!(r.draft_ingest.available());
        assert!(!r.attachments_for_web_turn.available());
        assert!(!r.search.available());
        assert!(!r.deep_research.available());
        assert!(!r.voice.available());
        assert!(r.artifacts.available());
        assert!(r.artifact_editing.available());
        assert!(!r.artifact_run.available());
        assert!(!r.artifact_apply.available());
    }

    #[test]
    fn capability_report_not_hardcoded() {
        // Same reporter with everything present must flip to Available;
        // a hardcoded report would stay red here.
        let r = capability_report(all_up());
        assert!(r.tools_for_web_turn.available());
        assert!(r.plugins_for_web_turn.available());
        assert!(r.approvals_for_web_turn.available());
        assert!(r.draft_ingest.available());
        assert!(r.attachments_for_web_turn.available());
        assert!(r.search.available());
        assert!(r.deep_research.available());
        assert!(r.voice.available());
        assert!(r.artifacts.available());
        assert!(r.artifact_editing.available());
        assert!(r.artifact_run.available());
        assert!(r.artifact_apply.available());
    }

    #[test]
    fn attachment_reaches_adapter_only_when_authorized() {
        let flags = WebTurnAvailability::current();
        let mut sink: Vec<String> = Vec::new();
        // No provider adapter: gate refuses before any sink write.
        let err = authorize_transmit("hello", 1, flags, true).unwrap_err();
        assert_eq!(err, AttachmentGateError::NoProviderAdapter);
        assert!(sink.is_empty(), "denied transmit must leave no side effects");
        // Adapter present but no explicit grant: still refused, sink clean.
        let mut with_adapter = flags;
        with_adapter.attachment_provider_adapter = true;
        let err = authorize_transmit("hello", 1, with_adapter, false).unwrap_err();
        assert_eq!(err, AttachmentGateError::Denied);
        assert!(sink.is_empty(), "denied transmit must leave no side effects");
        // Authorized path forwards exactly the permitted digests.
        let permit = authorize_transmit("hello", 1, with_adapter, true).unwrap();
        let n = transmit_attachments(permit, &["blob:abc".to_string()], &mut sink).unwrap();
        assert_eq!(n, 1);
        assert_eq!(sink, vec!["blob:abc".to_string()]);
    }

    #[test]
    fn artifact_edit_version_allowed_but_run_apply_gated() {
        let flags = WebTurnAvailability::current();
        // Edit/version works on the durable store today.
        assert!(authorize_edit(flags, 128).is_ok());
        // Run/apply refused: no safe executor installed.
        assert_eq!(
            authorize_run_apply(flags, true).unwrap_err(),
            ArtifactGateError::NoExecutor
        );
        // Executor present but no grant: denied.
        let mut with_exec = flags;
        with_exec.artifact_executor = true;
        assert_eq!(
            authorize_run_apply(with_exec, false).unwrap_err(),
            ArtifactGateError::Denied
        );
        assert!(authorize_run_apply(with_exec, true).is_ok());
    }

    #[test]
    fn unsupported_journeys_report_honest_unavailable() {
        let flags = WebTurnAvailability::current();
        for status in [
            voice_start_status(flags).unwrap_err(),
            search_status(flags).unwrap_err(),
            research_status(flags).unwrap_err(),
        ] {
            assert!(!status.available());
            let reason = status.reason().unwrap_or_default();
            assert!(!reason.is_empty(), "unavailable must carry a reason");
        }
        // Reasons name the missing adapter, never a fake success.
        assert!(voice_start_status(flags).unwrap_err().reason().unwrap().contains("adapter"));
        assert!(search_status(flags).unwrap_err().reason().unwrap().contains("adapter"));
        assert!(research_status(flags).unwrap_err().reason().unwrap().contains("adapter"));
        // And the capability report agrees with the journey gates.
        let r = capability_report(flags);
        assert!(!r.voice.available() && !r.search.available() && !r.deep_research.available());
    }
}

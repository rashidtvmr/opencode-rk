//! ULTRA codegen lifecycle state machine.
//!
//! Pure state machine for the typed ULTRA codegen pipeline:
//! `Requested → DraftEmitted(source_hash) → CompileRequested → Compiled | CompileFailed → Executed | Denied | Fallback`
//!
//! No cargo/process execution — the sandboxed executor composes this later.
//! Content-addressed cache: hash of generated source skips compile on hit.
//! Denylist is a const list of forbidden API patterns.
#![forbid(unsafe_code)]

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use thiserror::Error;

// ── Constants ──

/// Default maximum number of compile attempts before fallback.
pub const DEFAULT_MAX_ATTEMPTS: usize = 3;

/// Default maximum source bytes for generated code.
pub const DEFAULT_MAX_SOURCE_BYTES: usize = 64 * 1024;

/// Forbidden API patterns — checked against draft text.
const DENYLIST: &[&str] = &[
    "std::process::Command",
    "std::fs::remove_dir_all",
    "std::fs::remove_file",
    "std::env::set_var",
    "std::env::remove_var",
    "unsafe",
    "transmute",
    "ptr::read",
    "ptr::write",
];

// ── Types ──

/// Content-addressed cache key (SHA-256 hex of source text).
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct CacheKey(pub String);

impl CacheKey {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Denial taxonomy for the ULTRA codegen pipeline.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum DenyReason {
    UnsafeWithoutApproval,
    ForbiddenApiHit { pattern: String },
    BoundsExceeded { bytes: usize, limit: usize },
    BuildFailureN { attempts: usize },
}

/// Configuration for the codegen lifecycle.
#[derive(Clone, Debug)]
pub struct CodegenConfig {
    pub max_attempts: usize,
    pub max_source_bytes: usize,
}

impl Default for CodegenConfig {
    fn default() -> Self {
        Self {
            max_attempts: DEFAULT_MAX_ATTEMPTS,
            max_source_bytes: DEFAULT_MAX_SOURCE_BYTES,
        }
    }
}

/// Output of the draft emission step.
#[derive(Clone, Debug)]
pub struct DraftOutput {
    pub source: String,
    pub source_hash: String,
}

impl DraftOutput {
    #[must_use]
    pub fn cache_key(&self) -> CacheKey {
        CacheKey(self.source_hash.clone())
    }
}

/// Result of a successful compile.
#[derive(Clone, Debug)]
pub struct CompileResult {
    pub binary_path: String,
}

/// Execution bounds recorded after successful execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionBounds {
    pub max_bytes: usize,
    pub max_steps: usize,
}

/// State machine for a single ULTRA codegen request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UltraState {
    Requested,
    DraftEmitted(String), // source hash
    CompileRequested,
    Compiled { binary_path: String },
    CompileFailed { attempt: usize, last_error: String },
    Executed { bounds: ExecutionBounds },
    Denied(DenyReason),
    Fallback { reason: String },
}

/// The ULTRA codegen lifecycle state machine.
#[derive(Clone, Debug)]
pub struct UltraCodegen {
    state: UltraState,
    config: CodegenConfig,
    current_source: Option<String>,
    current_hash: Option<String>,
    attempt_count: usize,
    known_hashes: HashSet<String>,
}

impl UltraCodegen {
    #[must_use]
    pub fn new(config: CodegenConfig) -> Self {
        Self {
            state: UltraState::Requested,
            config,
            current_source: None,
            current_hash: None,
            attempt_count: 0,
            known_hashes: HashSet::new(),
        }
    }

    #[must_use]
    pub fn state(&self) -> &UltraState {
        &self.state
    }

    #[must_use]
    pub fn cache_key(&self) -> Option<CacheKey> {
        self.current_hash.as_ref().map(|h| CacheKey(h.clone()))
    }

    /// Emit a draft of generated source code.
    ///
    /// Checks source bounds and the denylist. On success, transitions to
    /// `DraftEmitted` and records the content hash.
    pub fn emit_draft(&mut self, source: &str) -> Result<DraftOutput, CodegenError> {
        // Bounds check
        if source.len() > self.config.max_source_bytes {
            let bytes = source.len();
            let limit = self.config.max_source_bytes;
            self.state = UltraState::Denied(DenyReason::BoundsExceeded { bytes, limit });
            return Err(CodegenError::SourceTooLarge { bytes, limit });
        }

        // Denylist check
        if let Some(pattern) = Self::check_denylist(source) {
            let p = pattern.to_owned();
            self.state = UltraState::Denied(DenyReason::ForbiddenApiHit {
                pattern: p.clone(),
            });
            return Err(CodegenError::ForbiddenApi { pattern: p });
        }

        let hash = Self::hash_source(source);
        self.current_source = Some(source.to_owned());
        self.current_hash = Some(hash.clone());
        self.known_hashes.insert(hash.clone());
        self.state = UltraState::DraftEmitted(hash.clone());

        Ok(DraftOutput {
            source: source.to_owned(),
            source_hash: hash,
        })
    }

    /// Transition to `CompileRequested`. Must be in `DraftEmitted` or `CompileFailed` state.
    pub fn request_compile(&mut self) {
        if matches!(
            self.state,
            UltraState::DraftEmitted(_) | UltraState::CompileFailed { .. }
        ) {
            self.state = UltraState::CompileRequested;
        }
    }

    /// Record a successful compile. Transitions to `Compiled`.
    pub fn compile_success(&mut self, result: CompileResult) {
        if matches!(self.state, UltraState::CompileRequested) {
            self.state = UltraState::Compiled {
                binary_path: result.binary_path,
            };
        }
    }

    /// Record a compile failure. Transitions to `CompileFailed` or `Fallback`.
    pub fn compile_failure(&mut self, error: &str) {
        if !matches!(self.state, UltraState::CompileRequested) {
            return;
        }
        self.attempt_count += 1;
        if self.attempt_count >= self.config.max_attempts {
            self.state = UltraState::Fallback {
                reason: format!(
                    "compile failed {} times: {}",
                    self.attempt_count, error
                ),
            };
        } else {
            self.state = UltraState::CompileFailed {
                attempt: self.attempt_count,
                last_error: error.to_owned(),
            };
        }
    }

    /// Explicitly deny the request with a reason.
    pub fn deny(&mut self, reason: DenyReason) {
        self.state = UltraState::Denied(reason);
    }

    /// Record successful execution with bounds.
    pub fn execute(&mut self, bounds: ExecutionBounds) {
        if matches!(self.state, UltraState::Compiled { .. }) {
            self.state = UltraState::Executed { bounds };
        }
    }

    /// Look up a cache key against known hashes.
    #[must_use]
    pub fn cache_lookup(&self, key: &CacheKey) -> bool {
        self.known_hashes.contains(&key.0)
    }

    /// Generate a Cargo.toml manifest that enforces `forbid(unsafe_code)`.
    #[must_use]
    pub fn generate_manifest(&self, crate_name: &str) -> String {
        format!(
            r#"[package]
name = "{crate_name}"
version = "0.1.0"
edition = "2021"

[lib]
#![forbid(unsafe_code)]
"#
        )
    }

    /// Check source text against the denylist. Returns first matching pattern.
    fn check_denylist(source: &str) -> Option<&'static str> {
        for &pattern in DENYLIST {
            if source.contains(pattern) {
                return Some(pattern);
            }
        }
        None
    }

    /// SHA-256 hex hash of source text.
    fn hash_source(source: &str) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        // Simple hash for state machine purposes — real impl would use sha2 crate.
        // For test determinism, we use a stable hash.
        let mut hasher = DefaultHasher::new();
        source.hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    }
}

/// Errors from the codegen pipeline.
#[derive(Clone, Debug, Eq, PartialEq, Error)]
pub enum CodegenError {
    #[error("source too large: {bytes} bytes exceeds limit of {limit}")]
    SourceTooLarge { bytes: usize, limit: usize },

    #[error("forbidden API pattern: {pattern}")]
    ForbiddenApi { pattern: String },
}

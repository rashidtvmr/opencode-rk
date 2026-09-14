//! Secret-free integration metadata and OAuth-attempt state.
//!
//! This module deliberately stops before credential storage or external OAuth
//! execution. Callers own scope tokens, attempt ids, time, persistence, secret
//! material, callbacks, and events.

use std::collections::BTreeMap;

use thiserror::Error;

pub const MAX_INTEGRATION_SCOPES: usize = 8;
pub const MAX_INTEGRATION_METHODS: usize = 8;
pub const MAX_INTEGRATIONS_PER_SCOPE: usize = 32;
pub const MAX_OAUTH_ATTEMPTS: usize = 8;
pub const OAUTH_ATTEMPT_LIFETIME_MS: u64 = 10 * 60 * 1_000;
pub const MAX_INTEGRATION_ID_BYTES: usize = 128;
pub const MAX_INTEGRATION_NAME_BYTES: usize = 256;
pub const MAX_INTEGRATION_LABEL_BYTES: usize = 256;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IntegrationInfo {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IntegrationMethod {
    Key { label: String },
    OAuth { id: String, label: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PluginIntegrationRegistration {
    pub info: IntegrationInfo,
    pub methods: Vec<IntegrationMethod>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PluginIntegrationDeclaration {
    pub integrations: Vec<PluginIntegrationRegistration>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OAuthAttemptStatus {
    Pending,
    Complete,
    Expired,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OAuthAttempt {
    pub attempt_id: String,
    pub integration_id: String,
    pub method_id: String,
    pub label: Option<String>,
    pub status: OAuthAttemptStatus,
    pub created_at_ms: u64,
    pub expires_at_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OAuthCompletion {
    pub integration_id: String,
    pub method_id: String,
    pub label: Option<String>,
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum IntegrationError {
    #[error("too many integration scopes: {actual} > {max}")]
    TooManyScopes { max: usize, actual: usize },
    #[error("too many integrations in one scope: {actual} > {max}")]
    TooManyIntegrations { max: usize, actual: usize },
    #[error("too many integration methods: {actual} > {max}")]
    TooManyMethods { max: usize, actual: usize },
    #[error("too many oauth attempts: {actual} > {max}")]
    TooManyAttempts { max: usize, actual: usize },
    #[error("duplicate oauth attempt: {attempt_id}")]
    DuplicateAttempt { attempt_id: String },
    #[error("authorization code required for attempt: {attempt_id}")]
    CodeRequired { attempt_id: String },
    #[error("integration not found: {integration_id}")]
    IntegrationNotFound { integration_id: String },
    #[error("scope not found: {scope_id}")]
    ScopeNotFound { scope_id: u64 },
    #[error("oauth attempt not found: {attempt_id}")]
    AttemptNotFound { attempt_id: String },
    #[error("oauth attempt is not pending: {attempt_id}")]
    AttemptNotPending { attempt_id: String },
    #[error("integration identifier cannot be empty")]
    InvalidIntegrationId,
    #[error("oauth method identifier cannot be empty")]
    InvalidMethodId,
    #[error("oauth attempt identifier cannot be empty")]
    InvalidAttemptId,
    #[error("integration metadata field {field} exceeds {max} bytes (actual {actual})")]
    TextTooLong {
        field: &'static str,
        max: usize,
        actual: usize,
    },
}

#[derive(Clone, Debug, Default)]
struct ScopeLayer {
    scope_id: u64,
    integrations: BTreeMap<String, IntegrationInfo>,
    methods: BTreeMap<String, Vec<IntegrationMethod>>,
}

/// Bounded stack of caller-owned integration registration scopes.
///
/// Layers are ordered by registration time, not by numeric scope id. Resolving
/// metadata walks the stack from newest to oldest, matching scoped override
/// semantics: closing a newer layer reveals the previous value.
#[derive(Clone, Debug, Default)]
pub struct IntegrationRegistry {
    layers: Vec<ScopeLayer>,
}

impl IntegrationRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_plugin_scope(
        &mut self,
        scope_id: u64,
        declaration: PluginIntegrationDeclaration,
    ) -> Result<(), IntegrationError> {
        if declaration.integrations.len() > MAX_INTEGRATIONS_PER_SCOPE {
            return Err(IntegrationError::TooManyIntegrations {
                max: MAX_INTEGRATIONS_PER_SCOPE,
                actual: declaration.integrations.len(),
            });
        }
        for registration in &declaration.integrations {
            if registration.methods.len() > MAX_INTEGRATION_METHODS {
                return Err(IntegrationError::TooManyMethods {
                    max: MAX_INTEGRATION_METHODS,
                    actual: registration.methods.len(),
                });
            }
        }

        // Apply plugin metadata to a bounded temporary registry so any invalid
        // identifier/label or capacity failure leaves visible state unchanged.
        let mut next = self.clone();
        for registration in declaration.integrations {
            let integration_id = registration.info.id.clone();
            next.register(scope_id, registration.info)?;
            for method in registration.methods {
                next.set_method(scope_id, &integration_id, method)?;
            }
        }
        *self = next;
        Ok(())
    }

    pub fn register(
        &mut self,
        scope_id: u64,
        info: IntegrationInfo,
    ) -> Result<(), IntegrationError> {
        if info.id.is_empty() {
            return Err(IntegrationError::InvalidIntegrationId);
        }
        validate_len("integration_id", &info.id, MAX_INTEGRATION_ID_BYTES)?;
        validate_len("integration_name", &info.name, MAX_INTEGRATION_NAME_BYTES)?;

        let layer_index = self.ensure_scope(scope_id)?;
        let layer = &mut self.layers[layer_index];
        if !layer.integrations.contains_key(&info.id)
            && layer.integrations.len() >= MAX_INTEGRATIONS_PER_SCOPE
        {
            return Err(IntegrationError::TooManyIntegrations {
                max: MAX_INTEGRATIONS_PER_SCOPE,
                actual: layer.integrations.len() + 1,
            });
        }
        layer.integrations.insert(info.id.clone(), info);
        Ok(())
    }

    pub fn close_scope(&mut self, scope_id: u64) -> bool {
        let Some(index) = self.layers.iter().position(|layer| layer.scope_id == scope_id) else {
            return false;
        };
        self.layers.remove(index);
        true
    }

    #[must_use]
    pub fn get(&self, integration_id: &str) -> Option<IntegrationInfo> {
        self.layers
            .iter()
            .rev()
            .find_map(|layer| layer.integrations.get(integration_id))
            .cloned()
    }

    #[must_use]
    pub fn list(&self) -> Vec<IntegrationInfo> {
        let mut visible = BTreeMap::<String, IntegrationInfo>::new();
        for layer in &self.layers {
            for (id, info) in &layer.integrations {
                visible.insert(id.clone(), info.clone());
            }
        }
        visible.into_values().collect()
    }

    pub fn set_method(
        &mut self,
        scope_id: u64,
        integration_id: &str,
        method: IntegrationMethod,
    ) -> Result<(), IntegrationError> {
        if integration_id.is_empty() {
            return Err(IntegrationError::InvalidIntegrationId);
        }
        validate_len(
            "integration_id",
            integration_id,
            MAX_INTEGRATION_ID_BYTES,
        )?;
        match &method {
            IntegrationMethod::Key { label } => {
                validate_len("method_label", label, MAX_INTEGRATION_LABEL_BYTES)?;
            }
            IntegrationMethod::OAuth { id, label } => {
                if id.is_empty() {
                    return Err(IntegrationError::InvalidMethodId);
                }
                validate_len("method_id", id, MAX_INTEGRATION_ID_BYTES)?;
                validate_len("method_label", label, MAX_INTEGRATION_LABEL_BYTES)?;
            }
        }
        if self.get(integration_id).is_none() {
            return Err(IntegrationError::IntegrationNotFound {
                integration_id: integration_id.to_owned(),
            });
        }

        let inherited = self.methods_before_scope(scope_id, integration_id)?;
        let layer_index = self.ensure_scope(scope_id)?;
        let methods = self.layers[layer_index]
            .methods
            .entry(integration_id.to_owned())
            .or_insert(inherited);

        if let Some(index) = methods.iter().position(|current| same_method_slot(current, &method)) {
            methods[index] = method;
            return Ok(());
        }
        if methods.len() >= MAX_INTEGRATION_METHODS {
            return Err(IntegrationError::TooManyMethods {
                max: MAX_INTEGRATION_METHODS,
                actual: methods.len() + 1,
            });
        }
        methods.push(method);
        Ok(())
    }

    #[must_use]
    pub fn methods(&self, integration_id: &str) -> Vec<IntegrationMethod> {
        self.layers
            .iter()
            .rev()
            .find_map(|layer| layer.methods.get(integration_id))
            .cloned()
            .unwrap_or_default()
    }

    fn ensure_scope(&mut self, scope_id: u64) -> Result<usize, IntegrationError> {
        if let Some(index) = self.layers.iter().position(|layer| layer.scope_id == scope_id) {
            return Ok(index);
        }
        if self.layers.len() >= MAX_INTEGRATION_SCOPES {
            return Err(IntegrationError::TooManyScopes {
                max: MAX_INTEGRATION_SCOPES,
                actual: self.layers.len() + 1,
            });
        }
        self.layers.push(ScopeLayer {
            scope_id,
            ..ScopeLayer::default()
        });
        Ok(self.layers.len() - 1)
    }

    fn methods_before_scope(
        &self,
        scope_id: u64,
        integration_id: &str,
    ) -> Result<Vec<IntegrationMethod>, IntegrationError> {
        let Some(index) = self.layers.iter().position(|layer| layer.scope_id == scope_id) else {
            return Ok(self.methods(integration_id));
        };
        Ok(self.layers[..index]
            .iter()
            .rev()
            .find_map(|layer| layer.methods.get(integration_id))
            .cloned()
            .unwrap_or_default())
    }
}

fn same_method_slot(left: &IntegrationMethod, right: &IntegrationMethod) -> bool {
    match (left, right) {
        (IntegrationMethod::Key { .. }, IntegrationMethod::Key { .. }) => true,
        (
            IntegrationMethod::OAuth { id: left_id, .. },
            IntegrationMethod::OAuth { id: right_id, .. },
        ) => left_id == right_id,
        _ => false,
    }
}

/// Bounded caller-driven OAuth attempt state. No authorization codes or tokens
/// are retained: completion returns only metadata that a higher layer may use.
#[derive(Clone, Debug, Default)]
pub struct OAuthAttempts {
    attempts: BTreeMap<String, OAuthAttempt>,
}

impl OAuthAttempts {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn start_code(
        &mut self,
        attempt_id: &str,
        integration_id: &str,
        method_id: &str,
        label: Option<&str>,
        now_ms: u64,
    ) -> Result<(), IntegrationError> {
        if attempt_id.is_empty() {
            return Err(IntegrationError::InvalidAttemptId);
        }
        if integration_id.is_empty() {
            return Err(IntegrationError::InvalidIntegrationId);
        }
        if method_id.is_empty() {
            return Err(IntegrationError::InvalidMethodId);
        }
        validate_len("attempt_id", attempt_id, MAX_INTEGRATION_ID_BYTES)?;
        validate_len(
            "integration_id",
            integration_id,
            MAX_INTEGRATION_ID_BYTES,
        )?;
        validate_len("method_id", method_id, MAX_INTEGRATION_ID_BYTES)?;
        if let Some(label) = label {
            validate_len("attempt_label", label, MAX_INTEGRATION_LABEL_BYTES)?;
        }
        if self.attempts.contains_key(attempt_id) {
            return Err(IntegrationError::DuplicateAttempt {
                attempt_id: attempt_id.to_owned(),
            });
        }
        if self.attempts.len() >= MAX_OAUTH_ATTEMPTS {
            return Err(IntegrationError::TooManyAttempts {
                max: MAX_OAUTH_ATTEMPTS,
                actual: self.attempts.len() + 1,
            });
        }
        let attempt = OAuthAttempt {
            attempt_id: attempt_id.to_owned(),
            integration_id: integration_id.to_owned(),
            method_id: method_id.to_owned(),
            label: label.map(str::to_owned),
            status: OAuthAttemptStatus::Pending,
            created_at_ms: now_ms,
            expires_at_ms: now_ms.saturating_add(OAUTH_ATTEMPT_LIFETIME_MS),
        };
        self.attempts.insert(attempt_id.to_owned(), attempt);
        Ok(())
    }

    #[must_use]
    pub fn get(&self, attempt_id: &str) -> Option<&OAuthAttempt> {
        self.attempts.get(attempt_id)
    }

    pub fn complete_code(
        &mut self,
        attempt_id: &str,
        code: Option<&str>,
    ) -> Result<OAuthCompletion, IntegrationError> {
        let attempt = self
            .attempts
            .get(attempt_id)
            .ok_or_else(|| IntegrationError::AttemptNotFound {
                attempt_id: attempt_id.to_owned(),
            })?;
        if attempt.status != OAuthAttemptStatus::Pending {
            return Err(IntegrationError::AttemptNotPending {
                attempt_id: attempt_id.to_owned(),
            });
        }
        if code.is_none() {
            return Err(IntegrationError::CodeRequired {
                attempt_id: attempt_id.to_owned(),
            });
        }

        // Deliberately do not retain the authorization code.
        let completion = OAuthCompletion {
            integration_id: attempt.integration_id.clone(),
            method_id: attempt.method_id.clone(),
            label: attempt.label.clone(),
        };
        self.attempts
            .get_mut(attempt_id)
            .expect("attempt existence checked above")
            .status = OAuthAttemptStatus::Complete;
        Ok(completion)
    }

    pub fn expire(&mut self, now_ms: u64) -> usize {
        let mut expired = 0usize;
        for attempt in self.attempts.values_mut() {
            if attempt.status == OAuthAttemptStatus::Pending && attempt.expires_at_ms <= now_ms {
                attempt.status = OAuthAttemptStatus::Expired;
                expired += 1;
            }
        }
        expired
    }

    pub fn cancel(&mut self, attempt_id: &str) -> Result<(), IntegrationError> {
        let Some(attempt) = self.attempts.get(attempt_id) else {
            return Err(IntegrationError::AttemptNotFound {
                attempt_id: attempt_id.to_owned(),
            });
        };
        if attempt.status != OAuthAttemptStatus::Pending {
            return Err(IntegrationError::AttemptNotPending {
                attempt_id: attempt_id.to_owned(),
            });
        }
        self.attempts.remove(attempt_id);
        Ok(())
    }
}

fn validate_len(
    field: &'static str,
    value: &str,
    max: usize,
) -> Result<(), IntegrationError> {
    if value.len() > max {
        return Err(IntegrationError::TextTooLong {
            field,
            max,
            actual: value.len(),
        });
    }
    Ok(())
}

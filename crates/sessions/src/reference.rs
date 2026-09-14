use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReferenceSource {
    Local {
        path: String,
        description: Option<String>,
        hidden: Option<bool>,
    },
    Git {
        repository: String,
        branch: Option<String>,
        description: Option<String>,
        hidden: Option<bool>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PluginReferenceRegistration {
    pub alias: String,
    pub source: ReferenceSource,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PluginReferenceDeclaration {
    pub references: Vec<PluginReferenceRegistration>,
}

pub const MAX_REFERENCE_SCOPES: usize = 8;
pub const MAX_REFERENCES_PER_SCOPE: usize = 32;
pub const MAX_REFERENCE_ALIAS_BYTES: usize = 128;
pub const MAX_REFERENCE_METADATA_BYTES: usize = 4 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReferenceError {
    InvalidAlias {
        alias: String,
    },
    DuplicateAlias {
        alias: String,
    },
    TextTooLong {
        field: &'static str,
        max: usize,
        actual: usize,
    },
    TooManyScopes {
        max: usize,
        actual: usize,
    },
    TooManyReferences {
        max: usize,
        actual: usize,
    },
}

#[derive(Clone, Debug, Default)]
struct ScopeLayer {
    scope_id: u64,
    references: BTreeMap<String, PluginReferenceRegistration>,
}

#[derive(Clone, Debug, Default)]
pub struct ReferenceRegistry {
    layers: Vec<ScopeLayer>,
}

impl ReferenceRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_plugin_scope(
        &mut self,
        scope_id: u64,
        declaration: PluginReferenceDeclaration,
    ) -> Result<(), ReferenceError> {
        if declaration.references.len() > MAX_REFERENCES_PER_SCOPE {
            return Err(ReferenceError::TooManyReferences {
                max: MAX_REFERENCES_PER_SCOPE,
                actual: declaration.references.len(),
            });
        }

        let mut aliases = BTreeSet::new();
        let mut references = BTreeMap::new();
        for registration in declaration.references {
            validate_registration(&registration)?;
            if !aliases.insert(registration.alias.clone()) {
                return Err(ReferenceError::DuplicateAlias {
                    alias: registration.alias,
                });
            }
            references.insert(registration.alias.clone(), registration);
        }

        if let Some(index) = self
            .layers
            .iter()
            .position(|layer| layer.scope_id == scope_id)
        {
            self.layers[index].references = references;
            return Ok(());
        }
        if self.layers.len() >= MAX_REFERENCE_SCOPES {
            return Err(ReferenceError::TooManyScopes {
                max: MAX_REFERENCE_SCOPES,
                actual: self.layers.len() + 1,
            });
        }
        self.layers.push(ScopeLayer {
            scope_id,
            references,
        });
        Ok(())
    }

    pub fn add(
        &mut self,
        scope_id: u64,
        registration: PluginReferenceRegistration,
    ) -> Result<(), ReferenceError> {
        validate_registration(&registration)?;
        let layer_index = self.ensure_scope(scope_id)?;
        let layer = &mut self.layers[layer_index];
        if !layer.references.contains_key(&registration.alias)
            && layer.references.len() >= MAX_REFERENCES_PER_SCOPE
        {
            return Err(ReferenceError::TooManyReferences {
                max: MAX_REFERENCES_PER_SCOPE,
                actual: layer.references.len() + 1,
            });
        }
        layer
            .references
            .insert(registration.alias.clone(), registration);
        Ok(())
    }

    pub fn remove(&mut self, scope_id: u64, alias: &str) -> Result<bool, ReferenceError> {
        validate_alias(alias)?;
        let Some(layer) = self
            .layers
            .iter_mut()
            .find(|layer| layer.scope_id == scope_id)
        else {
            return Ok(false);
        };
        Ok(layer.references.remove(alias).is_some())
    }

    #[must_use]
    pub fn list(&self) -> Vec<PluginReferenceRegistration> {
        let mut visible = BTreeMap::<String, PluginReferenceRegistration>::new();
        for layer in &self.layers {
            for (alias, registration) in &layer.references {
                visible.insert(alias.clone(), registration.clone());
            }
        }
        visible.into_values().collect()
    }

    pub fn close_scope(&mut self, scope_id: u64) -> bool {
        let Some(index) = self
            .layers
            .iter()
            .position(|layer| layer.scope_id == scope_id)
        else {
            return false;
        };
        self.layers.remove(index);
        true
    }

    fn ensure_scope(&mut self, scope_id: u64) -> Result<usize, ReferenceError> {
        if let Some(index) = self
            .layers
            .iter()
            .position(|layer| layer.scope_id == scope_id)
        {
            return Ok(index);
        }
        if self.layers.len() >= MAX_REFERENCE_SCOPES {
            return Err(ReferenceError::TooManyScopes {
                max: MAX_REFERENCE_SCOPES,
                actual: self.layers.len() + 1,
            });
        }
        self.layers.push(ScopeLayer {
            scope_id,
            references: BTreeMap::new(),
        });
        Ok(self.layers.len() - 1)
    }
}

fn validate_registration(registration: &PluginReferenceRegistration) -> Result<(), ReferenceError> {
    validate_alias(&registration.alias)?;
    match &registration.source {
        ReferenceSource::Local {
            path, description, ..
        } => {
            validate_metadata("path", path)?;
            if let Some(description) = description {
                validate_metadata("description", description)?;
            }
        }
        ReferenceSource::Git {
            repository,
            branch,
            description,
            ..
        } => {
            validate_metadata("repository", repository)?;
            if let Some(branch) = branch {
                validate_metadata("branch", branch)?;
            }
            if let Some(description) = description {
                validate_metadata("description", description)?;
            }
        }
    }
    Ok(())
}

fn validate_alias(alias: &str) -> Result<(), ReferenceError> {
    if alias.is_empty()
        || alias.chars().any(|character| {
            character.is_whitespace() || matches!(character, '/' | '\\' | '`' | ',')
        })
    {
        return Err(ReferenceError::InvalidAlias {
            alias: alias.to_owned(),
        });
    }
    validate_text("alias", alias, MAX_REFERENCE_ALIAS_BYTES)
}

fn validate_metadata(field: &'static str, value: &str) -> Result<(), ReferenceError> {
    validate_text(field, value, MAX_REFERENCE_METADATA_BYTES)
}

fn validate_text(field: &'static str, value: &str, max: usize) -> Result<(), ReferenceError> {
    if value.len() > max {
        return Err(ReferenceError::TextTooLong {
            field,
            max,
            actual: value.len(),
        });
    }
    Ok(())
}

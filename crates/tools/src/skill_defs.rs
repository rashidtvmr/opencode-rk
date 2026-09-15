//! Skill definitions qualifier (EXT-001).

use thiserror::Error;

pub const MAX_SKILL_DEFS: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillDef {
    pub name: String,
    pub desc: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SkillDefError {
    #[error("empty name")]
    EmptyName,
    #[error("empty desc")]
    EmptyDesc,
    #[error("too many: max {max}, actual {actual}")]
    TooMany { max: usize, actual: usize },
}

pub fn qualify_skills(items: &[(&str, &str)]) -> Result<Vec<SkillDef>, SkillDefError> {
    if items.len() > MAX_SKILL_DEFS {
        return Err(SkillDefError::TooMany {
            max: MAX_SKILL_DEFS,
            actual: items.len(),
        });
    }
    items
        .iter()
        .map(|(name, desc)| {
            if name.is_empty() {
                return Err(SkillDefError::EmptyName);
            }
            if desc.is_empty() {
                return Err(SkillDefError::EmptyDesc);
            }
            Ok(SkillDef {
                name: (*name).to_owned(),
                desc: (*desc).to_owned(),
            })
        })
        .collect()
}

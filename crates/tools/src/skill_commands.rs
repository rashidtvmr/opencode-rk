//! In-memory skill + slash-command name registry (EXT slice registry half).

use thiserror::Error;

pub const MAX_SKILLS: usize = 64;
pub const MAX_COMMANDS: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillEntry {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandEntry {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SkillRegistryError {
    #[error("empty name")]
    EmptyName,
    #[error("duplicate name: {name}")]
    DuplicateName { name: String },
    #[error("too many skills: max {max}, actual {actual}")]
    TooManySkills { max: usize, actual: usize },
    #[error("too many commands: max {max}, actual {actual}")]
    TooManyCommands { max: usize, actual: usize },
}

#[derive(Debug, Default)]
pub struct SkillRegistry {
    skills: Vec<SkillEntry>,
    commands: Vec<CommandEntry>,
}

impl SkillRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_skill(&mut self, e: SkillEntry) -> Result<(), SkillRegistryError> {
        if e.name.is_empty() {
            return Err(SkillRegistryError::EmptyName);
        }
        if self.skills.iter().any(|s| s.name == e.name) {
            return Err(SkillRegistryError::DuplicateName { name: e.name });
        }
        if self.skills.len() >= MAX_SKILLS {
            return Err(SkillRegistryError::TooManySkills {
                max: MAX_SKILLS,
                actual: self.skills.len() + 1,
            });
        }
        self.skills.push(e);
        Ok(())
    }

    pub fn register_command(&mut self, e: CommandEntry) -> Result<(), SkillRegistryError> {
        if e.name.is_empty() {
            return Err(SkillRegistryError::EmptyName);
        }
        if self.commands.iter().any(|c| c.name == e.name) {
            return Err(SkillRegistryError::DuplicateName { name: e.name });
        }
        if self.commands.len() >= MAX_COMMANDS {
            return Err(SkillRegistryError::TooManyCommands {
                max: MAX_COMMANDS,
                actual: self.commands.len() + 1,
            });
        }
        self.commands.push(e);
        Ok(())
    }

    pub fn skill(&self, n: &str) -> Option<&SkillEntry> {
        self.skills.iter().find(|s| s.name == n)
    }

    pub fn command(&self, n: &str) -> Option<&CommandEntry> {
        self.commands.iter().find(|c| c.name == n)
    }

    pub fn skills(&self) -> &[SkillEntry] {
        &self.skills
    }

    pub fn commands(&self) -> &[CommandEntry] {
        &self.commands
    }
}

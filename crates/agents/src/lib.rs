//! Agent registry and manager for the OpenCode RK runtime.
#![forbid(unsafe_code)]
#![allow(clippy::module_name_repetitions, clippy::missing_errors_doc)]

pub mod executor;
pub mod message;
pub mod turn_state;

use std::collections::HashMap;

use opencode_rk_contracts::AgentId;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AgentConfig {
    pub model: String,
    pub api_key: String,
    pub base_url: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Agent {
    pub id: AgentId,
    pub name: String,
    pub capabilities: Vec<String>,
    pub config: AgentConfig,
}

#[derive(Debug, Clone, Error, Eq, PartialEq)]
pub enum AgentError {
    #[error("agent already registered: {0}")]
    Duplicate(String),
    #[error("agent not found: {0}")]
    NotFound(String),
}

#[derive(Default)]
pub struct AgentManager {
    agents: HashMap<String, Agent>,
}

impl AgentManager {
    #[must_use]
    pub fn new() -> Self {
        Self {
            agents: HashMap::new(),
        }
    }

    pub fn register(&mut self, agent: Agent) -> Result<(), AgentError> {
        let key = agent.id.to_string();
        if self.agents.contains_key(&key) {
            return Err(AgentError::Duplicate(key));
        }
        self.agents.insert(key, agent);
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<&Agent> {
        self.agents.get(id)
    }

    pub fn list(&self) -> Vec<&Agent> {
        self.agents.values().collect()
    }

    pub fn remove(&mut self, id: &str) -> bool {
        self.agents.remove(id).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn make_agent(id: &str, name: &str) -> Agent {
        Agent {
            id: AgentId::from_uuid(Uuid::parse_str(id).unwrap()),
            name: name.to_owned(),
            capabilities: vec![],
            config: AgentConfig {
                model: "gpt-4".to_owned(),
                api_key: "test-key".to_owned(),
                base_url: "https://api.example.com".to_owned(),
            },
        }
    }

    #[test]
    fn register_and_get() {
        let mut mgr = AgentManager::new();
        let agent = make_agent("00000000-0000-0000-0000-000000000001", "Test Agent");
        let id_str = agent.id.to_string();
        mgr.register(agent).unwrap();
        let got = mgr.get(&id_str);
        assert!(got.is_some());
        assert_eq!(got.unwrap().name, "Test Agent");
    }

    #[test]
    fn list_returns_all() {
        let mut mgr = AgentManager::new();
        let a1 = make_agent("00000000-0000-0000-0000-000000000001", "Alpha");
        let a2 = make_agent("00000000-0000-0000-0000-000000000002", "Beta");
        mgr.register(a1).unwrap();
        mgr.register(a2).unwrap();
        let list = mgr.list();
        assert_eq!(list.len(), 2);
        let names: Vec<&str> = list.iter().map(|a| a.name.as_str()).collect();
        assert!(names.contains(&"Alpha"));
        assert!(names.contains(&"Beta"));
    }

    #[test]
    fn remove_deletes() {
        let mut mgr = AgentManager::new();
        let agent = make_agent("00000000-0000-0000-0000-000000000003", "Removable");
        let id_str = agent.id.to_string();
        mgr.register(agent).unwrap();
        assert!(mgr.get(&id_str).is_some());
        assert!(mgr.remove(&id_str));
        assert!(mgr.get(&id_str).is_none());
    }

    #[test]
    fn get_unknown_returns_none() {
        let mgr = AgentManager::new();
        assert!(mgr.get("nonexistent").is_none());
    }

    #[test]
    fn register_duplicate() {
        let mut mgr = AgentManager::new();
        let a1 = make_agent("00000000-0000-0000-0000-000000000004", "First");
        let id_str = a1.id.to_string();
        mgr.register(a1.clone()).unwrap();
        let result = mgr.register(a1);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            AgentError::Duplicate(id_str.to_string())
        );
    }
}

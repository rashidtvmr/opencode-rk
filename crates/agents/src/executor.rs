//! Agent execution lifecycle: tracks in-flight agent runs, supports
//! cancellation, status queries, and result collection.
#![forbid(unsafe_code)]

use chrono::{DateTime, Utc};
use opencode_rk_contracts::AgentId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use thiserror::Error;

/// The outcome state of an agent execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStatus {
    Running,
    Completed,
    Failed,
    Cancelled,
}

/// A single agent execution record.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AgentExecution {
    pub agent_id: AgentId,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub status: ExecutionStatus,
    pub result: Option<String>,
}

impl AgentExecution {
    /// Create a new execution in the `Running` state.
    #[must_use]
    pub fn new(agent_id: AgentId) -> Self {
        Self {
            agent_id,
            started_at: Utc::now(),
            completed_at: None,
            status: ExecutionStatus::Running,
            result: None,
        }
    }

    /// Mark the execution as completed with a result.
    pub fn complete(&mut self, result: impl Into<String>) {
        self.completed_at = Some(Utc::now());
        self.status = ExecutionStatus::Completed;
        self.result = Some(result.into());
    }

    /// Mark the execution as failed with an error message.
    pub fn fail(&mut self, error: impl Into<String>) {
        self.completed_at = Some(Utc::now());
        self.status = ExecutionStatus::Failed;
        self.result = Some(error.into());
    }

    /// Mark the execution as cancelled.
    pub fn cancel(&mut self) {
        self.completed_at = Some(Utc::now());
        self.status = ExecutionStatus::Cancelled;
    }

    /// Return the execution id (same as agent_id for now).
    #[must_use]
    pub fn id(&self) -> String {
        self.agent_id.to_string()
    }
}

/// Error type for execution management operations.
#[derive(Debug, Error)]
pub enum ExecutionError {
    #[error("no execution with id {0}")]
    NotFound(String),
    #[error("execution already exists with id {0}")]
    AlreadyExists(String),
}

/// Manages the lifecycle of agent executions.
#[derive(Default, Clone)]
pub struct ExecutionManager {
    executions: Arc<Mutex<HashMap<String, AgentExecution>>>,
}

impl ExecutionManager {
    /// Create a new empty execution manager.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Start tracking a new execution. Returns an error if the agent_id already exists.
    pub fn execute(&self, agent_id: AgentId) -> Result<String, ExecutionError> {
        let id = agent_id.to_string();
        let execution = AgentExecution::new(agent_id);
        let mut map = self
            .executions
            .lock()
            .map_err(|_| ExecutionError::NotFound("mutex poisoned".to_string()))?;
        if map.contains_key(&id) {
            return Err(ExecutionError::AlreadyExists(id));
        }
        map.insert(id.clone(), execution);
        Ok(id)
    }

    /// Cancel an in-flight execution. Returns the cancelled execution if it was running.
    pub fn cancel(&self, id: &str) -> Option<AgentExecution> {
        let mut map = self.executions.lock().ok()?;
        let execution = map.get_mut(id)?;
        if execution.status != ExecutionStatus::Running {
            return None;
        }
        execution.cancel();
        Some(execution.clone())
    }

    /// Get the status of an execution by id.
    pub fn status(&self, id: &str) -> Option<ExecutionStatus> {
        let map = self.executions.lock().ok()?;
        map.get(id).map(|e| e.status)
    }

    /// Collect results of all completed executions.
    pub fn results(&self) -> Vec<AgentExecution> {
        let map = match self.executions.lock() {
            Ok(map) => map,
            Err(_) => return Vec::new(),
        };
        map.values()
            .filter(|e| {
                e.status == ExecutionStatus::Completed || e.status == ExecutionStatus::Failed
            })
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencode_rk_contracts::AgentId;

    #[test]
    fn execute_creates() {
        let manager = ExecutionManager::new();
        let agent_id = AgentId::new();
        let id = manager.execute(agent_id).unwrap();
        assert_eq!(id, agent_id.to_string());
        assert_eq!(manager.status(&id), Some(ExecutionStatus::Running));
    }

    #[test]
    fn cancel_stops() {
        let manager = ExecutionManager::new();
        let agent_id = AgentId::new();
        let id = manager.execute(agent_id).unwrap();
        let cancelled = manager.cancel(&id).unwrap();
        assert_eq!(cancelled.status, ExecutionStatus::Cancelled);
        assert_ne!(cancelled.completed_at, None);
    }

    #[test]
    fn status_updates() {
        let manager = ExecutionManager::new();
        let agent_id = AgentId::new();
        let id = manager.execute(agent_id).unwrap();

        // Initially running
        assert_eq!(manager.status(&id), Some(ExecutionStatus::Running));

        // Simulate completion
        {
            let mut map = manager.executions.lock().unwrap();
            let exec = map.get_mut(&id).unwrap();
            exec.complete("done");
        }
        assert_eq!(manager.status(&id), Some(ExecutionStatus::Completed));
    }

    #[test]
    fn results_collected() {
        let manager = ExecutionManager::new();

        let id1 = manager.execute(AgentId::new()).unwrap();
        let id2 = manager.execute(AgentId::new()).unwrap();

        // Mark one as completed, one still running
        {
            let mut map = manager.executions.lock().unwrap();
            map.get_mut(&id1).unwrap().complete("result-1");
        }

        let results = manager.results();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, ExecutionStatus::Completed);
        assert_eq!(results[0].result.as_deref(), Some("result-1"));

        // Mark second as failed
        {
            let mut map = manager.executions.lock().unwrap();
            map.get_mut(&id2).unwrap().fail("error-2");
        }

        let results = manager.results();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn cancel_unknown() {
        let manager = ExecutionManager::new();
        let result = manager.cancel("nonexistent-id");
        assert!(result.is_none());
    }
}

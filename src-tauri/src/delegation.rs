// src-tauri/src/delegation.rs
//
// Delegation Protocol: when SourceForge cannot automate a step,
// creates a structured task for the user with clear instructions.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DelegationStatus {
    Pending,
    Completed,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelegationTask {
    pub id: String,
    pub step: String,
    pub instructions: String,
    pub reason: String,
    pub status: DelegationStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelegationReport {
    pub tasks: Vec<DelegationTask>,
    pub pending_count: usize,
    pub completed_count: usize,
}

impl DelegationReport {
    pub fn new() -> Self {
        Self {
            tasks: Vec::new(),
            pending_count: 0,
            completed_count: 0,
        }
    }

    pub fn add_task(&mut self, step: &str, instructions: &str, reason: &str) {
        let task = DelegationTask {
            id: uuid_v4(),
            step: step.to_string(),
            instructions: instructions.to_string(),
            reason: reason.to_string(),
            status: DelegationStatus::Pending,
        };
        self.pending_count += 1;
        self.tasks.push(task);
    }

    pub fn is_empty(&self) -> bool {
        self.tasks.is_empty()
    }
}

impl Default for DelegationReport {
    fn default() -> Self {
        Self::new()
    }
}

fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("deleg-{:x}", ts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_task_increments_pending() {
        let mut r = DelegationReport::new();
        r.add_task("Install Vercel CLI", "npm install -g vercel", "not found on PATH");
        assert_eq!(r.pending_count, 1);
        assert_eq!(r.tasks.len(), 1);
        assert_eq!(r.tasks[0].status, DelegationStatus::Pending);
    }

    #[test]
    fn test_empty_report() {
        let r = DelegationReport::new();
        assert!(r.is_empty());
        assert_eq!(r.pending_count, 0);
    }

    #[test]
    fn test_multiple_tasks() {
        let mut r = DelegationReport::new();
        r.add_task("Step 1", "do x", "reason x");
        r.add_task("Step 2", "do y", "reason y");
        assert_eq!(r.pending_count, 2);
        assert!(!r.is_empty());
    }
}

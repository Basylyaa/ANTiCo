//! ANTICO Brain v3.1: Cognitive Coordinator & Intent Perception.
//! Classifies semantic knowledge gaps and produces structured execution plans.

use antico_core::{CapabilityId, ScopedTask, TaskId};
use antico_memory::{AnticoMemoryEngine, MemoryContract};
use serde::{Deserialize, Serialize};

/// High-level intent specification parsed from input.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentSpec {
    pub goal: String,
    pub mission_type: String,
    pub required_capability: CapabilityId,
    pub target_entity: String,
    pub needs_discovery: bool,
}

/// Plan composed of sequential scoped tasks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPlan {
    pub intent: IntentSpec,
    pub tasks: Vec<ScopedTask>,
}

/// Cognitive brain coordinator.
pub struct AnticoBrain {
    memory: AnticoMemoryEngine,
}

impl AnticoBrain {
    pub fn new(memory: AnticoMemoryEngine) -> Self {
        Self { memory }
    }

    /// Parse user prompt into structured IntentSpec.
    pub async fn perceive_intent(&self, prompt: &str) -> IntentSpec {
        let lower = prompt.to_lowercase();

        let (mission_type, cap, entity) = if lower.contains("redis") || lower.contains("docker") {
            (
                "docker_deploy".to_string(),
                CapabilityId("container_deployment".to_string()),
                "redis_container".to_string(),
            )
        } else if lower.contains("auth.log") || lower.contains("log") {
            (
                "log_parsing".to_string(),
                CapabilityId("bash:execute".to_string()),
                "auth_logs".to_string(),
            )
        } else if lower.contains("codeflow") || lower.contains("repository") {
            (
                "codeflow_analysis".to_string(),
                CapabilityId("local:ast".to_string()),
                "codeflow_repo".to_string(),
            )
        } else {
            (
                "generic_task".to_string(),
                CapabilityId("system:default".to_string()),
                "system".to_string(),
            )
        };

        // Constitutional Law §1.1: Memory First - check if knowledge already exists in L1
        let facts = self.memory.read_facts(&entity).await;
        let needs_discovery = facts.is_empty();

        IntentSpec {
            goal: prompt.to_string(),
            mission_type,
            required_capability: cap,
            target_entity: entity,
            needs_discovery,
        }
    }

    /// Synthesize an execution plan based on the intent specification.
    pub async fn plan(&self, intent: &IntentSpec) -> ExecutionPlan {
        let task = ScopedTask {
            id: TaskId(format!("task-{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis())),
            mission_type: intent.mission_type.clone(),
            required_capability: intent.required_capability.clone(),
            payload: intent.goal.clone(),
            timeout_ms: 10000,
            memory_refs: vec![intent.target_entity.clone()],
        };

        ExecutionPlan {
            intent: intent.clone(),
            tasks: vec![task],
        }
    }
}

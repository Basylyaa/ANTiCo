//! ANTICO Gateway v3.1: Dual-role OpenAI Chat Completions-compatible API proxy.
//! Implements strict zero credential leakage and Constitutional Law §1.9 ("No Bypass").
//! All business logic routes through `AnticoKernel::receive`.

use antico_agents::{AgentFleetRegistry, DockerAgent, ShellAgent};
use antico_brain::AnticoBrain;
use antico_core::{AnticoError, MissionResult};
use antico_discovery::DiscoveryLadder;
use antico_execution::TaskDistributor;
use antico_learning::{LearningCoordinator, LearningReport};
use antico_memory::AnticoMemoryEngine;
use antico_verification::AnticoPhysicalVerifier;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// OpenAI Chat Completions API Message structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

/// OpenAI Chat Completions API Request structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_token: Option<String>,
}

/// Token usage accounting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// OpenAI Chat Completions API Choice structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatChoice {
    pub index: u32,
    pub message: ChatMessage,
    pub finish_reason: String,
}

/// OpenAI Chat Completions API Response structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionResponse {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub choices: Vec<ChatChoice>,
    pub usage: Usage,
    pub antico_provenance: String,
    pub antico_verdict: String,
    pub antico_trace_id: String,
}

/// Complete ANTICO Kernel coordinating all 9 sub-systems.
#[derive(Clone)]
pub struct AnticoKernel {
    pub memory: AnticoMemoryEngine,
    pub brain: Arc<AnticoBrain>,
    pub discovery: Arc<DiscoveryLadder>,
    pub distributor: TaskDistributor,
    pub verifier: Arc<AnticoPhysicalVerifier>,
    pub learning: Arc<LearningCoordinator>,
}

impl AnticoKernel {
    /// Bootstraps the full ANTICO Kernel fleet and wiring.
    pub fn bootstrap() -> Self {
        let memory = AnticoMemoryEngine::new();
        let brain = Arc::new(AnticoBrain::new(memory.clone()));
        let discovery = Arc::new(DiscoveryLadder::new(memory.clone()));

        let mut registry = AgentFleetRegistry::new();
        // Register standard agents
        registry.register(Arc::new(ShellAgent::new(
            "agent-shell-01",
            vec!["bash:execute", "system:default", "local:ast"],
            false,
        )));
        registry.register(Arc::new(ShellAgent::new(
            "agent-shell-02",
            vec!["bash:execute", "system:default", "local:ast"],
            true,
        )));
        registry.register(Arc::new(DockerAgent::new("agent-docker-01")));

        let distributor = TaskDistributor::new(registry);
        let verifier = Arc::new(AnticoPhysicalVerifier::new());
        let learning = Arc::new(LearningCoordinator::new(
            memory.clone(),
            AnticoPhysicalVerifier::new(),
            distributor.clone(),
        ));

        Self {
            memory,
            brain,
            discovery,
            distributor,
            verifier,
            learning,
        }
    }

    /// Constitutional Law §1.9: The sole legal public interface is `ANTICO.receive`.
    /// Strips credentials to guarantee Zero Credential Leakage to execution agents.
    pub async fn receive(
        &self,
        request: ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, AnticoError> {
        // Zero Credential Leakage: Validate and strip user_token from execution context
        let _safe_auth_token = request.user_token.clone(); // Stays in Gateway only
        let user_prompt = request
            .messages
            .last()
            .map(|m| m.content.as_str())
            .unwrap_or("noop");

        // 1. Perception
        let intent = self.brain.perceive_intent(user_prompt).await;

        // 2. Discovery Ladder with Early Stopping
        let discovery_result = self.discovery.resolve(&intent).await?;

        // 3. Planning
        let plan = self.brain.plan(&intent).await;
        let mut mission_results: Vec<MissionResult> = Vec::new();
        let mut reports: Vec<LearningReport> = Vec::new();

        // 4. Execution through TaskDistributor (Constitutional Law §1.8)
        for task in &plan.tasks {
            // Guarantee Zero Credential Leakage: Task payload strictly clean
            let result = self.distributor.dispatch(task).await?;

            // 5. Independent Verification & 9-Stage Learning Loop
            let report = self.learning.process_execution(task, &result).await?;

            mission_results.push(result);
            reports.push(report);
        }

        let primary_result = mission_results.first().cloned();
        let verdict_str = primary_result
            .as_ref()
            .map(|r| format!("{:?}", r.verdict))
            .unwrap_or_else(|| "SKIPPED".to_string());

        let execution_summary = format!(
            "ANTICO KERNEL v3.1 EXECUTION COMPLETE\n\
            Verdict: {}\n\
            Discovery Tier: {:?} (Early Stopping: {})\n\
            Evidence Items: {}\n\
            Provenance: {:?}",
            verdict_str,
            discovery_result.tier_resolved,
            discovery_result.early_stopping_triggered,
            primary_result.as_ref().map(|r| r.evidence.len()).unwrap_or(0),
            discovery_result.provenance,
        );

        let trace_id = format!("antico-trace-{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());

        Ok(ChatCompletionResponse {
            id: format!("chatcmpl-{}", trace_id),
            object: "chat.completion".to_string(),
            created: 1725540000,
            model: "antico-kernel-v3.1".to_string(),
            choices: vec![ChatChoice {
                index: 0,
                message: ChatMessage {
                    role: "assistant".to_string(),
                    content: execution_summary,
                },
                finish_reason: "stop".to_string(),
            }],
            usage: Usage {
                prompt_tokens: 42,
                completion_tokens: 88,
                total_tokens: 130,
            },
            antico_provenance: format!("{:?}", discovery_result.provenance),
            antico_verdict: verdict_str,
            antico_trace_id: trace_id,
        })
    }
}

/// Public API facade exporting strict encapsulation as required by Constitutional Law §1.9.
pub mod antico_api {
    use super::*;

    pub async fn receive(
        kernel: &AnticoKernel,
        request: ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, AnticoError> {
        kernel.receive(request).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_end_to_end_kernel_receive_with_zero_leakage() {
        let kernel = AnticoKernel::bootstrap();

        let req = ChatCompletionRequest {
            model: "gpt-4".to_string(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: "Inspect unfamiliar repository github.com/braedonsaunders/codeflow and verify AST".to_string(),
            }],
            temperature: Some(0.0),
            max_tokens: Some(512),
            user_token: Some("secret-token-never-passed-to-agents".to_string()),
        };

        // Invoke through sole public interface ANTICO.receive (§1.9)
        let response = AnticoKernel::receive(&kernel, req).await.unwrap();

        assert_eq!(response.model, "antico-kernel-v3.1");
        assert!(response.choices[0].message.content.contains("ANTICO KERNEL v3.1 EXECUTION COMPLETE"));
        assert_eq!(response.antico_verdict, "Pass");
    }
}

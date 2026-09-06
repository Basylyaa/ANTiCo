//! ANTICO Execution v3.1: Task distributor with Bayesian Beta confidence routing.
//! Enforces Constitutional Law §1.8 ("No Capability, No Execution").

use antico_agents::AgentFleetRegistry;
use antico_core::{
    AgentId, AnticoError, CapabilityId, ExecutionAgent, MissionResult, ScopedTask,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Bayesian Beta distribution confidence state for (Agent, Capability) pair.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BetaConfidence {
    pub alpha: f64, // Success count (+ prior)
    pub beta: f64,  // Failure count (+ prior)
}

impl BetaConfidence {
    pub fn new(alpha: f64, beta: f64) -> Self {
        Self { alpha, beta }
    }

    /// Expected value of the Beta distribution: E[X] = alpha / (alpha + beta)
    pub fn score(&self) -> f64 {
        if self.alpha + self.beta == 0.0 {
            0.5
        } else {
            self.alpha / (self.alpha + self.beta)
        }
    }

    pub fn record_success(&mut self) {
        self.alpha += 1.0;
    }

    pub fn record_failure(&mut self) {
        self.beta += 1.0;
    }
}

impl Default for BetaConfidence {
    fn default() -> Self {
        Self::new(10.0, 1.0) // Initial optimistic prior
    }
}

/// Central Task Distributor that enforces capability verification and dynamic routing.
#[derive(Clone)]
pub struct TaskDistributor {
    registry: AgentFleetRegistry,
    // (AgentId, CapabilityId) -> BetaConfidence
    confidence_table: Arc<RwLock<HashMap<(AgentId, CapabilityId), BetaConfidence>>>,
}

impl TaskDistributor {
    pub fn new(registry: AgentFleetRegistry) -> Self {
        Self {
            registry,
            confidence_table: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Initialize known agent priors.
    pub async fn set_initial_confidence(
        &self,
        agent_id: AgentId,
        capability: CapabilityId,
        alpha: f64,
        beta: f64,
    ) {
        let mut table = self.confidence_table.write().await;
        table.insert((agent_id, capability), BetaConfidence::new(alpha, beta));
    }

    /// Select optimal agent based on capability contract and Bayesian score.
    pub async fn select_agent(
        &self,
        capability: &CapabilityId,
    ) -> Result<Arc<dyn ExecutionAgent>, AnticoError> {
        let candidates = self.registry.find_capable_agents(capability);

        // Constitutional Law §1.8: No Capability, No Execution
        if candidates.is_empty() {
            return Err(AnticoError::CapabilityNotDeclared {
                agent: AgentId("unknown".to_string()),
                required: capability.clone(),
            });
        }

        let table = self.confidence_table.read().await;

        let mut best_agent: Option<Arc<dyn ExecutionAgent>> = None;
        let mut best_score = -1.0;

        for agent in candidates {
            let key = (agent.agent_id(), capability.clone());
            let confidence = table.get(&key).cloned().unwrap_or_default();
            let base_score = confidence.score();

            // Factor in agent health and latency
            let health = agent.health();
            let latency_penalty = (health.latency_ms / 100.0).min(0.2);
            let final_score = (base_score * 0.8) + (health.uptime_percent / 100.0 * 0.2) - latency_penalty;

            if final_score > best_score {
                best_score = final_score;
                best_agent = Some(agent);
            }
        }

        best_agent.ok_or_else(|| AnticoError::Internal("No suitable candidate found".to_string()))
    }

    /// Update Bayesian Beta confidence based on execution outcome.
    pub async fn update_confidence(
        &self,
        agent_id: &AgentId,
        capability: &CapabilityId,
        is_success: bool,
    ) {
        let mut table = self.confidence_table.write().await;
        let entry = table
            .entry((agent_id.clone(), capability.clone()))
            .or_insert_with(BetaConfidence::default);

        if is_success {
            entry.record_success();
        } else {
            entry.record_failure();
        }
    }

    /// Dispatches task through distributor, ensuring strict boundary check.
    pub async fn dispatch(&self, task: &ScopedTask) -> Result<MissionResult, AnticoError> {
        let agent = self.select_agent(&task.required_capability).await?;
        let result = agent.execute(task).await?;

        // Update confidence distribution
        let is_pass = result.verdict.is_pass();
        self.update_confidence(&agent.agent_id(), &task.required_capability, is_pass)
            .await;

        Ok(result)
    }

    pub async fn get_confidence_score(
        &self,
        agent_id: &AgentId,
        capability: &CapabilityId,
    ) -> f64 {
        let table = self.confidence_table.read().await;
        table
            .get(&(agent_id.clone(), capability.clone()))
            .map(|b| b.score())
            .unwrap_or(0.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use antico_agents::ShellAgent;

    #[tokio::test]
    async fn test_agent_a_failure_biases_routing_to_agent_b() {
        let mut registry = AgentFleetRegistry::new();
        let agent_a = Arc::new(ShellAgent::new("agent-shell-01", vec!["bash:execute"], false));
        let agent_b = Arc::new(ShellAgent::new("agent-shell-02", vec!["bash:execute"], true));

        registry.register(agent_a.clone());
        registry.register(agent_b.clone());

        let distributor = TaskDistributor::new(registry);
        let cap = CapabilityId("bash:execute".to_string());

        // Initial state: Give Agent A high prior, Agent B slightly lower prior
        distributor
            .set_initial_confidence(AgentId("agent-shell-01".to_string()), cap.clone(), 10.0, 1.0)
            .await;
        distributor
            .set_initial_confidence(AgentId("agent-shell-02".to_string()), cap.clone(), 5.0, 1.0)
            .await;

        // Verify initial selection picks Agent A
        let selected_first = distributor.select_agent(&cap).await.unwrap();
        assert_eq!(selected_first.agent_id().0, "agent-shell-01");

        // Simulate 5 consecutive failures for Agent A (e.g. multi-line bash crashes)
        for _ in 0..5 {
            distributor
                .update_confidence(&AgentId("agent-shell-01".to_string()), &cap, false)
                .await;
        }

        // Agent B succeeds 3 times
        for _ in 0..3 {
            distributor
                .update_confidence(&AgentId("agent-shell-02".to_string()), &cap, true)
                .await;
        }

        // Now TaskDistributor MUST automatically bias routing to Agent B!
        let selected_second = distributor.select_agent(&cap).await.unwrap();
        assert_eq!(selected_second.agent_id().0, "agent-shell-02");

        let score_a = distributor.get_confidence_score(&AgentId("agent-shell-01".to_string()), &cap).await;
        let score_b = distributor.get_confidence_score(&AgentId("agent-shell-02".to_string()), &cap).await;
        assert!(score_b > score_a);
    }
}

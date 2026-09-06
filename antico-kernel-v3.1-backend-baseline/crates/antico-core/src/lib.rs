//! ANTICO Core v3.1: Core domain primitives, evidence types, and constitutional traits.
//! Enforces Constitutional Law §1.7 ("No Claim Without Evidence") and §1.8 ("No Capability, No Execution").

use serde::{Deserialize, Serialize};
use std::fmt;

/// Strongly typed task identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaskId(pub String);

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Strongly typed agent identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentId(pub String);

impl fmt::Display for AgentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Strongly typed capability identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CapabilityId(pub String);

impl fmt::Display for CapabilityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Strongly typed trace identifier for OpenTelemetry auditing.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TraceId(pub String);

/// Provenance of knowledge and execution artifacts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Provenance {
    VerifiedEmpirical,
    LocalAstAnalysis,
    OsintOfficialFeed,
    AgentMultiStep,
    WebPublicCrawl,
    ModelGenerated,
}

impl Provenance {
    /// Constitutional Law §1.6: Model generated outputs can NEVER serve as empirical proof.
    pub fn is_empirical_proof(&self) -> bool {
        !matches!(self, Provenance::ModelGenerated)
    }
}

/// Physical evidence kind collected during task execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EvidenceKind {
    ExitCode(i32),
    Sha256Checksum(String),
    PortBinding(u16),
    StdoutHash(String),
    ContainerId(String),
    ProbeArtifact { name: String, hash: String },
}

/// Physical evidence artifact collected from real-world execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Evidence {
    pub id: String,
    pub task_id: TaskId,
    pub kind: EvidenceKind,
    pub payload: String,
    pub collected_at_ms: u64,
}

/// Independent verifier verdict on execution results.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Verdict {
    Pass,
    Fail { reason: String },
    Quarantined { violation: String },
    Skipped { reason: String },
}

impl Verdict {
    pub fn is_pass(&self) -> bool {
        matches!(self, Verdict::Pass)
    }
}

/// Scoped task dispatched to an execution agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopedTask {
    pub id: TaskId,
    pub mission_type: String,
    pub required_capability: CapabilityId,
    pub payload: String,
    pub timeout_ms: u64,
    pub memory_refs: Vec<String>,
}

/// Result of a mission execution.
/// Constitutional Law §1.7: A mission cannot be COMPLETED without physical evidence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissionResult {
    pub task_id: TaskId,
    pub agent_id: AgentId,
    pub verdict: Verdict,
    pub evidence: Vec<Evidence>,
    pub execution_time_ms: u64,
    pub completed_at_ms: u64,
}

impl MissionResult {
    /// Validates compliance with Constitutional Law §1.7.
    pub fn is_valid_completion(&self) -> bool {
        self.verdict.is_pass() && !self.evidence.is_empty()
    }
}

/// Core domain errors in ANTICO Kernel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnticoError {
    SecurityViolation(String),
    CapabilityNotDeclared { agent: AgentId, required: CapabilityId },
    NoPhysicalEvidence(String),
    VerificationFailed(String),
    EgressBypassAttempt(String),
    ContradictionDetected(String),
    Timeout(String),
    EarlyStoppingSatisfied,
    Internal(String),
}

impl fmt::Display for AnticoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AnticoError::SecurityViolation(s) => write!(f, "Security Violation (§1.9): {}", s),
            AnticoError::CapabilityNotDeclared { agent, required } => {
                write!(f, "Capability Not Declared (§1.8): agent {} lacks capability {}", agent, required)
            }
            AnticoError::NoPhysicalEvidence(s) => write!(f, "No Physical Evidence (§1.7): {}", s),
            AnticoError::VerificationFailed(s) => write!(f, "Verification Failed (§1.10): {}", s),
            AnticoError::EgressBypassAttempt(s) => write!(f, "Egress Bypass Attempt (§1.9): {}", s),
            AnticoError::ContradictionDetected(s) => write!(f, "Contradiction in Knowledge (§1.3): {}", s),
            AnticoError::Timeout(s) => write!(f, "Execution Timeout: {}", s),
            AnticoError::EarlyStoppingSatisfied => write!(f, "Early Stopping Satisfied"),
            AnticoError::Internal(s) => write!(f, "Internal Kernel Error: {}", s),
        }
    }
}

impl std::error::Error for AnticoError {}

use std::future::Future;
use std::pin::Pin;

/// Convenient type alias for boxed asynchronous futures.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Agent health report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentHealth {
    pub is_healthy: bool,
    pub latency_ms: f64,
    pub uptime_percent: f64,
    pub consecutive_failures: u32,
}

/// Dynamic trait for execution agents.
pub trait ExecutionAgent: Send + Sync {
    fn agent_id(&self) -> AgentId;
    fn capabilities(&self) -> &[CapabilityId];
    fn health(&self) -> AgentHealth;
    fn execute<'a>(
        &'a self,
        task: &'a ScopedTask,
    ) -> BoxFuture<'a, Result<MissionResult, AnticoError>>;
}

/// Dynamic trait for physical evidence verification.
pub trait PhysicalVerifierTrait: Send + Sync {
    fn verify_evidence<'a>(
        &'a self,
        task: &'a ScopedTask,
        evidence: &'a [Evidence],
    ) -> BoxFuture<'a, Result<Verdict, AnticoError>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constitutional_evidence_law() {
        let task_id = TaskId("task-001".to_string());
        let agent_id = AgentId("agent-shell-01".to_string());

        // Result with PASS but empty evidence MUST fail constitutional verification
        let invalid_result = MissionResult {
            task_id: task_id.clone(),
            agent_id: agent_id.clone(),
            verdict: Verdict::Pass,
            evidence: vec![],
            execution_time_ms: 45,
            completed_at_ms: 1000,
        };
        assert!(!invalid_result.is_valid_completion());

        // Result with PASS and valid evidence satisfies §1.7
        let valid_result = MissionResult {
            task_id,
            agent_id,
            verdict: Verdict::Pass,
            evidence: vec![Evidence {
                id: "ev-01".to_string(),
                task_id: TaskId("task-001".to_string()),
                kind: EvidenceKind::ExitCode(0),
                payload: "Process exited with 0".to_string(),
                collected_at_ms: 1000,
            }],
            execution_time_ms: 45,
            completed_at_ms: 1000,
        };
        assert!(valid_result.is_valid_completion());
    }

    #[test]
    fn test_provenance_empirical_guard() {
        assert!(Provenance::VerifiedEmpirical.is_empirical_proof());
        assert!(Provenance::LocalAstAnalysis.is_empirical_proof());
        assert!(!Provenance::ModelGenerated.is_empirical_proof());
    }
}

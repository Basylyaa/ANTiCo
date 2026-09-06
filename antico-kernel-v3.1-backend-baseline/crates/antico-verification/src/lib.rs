//! ANTICO Verification v3.1: Independent physical evidence verification engine.
//! Enforces Constitutional Law §1.7 ("No Claim Without Evidence") and §1.10 ("Physical Truth Wins").

use antico_core::{
    AnticoError, BoxFuture, Evidence, EvidenceKind, PhysicalVerifierTrait, ScopedTask, Verdict,
};

/// Dedicated physical verification engine.
pub struct AnticoPhysicalVerifier;

impl AnticoPhysicalVerifier {
    pub fn new() -> Self {
        Self
    }
}

impl Default for AnticoPhysicalVerifier {
    fn default() -> Self {
        Self::new()
    }
}

impl PhysicalVerifierTrait for AnticoPhysicalVerifier {
    fn verify_evidence<'a>(
        &'a self,
        task: &'a ScopedTask,
        evidence: &'a [Evidence],
    ) -> BoxFuture<'a, Result<Verdict, AnticoError>> {
        Box::pin(async move {
            // Constitutional Law §1.7: A task cannot be validated without physical evidence
            if evidence.is_empty() {
                return Err(AnticoError::NoPhysicalEvidence(format!(
                    "Task {} rejected: Evidence list is empty (§1.7)",
                    task.id
                )));
            }

            let mut has_exit_code = false;
            let mut exit_code_clean = false;

            for ev in evidence {
                match &ev.kind {
                    EvidenceKind::ExitCode(code) => {
                        has_exit_code = true;
                        if *code == 0 {
                            exit_code_clean = true;
                        } else {
                            return Ok(Verdict::Fail {
                                reason: format!("Process exited with non-zero status code {}", code),
                            });
                        }
                    }
                    EvidenceKind::Sha256Checksum(hash) if hash.len() != 64 => {
                        return Ok(Verdict::Fail {
                            reason: "Checksum verification failed: invalid sha256 length".to_string(),
                        });
                    }
                    EvidenceKind::PortBinding(port) if *port == 0 => {
                        return Ok(Verdict::Fail {
                            reason: "Port binding invalid: port 0".to_string(),
                        });
                    }
                    _ => {}
                }
            }

            // If execution task had an exit code requirement, it must be 0
            if has_exit_code && !exit_code_clean {
                return Ok(Verdict::Fail {
                    reason: "Execution produced non-zero exit code".to_string(),
                });
            }

            Ok(Verdict::Pass)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use antico_core::{CapabilityId, TaskId};

    #[tokio::test]
    async fn test_empty_evidence_rejection() {
        let verifier = AnticoPhysicalVerifier::new();
        let task = ScopedTask {
            id: TaskId("task-ev-01".to_string()),
            mission_type: "test".to_string(),
            required_capability: CapabilityId("cap".to_string()),
            payload: "cmd".to_string(),
            timeout_ms: 1000,
            memory_refs: vec![],
        };

        let res = verifier.verify_evidence(&task, &[]).await;
        assert!(matches!(res, Err(AnticoError::NoPhysicalEvidence(_))));
    }

    #[tokio::test]
    async fn test_non_zero_exit_code_rejection() {
        let verifier = AnticoPhysicalVerifier::new();
        let task = ScopedTask {
            id: TaskId("task-ev-02".to_string()),
            mission_type: "test".to_string(),
            required_capability: CapabilityId("cap".to_string()),
            payload: "cmd".to_string(),
            timeout_ms: 1000,
            memory_refs: vec![],
        };

        let evidence = vec![Evidence {
            id: "ev-1".to_string(),
            task_id: task.id.clone(),
            kind: EvidenceKind::ExitCode(1),
            payload: "Exit 1".to_string(),
            collected_at_ms: 100,
        }];

        let verdict = verifier.verify_evidence(&task, &evidence).await.unwrap();
        assert!(matches!(verdict, Verdict::Fail { .. }));
    }
}

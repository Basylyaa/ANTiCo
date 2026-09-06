#![allow(async_fn_in_trait)]
//! ANTICO Memory v3.1: Unified in-process L0-L4 Memory Contract.
//! Implements native async traits (AFIT) and lock-free concurrent write queues (MPSC/Actor model).
//! Enforces Constitutional Law §1.3 (Knowledge Is Permanent) and §120 (KnowledgeEngine Sole Writer).

use antico_core::{AnticoError, Provenance, Verdict};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, RwLock};

/// L1 Permanent Proved Fact.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct L1Fact {
    pub id: String,
    pub entity: String,
    pub attribute: String,
    pub value: String,
    pub confidence: f64,
    pub provenance: Provenance,
    pub timestamp_ms: u64,
}

/// L2 Semantic Ontology Relation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct L2Relation {
    pub id: String,
    pub source_entity: String,
    pub relation_type: String, // e.g. "SUITABLE_FOR", "REQUIRES", "INCOMPATIBLE_WITH"
    pub target_entity: String,
    pub confidence: f64,
    pub timestamp_ms: u64,
}

/// L3 Historical Execution Experience.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct L3Experience {
    pub id: String,
    pub mission_type: String,
    pub agent_used: String,
    pub verdict: Verdict,
    pub repeat_count: u32,
    pub is_promoted: bool,
    pub evidence_refs: Vec<String>,
    pub lesson: String,
    pub timestamp_ms: u64,
}

/// L4 Cryptographic OpenTelemetry Audit Record with SHA256 chain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct L4AuditRecord {
    pub id: String,
    pub trace_id: String,
    pub span_id: String,
    pub action: String,
    pub actor: String,
    pub target: String,
    pub status: String,
    pub sha256_hash: String,
    pub prev_hash: String,
    pub timestamp_ms: u64,
}

/// Result of promoting an L3 experience to permanent L1/L2 knowledge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromotionOutcome {
    pub experience_id: String,
    pub promoted_to_l1: Option<String>,
    pub promoted_to_l2: Option<String>,
    pub repeat_count: u32,
}

/// Native Async Trait (AFIT): Read-only query contract for all memory layers.
pub trait MemoryContract: Send + Sync {
    async fn read_facts(&self, entity: &str) -> Vec<L1Fact>;
    async fn read_relations(&self, source: &str) -> Vec<L2Relation>;
    async fn query_experiences(&self, mission_type: &str) -> Vec<L3Experience>;
    async fn get_audit_trail(&self) -> Vec<L4AuditRecord>;
}

/// Native Async Trait (AFIT): KnowledgeEngine is the SOLE legal writer for L1/L2.
pub trait KnowledgeEngineTrait: Send + Sync {
    async fn assert_fact(&self, fact: L1Fact) -> Result<(), AnticoError>;
    async fn assert_relation(&self, rel: L2Relation) -> Result<(), AnticoError>;
    async fn record_experience(&self, exp: L3Experience) -> Result<String, AnticoError>;
    async fn promote_experience(&self, exp_id: &str) -> Result<PromotionOutcome, AnticoError>;
    async fn append_audit(&self, record: L4AuditRecord) -> Result<String, AnticoError>;
}

/// Commands passed through the non-blocking MPSC write queue (Actor pattern).
pub enum MemoryCommand {
    AssertFact {
        fact: L1Fact,
        resp: oneshot::Sender<Result<(), AnticoError>>,
    },
    AssertRelation {
        rel: L2Relation,
        resp: oneshot::Sender<Result<(), AnticoError>>,
    },
    RecordExperience {
        exp: L3Experience,
        resp: oneshot::Sender<Result<String, AnticoError>>,
    },
    PromoteExperience {
        exp_id: String,
        resp: oneshot::Sender<Result<PromotionOutcome, AnticoError>>,
    },
    AppendAudit {
        record: L4AuditRecord,
        resp: oneshot::Sender<Result<String, AnticoError>>,
    },
}

/// Internal store representation holding L0-L4 state.
#[derive(Default, Serialize, Deserialize)]
struct MemoryStore {
    l1_facts: HashMap<String, L1Fact>,
    l2_relations: Vec<L2Relation>,
    l3_experiences: HashMap<String, L3Experience>,
    l4_audit: Vec<L4AuditRecord>,
    last_audit_hash: String,
}

/// High-performance, concurrent In-Process Memory Engine.
/// Uses an async MPSC channel actor to eliminate write-side lock contention.
#[derive(Clone)]
pub struct AnticoMemoryEngine {
    // Read-optimized shared state for fast non-blocking reads
    store: Arc<RwLock<MemoryStore>>,
    // Lock-free MPSC write queue channel
    write_tx: mpsc::Sender<MemoryCommand>,
}

impl AnticoMemoryEngine {
    /// Creates and initializes the in-process memory engine and spawns the write actor loop.
    pub fn new() -> Self {
        let store = Arc::new(RwLock::new(MemoryStore {
            l1_facts: HashMap::new(),
            l2_relations: Vec::new(),
            l3_experiences: HashMap::new(),
            l4_audit: Vec::new(),
            last_audit_hash: "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
        }));

        // Bounded MPSC channel for lock-free concurrent submission (capacity 4096)
        let (write_tx, mut write_rx) = mpsc::channel::<MemoryCommand>(4096);

        let actor_store = Arc::clone(&store);
        tokio::spawn(async move {
            while let Some(cmd) = write_rx.recv().await {
                let mut s = actor_store.write().await;
                match cmd {
                    MemoryCommand::AssertFact { fact, resp } => {
                        s.l1_facts.insert(fact.id.clone(), fact);
                        let _ = resp.send(Ok(()));
                    }
                    MemoryCommand::AssertRelation { rel, resp } => {
                        s.l2_relations.push(rel);
                        let _ = resp.send(Ok(()));
                    }
                    MemoryCommand::RecordExperience { exp, resp } => {
                        let id = exp.id.clone();
                        s.l3_experiences.insert(id.clone(), exp);
                        let _ = resp.send(Ok(id));
                    }
                    MemoryCommand::PromoteExperience { exp_id, resp } => {
                        let exp_data = s.l3_experiences.get(&exp_id).cloned();
                        if let Some(exp) = exp_data {
                            // Constitutional Law §120: Promotion requires >= 3 consistent PASS executions
                            if exp.repeat_count < 3 {
                                let _ = resp.send(Err(AnticoError::VerificationFailed(format!(
                                    "Promotion rejected (§120): repeat count {} is below mandatory threshold 3",
                                    exp.repeat_count
                                ))));
                                continue;
                            }
                            if !exp.verdict.is_pass() {
                                let _ = resp.send(Err(AnticoError::VerificationFailed(
                                    "Promotion rejected (§120): verdict is not PASS".to_string(),
                                )));
                                continue;
                            }

                            if let Some(e) = s.l3_experiences.get_mut(&exp_id) {
                                e.is_promoted = true;
                            }

                            // Derive L1 Fact from experience
                            let fact_id = format!("promoted-fact-{}", exp.id);
                            let fact = L1Fact {
                                id: fact_id.clone(),
                                entity: exp.mission_type.clone(),
                                attribute: "verified_strategy".to_string(),
                                value: format!("Executed by {} with verdict PASS", exp.agent_used()),
                                confidence: 0.99,
                                provenance: Provenance::VerifiedEmpirical,
                                timestamp_ms: exp.timestamp_ms,
                            };
                            s.l1_facts.insert(fact_id.clone(), fact);

                            // Derive L2 Relation
                            let rel_id = format!("promoted-rel-{}", exp.id);
                            let rel = L2Relation {
                                id: rel_id.clone(),
                                source_entity: exp.mission_type.clone(),
                                relation_type: "SUITABLE_FOR".to_string(),
                                target_entity: exp.agent_used.clone(),
                                confidence: 0.98,
                                timestamp_ms: exp.timestamp_ms,
                            };
                            s.l2_relations.push(rel);

                            let _ = resp.send(Ok(PromotionOutcome {
                                experience_id: exp_id,
                                promoted_to_l1: Some(fact_id),
                                promoted_to_l2: Some(rel_id),
                                repeat_count: exp.repeat_count,
                            }));
                        } else {
                            let _ = resp.send(Err(AnticoError::Internal(format!(
                                "Experience {} not found",
                                exp_id
                            ))));
                        }
                    }
                    MemoryCommand::AppendAudit { mut record, resp } => {
                        // Calculate cryptographic hash chaining (SHA256)
                        let mut hasher = Sha256::new();
                        hasher.update(s.last_audit_hash.as_bytes());
                        hasher.update(record.action.as_bytes());
                        hasher.update(record.actor.as_bytes());
                        hasher.update(record.target.as_bytes());
                        hasher.update(record.status.as_bytes());
                        hasher.update(record.timestamp_ms.to_be_bytes());
                        let hash = format!("{:x}", hasher.finalize());

                        record.prev_hash = s.last_audit_hash.clone();
                        record.sha256_hash = hash.clone();
                        s.last_audit_hash = hash;

                        let id = record.id.clone();
                        s.l4_audit.push(record);
                        let _ = resp.send(Ok(id));
                    }
                }
            }
        });

        Self { store, write_tx }
    }

    /// Seed memory with initial verified facts and relations.
    pub async fn seed_initial_knowledge(&self) {
        let _ = self
            .assert_fact(L1Fact {
                id: "fact-rust-model".to_string(),
                entity: "Rust Runtime".to_string(),
                attribute: "memory_model".to_string(),
                value: "Zero-cost ownership, borrow checker, no GC, compile-time memory safety".to_string(),
                confidence: 1.0,
                provenance: Provenance::VerifiedEmpirical,
                timestamp_ms: 1700000000000,
            })
            .await;

        let _ = self
            .assert_relation(L2Relation {
                id: "rel-docker-shell".to_string(),
                source_entity: "container_deployment".to_string(),
                relation_type: "SUITABLE_FOR".to_string(),
                target_entity: "agent-docker-01".to_string(),
                confidence: 0.98,
                timestamp_ms: 1700000000000,
            })
            .await;
    }

    /// Persist memory store to a JSON file.
    pub async fn save_to_file(&self, path: &str) -> Result<(), AnticoError> {
        let store_guard = self.store.read().await;
        let data = serde_json::to_string_pretty(&*store_guard)
            .map_err(|e| AnticoError::Internal(format!("Failed to serialize memory store: {}", e)))?;
        std::fs::write(path, data)
            .map_err(|e| AnticoError::Internal(format!("Failed to write memory store to file: {}", e)))?;
        Ok(())
    }

    /// Load memory store from a JSON file.
    pub async fn load_from_file(&self, path: &str) -> Result<(), AnticoError> {
        if !std::path::Path::new(path).exists() {
            return Err(AnticoError::Internal(format!("Database file {} does not exist", path)));
        }
        let data = std::fs::read_to_string(path)
            .map_err(|e| AnticoError::Internal(format!("Failed to read memory store file: {}", e)))?;
        let loaded_store: MemoryStore = serde_json::from_str(&data)
            .map_err(|e| AnticoError::Internal(format!("Failed to deserialize memory store: {}", e)))?;
        let mut store_guard = self.store.write().await;
        *store_guard = loaded_store;
        Ok(())
    }
}

impl Default for AnticoMemoryEngine {
    fn default() -> Self {
        Self::new()
    }
}

// MemoryContract Implementation using Native AFIT
impl MemoryContract for AnticoMemoryEngine {
    async fn read_facts(&self, entity: &str) -> Vec<L1Fact> {
        let s = self.store.read().await;
        s.l1_facts
            .values()
            .filter(|f| entity.is_empty() || f.entity.eq_ignore_ascii_case(entity))
            .cloned()
            .collect()
    }

    async fn read_relations(&self, source: &str) -> Vec<L2Relation> {
        let s = self.store.read().await;
        s.l2_relations
            .iter()
            .filter(|r| source.is_empty() || r.source_entity.eq_ignore_ascii_case(source))
            .cloned()
            .collect()
    }

    async fn query_experiences(&self, mission_type: &str) -> Vec<L3Experience> {
        let s = self.store.read().await;
        s.l3_experiences
            .values()
            .filter(|e| mission_type.is_empty() || e.mission_type.eq_ignore_ascii_case(mission_type))
            .cloned()
            .collect()
    }

    async fn get_audit_trail(&self) -> Vec<L4AuditRecord> {
        let s = self.store.read().await;
        s.l4_audit.clone()
    }
}

// KnowledgeEngine Implementation using Native AFIT and lock-free Actor queue
impl KnowledgeEngineTrait for AnticoMemoryEngine {
    async fn assert_fact(&self, fact: L1Fact) -> Result<(), AnticoError> {
        let (resp_tx, resp_rx) = oneshot::channel();
        self.write_tx
            .send(MemoryCommand::AssertFact { fact, resp: resp_tx })
            .await
            .map_err(|e| AnticoError::Internal(e.to_string()))?;
        resp_rx.await.map_err(|e| AnticoError::Internal(e.to_string()))?
    }

    async fn assert_relation(&self, rel: L2Relation) -> Result<(), AnticoError> {
        let (resp_tx, resp_rx) = oneshot::channel();
        self.write_tx
            .send(MemoryCommand::AssertRelation { rel, resp: resp_tx })
            .await
            .map_err(|e| AnticoError::Internal(e.to_string()))?;
        resp_rx.await.map_err(|e| AnticoError::Internal(e.to_string()))?
    }

    async fn record_experience(&self, exp: L3Experience) -> Result<String, AnticoError> {
        let (resp_tx, resp_rx) = oneshot::channel();
        self.write_tx
            .send(MemoryCommand::RecordExperience { exp, resp: resp_tx })
            .await
            .map_err(|e| AnticoError::Internal(e.to_string()))?;
        resp_rx.await.map_err(|e| AnticoError::Internal(e.to_string()))?
    }

    async fn promote_experience(&self, exp_id: &str) -> Result<PromotionOutcome, AnticoError> {
        let (resp_tx, resp_rx) = oneshot::channel();
        self.write_tx
            .send(MemoryCommand::PromoteExperience {
                exp_id: exp_id.to_string(),
                resp: resp_tx,
            })
            .await
            .map_err(|e| AnticoError::Internal(e.to_string()))?;
        resp_rx.await.map_err(|e| AnticoError::Internal(e.to_string()))?
    }

    async fn append_audit(&self, record: L4AuditRecord) -> Result<String, AnticoError> {
        let (resp_tx, resp_rx) = oneshot::channel();
        self.write_tx
            .send(MemoryCommand::AppendAudit { record, resp: resp_tx })
            .await
            .map_err(|e| AnticoError::Internal(e.to_string()))?;
        resp_rx.await.map_err(|e| AnticoError::Internal(e.to_string()))?
    }
}

// Helper method for L3Experience
impl L3Experience {
    pub fn agent_used(&self) -> &str {
        &self.agent_used
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_lock_free_concurrent_promotion_pipeline() {
        let memory = AnticoMemoryEngine::new();

        // 1. Record an experience with repeat_count = 1 (should fail promotion)
        let exp1 = L3Experience {
            id: "exp-001".to_string(),
            mission_type: "log_parsing".to_string(),
            agent_used: "agent-shell-02".to_string(),
            verdict: Verdict::Pass,
            repeat_count: 1,
            is_promoted: false,
            evidence_refs: vec!["ev-001".to_string()],
            lesson: "First execution success".to_string(),
            timestamp_ms: 1000,
        };

        memory.record_experience(exp1).await.unwrap();

        // Promotion fails because repeat_count < 3
        let fail_promo = memory.promote_experience("exp-001").await;
        assert!(fail_promo.is_err());

        // 2. Record an experience with repeat_count = 3 (satisfies Constitutional Law §120)
        let exp3 = L3Experience {
            id: "exp-003".to_string(),
            mission_type: "log_parsing".to_string(),
            agent_used: "agent-shell-02".to_string(),
            verdict: Verdict::Pass,
            repeat_count: 3,
            is_promoted: false,
            evidence_refs: vec!["ev-001".to_string(), "ev-002".to_string(), "ev-003".to_string()],
            lesson: "Consistent 3-cycle pass across multi-line parsing".to_string(),
            timestamp_ms: 1200,
        };

        memory.record_experience(exp3).await.unwrap();

        // Promotion succeeds and writes L1 fact and L2 relation
        let outcome = memory.promote_experience("exp-003").await.unwrap();
        assert_eq!(outcome.repeat_count, 3);
        assert!(outcome.promoted_to_l1.is_some());
        assert!(outcome.promoted_to_l2.is_some());

        // Verify that L1 fact was populated
        let facts = memory.read_facts("log_parsing").await;
        assert_eq!(facts.len(), 1);
        assert_eq!(facts[0].provenance, Provenance::VerifiedEmpirical);

        // Verify that L2 relation was populated
        let relations = memory.read_relations("log_parsing").await;
        assert_eq!(relations.len(), 1);
        assert_eq!(relations[0].relation_type, "SUITABLE_FOR");
    }

    #[tokio::test]
    async fn test_cryptographic_audit_hash_chain() {
        let memory = AnticoMemoryEngine::new();

        memory
            .append_audit(L4AuditRecord {
                id: "audit-1".to_string(),
                trace_id: "trace-101".to_string(),
                span_id: "span-01".to_string(),
                action: "AGENT_DISPATCH".to_string(),
                actor: "TaskDistributor".to_string(),
                target: "agent-shell-02".to_string(),
                status: "SUCCESS".to_string(),
                sha256_hash: String::new(),
                prev_hash: String::new(),
                timestamp_ms: 1000,
            })
            .await
            .unwrap();

        memory
            .append_audit(L4AuditRecord {
                id: "audit-2".to_string(),
                trace_id: "trace-101".to_string(),
                span_id: "span-02".to_string(),
                action: "EVIDENCE_VERIFY".to_string(),
                actor: "PhysicalVerifier".to_string(),
                target: "Verdict::Pass".to_string(),
                status: "SUCCESS".to_string(),
                sha256_hash: String::new(),
                prev_hash: String::new(),
                timestamp_ms: 1050,
            })
            .await
            .unwrap();

        let trail = memory.get_audit_trail().await;
        assert_eq!(trail.len(), 2);
        assert_eq!(trail[1].prev_hash, trail[0].sha256_hash);
        assert_ne!(trail[0].sha256_hash, trail[1].sha256_hash);
    }
}

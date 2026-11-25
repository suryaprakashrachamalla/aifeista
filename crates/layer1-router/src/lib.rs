use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

use layer0_contracts::{
    HarnessEventKind, LaneKind, ModelTier, OptimizationFlags, TurnRequest,
};
use layer0_store::EventStore;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillNode {
    pub id: String,
    pub domain: String,
    pub description: String,
    pub schema_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillRouteResult {
    pub domain_router: String,
    pub loaded_skills: Vec<SkillNode>,
    pub total_catalog_skills: usize,
    pub pruned_count: usize,
}

/// Hierarchical `ROOT -> ROUTER -> SKILL` lazy skill router.
/// Prevents dumping the full tool/skill catalog into every prompt.
#[derive(Clone)]
pub struct SkillTreeRouter {
    skills: Vec<SkillNode>,
}

impl Default for SkillTreeRouter {
    fn default() -> Self {
        Self::new()
    }
}

impl SkillTreeRouter {
    pub fn new() -> Self {
        let skills = vec![
            SkillNode {
                id: "companion.pointer".into(),
                domain: "spatial_companion".into(),
                description: "Move flying pointer to a UI coordinate [POINT:x,y:label]".into(),
                schema_json: r#"{"name":"point_ui","params":["x","y","label"]}"#.into(),
            },
            SkillNode {
                id: "companion.draw".into(),
                domain: "spatial_companion".into(),
                description: "Draw translucent annotation overlays on screen".into(),
                schema_json: r#"{"name":"draw_overlay","params":["shape","x","y","w","h"]}"#.into(),
            },
            SkillNode {
                id: "companion.cua".into(),
                domain: "spatial_companion".into(),
                description: "Permission-gated mouse/keyboard automation".into(),
                schema_json: r#"{"name":"cua_action","params":["target_app","action"]}"#.into(),
            },
            SkillNode {
                id: "builder.flashgrep".into(),
                domain: "code_builder".into(),
                description: "Sub-ms resident workspace symbol & file search".into(),
                schema_json: r#"{"name":"workspace_search","params":["query","max"]}"#.into(),
            },
            SkillNode {
                id: "builder.ptc_exec".into(),
                domain: "code_builder".into(),
                description: "Batch multi-tool calls into one isolated PTC script".into(),
                schema_json: r#"{"name":"ptc_run","params":["script"]}"#.into(),
            },
            SkillNode {
                id: "builder.miniapp".into(),
                domain: "code_builder".into(),
                description: "Generate or update an interactive Mini App panel".into(),
                schema_json: r#"{"name":"miniapp_build","params":["spec"]}"#.into(),
            },
            SkillNode {
                id: "system.sandbox_shell".into(),
                domain: "system_ops".into(),
                description: "Execute command inside macOS Seatbelt CapabilityEnvelope".into(),
                schema_json: r#"{"name":"sandbox_exec","params":["cmd","args"]}"#.into(),
            },
            SkillNode {
                id: "system.session_fork".into(),
                domain: "system_ops".into(),
                description: "Fork or replay append-only trajectory from SQLite WAL".into(),
                schema_json: r#"{"name":"session_fork","params":["session_id","seq"]}"#.into(),
            },
        ];
        Self { skills }
    }

    pub fn total_skills(&self) -> usize {
        self.skills.len()
    }

    pub fn route(&self, req: &TurnRequest, flags: &OptimizationFlags) -> SkillRouteResult {
        if !flags.skill_tree_pruning {
            return SkillRouteResult {
                domain_router: "all_unpruned".into(),
                loaded_skills: self.skills.clone(),
                total_catalog_skills: self.skills.len(),
                pruned_count: 0,
            };
        }

        let lower = req.prompt.to_lowercase();
        let domain = if req.lane == LaneKind::CompanionAmbient
            || lower.contains("point")
            || lower.contains("where")
            || lower.contains("click")
            || lower.contains("screen")
            || lower.contains("draw")
        {
            "spatial_companion"
        } else if lower.contains("sandbox")
            || lower.contains("fork")
            || lower.contains("replay")
            || lower.contains("permission")
        {
            "system_ops"
        } else {
            "code_builder"
        };

        let loaded: Vec<SkillNode> = self
            .skills
            .iter()
            .filter(|s| s.domain == domain)
            .cloned()
            .collect();
        let pruned_count = self.skills.len().saturating_sub(loaded.len());

        SkillRouteResult {
            domain_router: domain.to_string(),
            loaded_skills: loaded,
            total_catalog_skills: self.skills.len(),
            pruned_count,
        }
    }
}

/// Selects between Local Triage, Primary Cloud, and BYOK Cloud tiers.
#[derive(Debug, Clone, Default)]
pub struct TieredModelRouter;

impl TieredModelRouter {
    pub fn select_tier(&self, req: &TurnRequest, flags: &OptimizationFlags) -> ModelTier {
        let lower = req.prompt.to_lowercase();
        let complex = req.prompt.len() > 240
            || lower.contains("refactor")
            || lower.contains("architecture")
            || lower.contains("multi-step")
            || lower.contains("debug");

        if flags.local_triage && !complex {
            ModelTier::LocalTriage
        } else if lower.contains("claude") || lower.contains("byok") {
            ModelTier::ByokCloud
        } else {
            ModelTier::PrimaryCloud
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreparedPrompt {
    pub prefix_hash: String,
    pub cache_hit: bool,
    pub frozen_prefix: String,
    pub tail_context: String,
    pub model_tier: ModelTier,
    pub route: SkillRouteResult,
    pub ptc_eligible: bool,
}

#[derive(Debug, Clone, Default)]
struct OptStats {
    seen_prefixes: BTreeMap<String, u64>,
    hits: u64,
    misses: u64,
    last_loaded_skills: usize,
    last_model_tier: Option<ModelTier>,
    last_latency_ms: Option<u64>,
}

/// Layer-1 Optimization Middleware:
/// - Prefix-Cache Lock (stable sorted prefix hash + volatile state strictly in tail_context)
/// - Progressive Skill Tree Pruning
/// - PTC batching eligibility detection
/// - Signed Context Compaction into SQLite WAL
#[derive(Clone)]
pub struct OptimizationMiddleware {
    flags: Arc<RwLock<OptimizationFlags>>,
    stats: Arc<RwLock<OptStats>>,
    skill_router: SkillTreeRouter,
    model_router: TieredModelRouter,
}

impl Default for OptimizationMiddleware {
    fn default() -> Self {
        Self::new(OptimizationFlags::default())
    }
}

impl OptimizationMiddleware {
    pub fn new(initial: OptimizationFlags) -> Self {
        Self {
            flags: Arc::new(RwLock::new(initial)),
            stats: Arc::new(RwLock::new(OptStats::default())),
            skill_router: SkillTreeRouter::new(),
            model_router: TieredModelRouter,
        }
    }

    pub fn flags(&self) -> OptimizationFlags {
        self.flags.read().map(|g| g.clone()).unwrap_or_default()
    }

    pub fn set_flags(&self, new_flags: OptimizationFlags) {
        if let Ok(mut g) = self.flags.write() {
            *g = new_flags;
        }
    }

    pub fn total_skills(&self) -> usize {
        self.skill_router.total_skills()
    }

    pub fn metrics(&self) -> (u64, u64, f64, usize, Option<String>, Option<u64>) {
        let Ok(s) = self.stats.read() else {
            return (0, 0, 0.0, 0, None, None);
        };
        let total = s.hits + s.misses;
        let rate = if total == 0 {
            0.0
        } else {
            (s.hits as f64) / (total as f64)
        };
        let tier = s.last_model_tier.map(|t| format!("{t:?}"));
        (
            s.hits,
            s.misses,
            rate,
            s.last_loaded_skills,
            tier,
            s.last_latency_ms,
        )
    }

    pub fn record_latency(&self, latency_ms: u64) {
        if let Ok(mut s) = self.stats.write() {
            s.last_latency_ms = Some(latency_ms);
        }
    }

    /// Assembles a Prefix-Cache-Locked prompt where the system prompt and sorted
    /// skill schemas stay byte-identical across turns while volatile screen context
    /// and timestamps are isolated strictly in `tail_context`.
    pub fn prepare_turn(&self, req: &TurnRequest) -> PreparedPrompt {
        let flags = req
            .optimizations
            .clone()
            .unwrap_or_else(|| self.flags());

        let route = self.skill_router.route(req, &flags);
        let model_tier = self.model_router.select_tier(req, &flags);

        let mut sorted_schemas: Vec<&str> = route
            .loaded_skills
            .iter()
            .map(|s| s.schema_json.as_str())
            .collect();
        sorted_schemas.sort_unstable();

        // When Prefix-Cache Lock is OFF, simulate uncached dynamic timestamp busting prefix.
        let frozen_prefix = if flags.prefix_cache_lock {
            format!(
                "SYSTEM:AI_CARNIVAL_UNIFIED_V1\nDOMAIN:{}\nTOOLS:[{}]",
                route.domain_router,
                sorted_schemas.join(",")
            )
        } else {
            format!(
                "SYSTEM:AI_CARNIVAL_UNIFIED_V1\nREQ_ID:{}\nDOMAIN:{}\nTOOLS:[{}]",
                req.request_id,
                route.domain_router,
                sorted_schemas.join(",")
            )
        };

        let mut hasher = Sha256::new();
        hasher.update(frozen_prefix.as_bytes());
        let prefix_hash = format!("{:x}", hasher.finalize());

        let tail_context = serde_json::to_string(&json!({
            "request_id": req.request_id,
            "lane": req.lane,
            "screen_context": req.screen_context,
            "prompt": req.prompt,
        }))
        .unwrap_or_default();

        let ptc_eligible = flags.ptc_batching
            && (req.prompt.to_lowercase().contains("batch")
                || req.prompt.to_lowercase().contains("multi")
                || req.prompt.to_lowercase().contains("and then"));

        let cache_hit = if let Ok(mut stats) = self.stats.write() {
            let entry = stats.seen_prefixes.entry(prefix_hash.clone()).or_insert(0);
            *entry += 1;
            let hit = *entry > 1;
            if hit {
                stats.hits += 1;
            } else {
                stats.misses += 1;
            }
            stats.last_loaded_skills = route.loaded_skills.len();
            stats.last_model_tier = Some(model_tier);
            hit
        } else {
            false
        };

        PreparedPrompt {
            prefix_hash,
            cache_hit,
            frozen_prefix,
            tail_context,
            model_tier,
            route,
            ptc_eligible,
        }
    }

    /// Compacts historical events into a signed summary event in SQLite WAL
    /// while preserving all original append-only events.
    pub fn compact_session_summary(
        &self,
        store: &EventStore,
        session_id: &str,
    ) -> Result<String, layer0_store::StoreError> {
        let events = store.project_session_events(session_id)?;
        let mut hasher = Sha256::new();
        for ev in &events {
            hasher.update(ev.seq.to_string().as_bytes());
            hasher.update(ev.payload.to_string().as_bytes());
        }
        let digest = format!("{:x}", hasher.finalize());
        store.append_event(
            session_id,
            HarnessEventKind::Telemetry,
            json!({
                "compaction_summary": true,
                "covered_events": events.len(),
                "replay_digest": digest,
            }),
        )?;
        Ok(digest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use layer0_contracts::ScreenContext;

    #[test]
    fn prefix_cache_lock_and_skill_tree_pruning_work_across_volatile_turns() {
        let mw = OptimizationMiddleware::default();

        let req1 = TurnRequest {
            request_id: "r1".into(),
            session_id: "s1".into(),
            lane: LaneKind::CompanionAmbient,
            prompt: "Where is the export button?".into(),
            screen_context: Some(ScreenContext {
                active_app: Some("Figma".into()),
                cursor_x: Some(100),
                cursor_y: Some(200),
                ..Default::default()
            }),
            optimizations: None,
        };
        let req2 = TurnRequest {
            request_id: "r2".into(),
            session_id: "s1".into(),
            lane: LaneKind::CompanionAmbient,
            prompt: "Where is the save button?".into(),
            screen_context: Some(ScreenContext {
                active_app: Some("Xcode".into()),
                cursor_x: Some(900),
                cursor_y: Some(50),
                ..Default::default()
            }),
            optimizations: None,
        };

        let p1 = mw.prepare_turn(&req1);
        let p2 = mw.prepare_turn(&req2);

        assert!(!p1.cache_hit);
        assert!(p2.cache_hit);
        assert_eq!(p1.prefix_hash, p2.prefix_hash);
        assert!(p1.route.loaded_skills.len() < p1.route.total_catalog_skills);
        assert_eq!(p1.route.domain_router, "spatial_companion");
    }
}

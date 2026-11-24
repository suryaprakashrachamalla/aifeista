use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use layer0_contracts::{now_ms, CapabilityEnvelope, HarnessEventKind, SnapshotRecord};
use layer0_core::{CapabilitySeam, ImmutableCoreGuard, SeamError, SeamMetadata, SeamRegistry};
use layer0_store::{EventStore, StoreError};
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum WatchdogError {
    #[error("policy audit rejected target: {0}")]
    PolicyRejected(String),
    #[error("shadow dry-run failed for '{0}': {1}")]
    ShadowFailed(String, String),
    #[error("seam registry error: {0}")]
    Seam(#[from] SeamError),
    #[error("store error: {0}")]
    Store(#[from] StoreError),
    #[error("rollback also failed: {0}")]
    RollbackFailed(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RollbackReport {
    pub seam_id: String,
    pub snapshot_id: String,
    pub reason: String,
    pub rollback_ms: u64,
    pub restored_version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HotSwapOutcome {
    Committed {
        seam: SeamMetadata,
        snapshot: SnapshotRecord,
        elapsed_ms: u64,
    },
    RolledBack(RollbackReport),
}

pub type SeamFactory = Arc<dyn Fn() -> Arc<dyn CapabilitySeam> + Send + Sync>;

/// Layer-0 Watchdog supervising the 6-stage self-rebuilding lifecycle:
/// 1. Policy & CapabilityEnvelope Audit
/// 2. Shadow Dry-Run (mount + health_check in isolated registry)
/// 3. Atomic Checkpoint Snapshot in SQLite WAL
/// 4. Reversible Hot-Swap via EffectGuard
/// 5. Probation Health Probe
/// 6. Automatic <1s Rollback on failure or panic
pub struct Watchdog {
    registry: SeamRegistry,
    store: EventStore,
    core_guard: ImmutableCoreGuard,
}

impl Watchdog {
    pub fn new(registry: SeamRegistry, store: EventStore, core_guard: ImmutableCoreGuard) -> Self {
        Self {
            registry,
            store,
            core_guard,
        }
    }

    /// Audits proposed file paths + capabilities, shadow-tests the candidate seam,
    /// commits an atomic checkpoint, hot-swaps live, probes health, and rolls back in <1s if needed.
    pub fn hot_swap_seam(
        &self,
        edited_paths: &[PathBuf],
        allowed_envelope: &CapabilityEnvelope,
        previous_factory: Option<SeamFactory>,
        candidate_factory: SeamFactory,
        probation_probe: impl FnOnce(&SeamRegistry) -> Result<(), String>,
    ) -> Result<HotSwapOutcome, WatchdogError> {
        let start = Instant::now();

        // Stage 1: Static Policy & Capability Audit (fail-closed)
        for path in edited_paths {
            self.core_guard
                .check_layer1_write(path)
                .map_err(|e| WatchdogError::PolicyRejected(e.to_string()))?;
        }

        let candidate = candidate_factory();
        let seam_id = candidate.id().to_string();
        let req_caps = candidate.required_capabilities();
        if req_caps.network_allow && !allowed_envelope.network_allow {
            return Err(WatchdogError::PolicyRejected(format!(
                "seam '{seam_id}' requests disallowed network access"
            )));
        }
        if req_caps.allow_vision && !allowed_envelope.allow_vision {
            return Err(WatchdogError::PolicyRejected(format!(
                "seam '{seam_id}' requests disallowed vision capture"
            )));
        }
        if req_caps.allow_audio && !allowed_envelope.allow_audio {
            return Err(WatchdogError::PolicyRejected(format!(
                "seam '{seam_id}' requests disallowed audio capture"
            )));
        }

        // Stage 2: Shadow Dry-Run in isolated registry
        let live_ctx = self.registry.context()?;
        let shadow_registry = SeamRegistry::new(&live_ctx.workspace_root);
        for svc in &live_ctx.active_services {
            shadow_registry.register_service(svc.clone())?;
        }
        shadow_registry
            .mount(Arc::clone(&candidate))
            .map_err(|e| WatchdogError::ShadowFailed(seam_id.clone(), e.to_string()))?;
        drop(shadow_registry);

        // Stage 3: Atomic Checkpoint Snapshot of known-good graph BEFORE live mutation
        let current_seams = self.registry.list()?;
        let snapshot_id = format!("snap-{}", Uuid::new_v4());
        let snapshot = self.store.commit_snapshot(
            &snapshot_id,
            format!("pre-swap:{seam_id}"),
            serde_json::to_string(&current_seams).unwrap_or_else(|_| "[]".into()),
            true,
        )?;

        // Stage 4: Reversible Hot-Swap Mount (live)
        if let Err(err) = self.registry.mount(candidate) {
            return self.execute_rollback(
                &seam_id,
                &snapshot.id,
                format!("live mount failed: {err}"),
                previous_factory,
            );
        }

        // Stage 5: Probation Health Probe (catches Err AND panics)
        let probe_result = catch_unwind(AssertUnwindSafe(|| {
            for (id, status) in self.registry.health_check_all().map_err(|e| e.to_string())? {
                if let Err(err) = status {
                    return Err(format!("seam '{id}' unhealthy: {err}"));
                }
            }
            probation_probe(&self.registry)
        }));

        match probe_result {
            Ok(Ok(())) => {
                let meta = self
                    .registry
                    .list()?
                    .into_iter()
                    .find(|m| m.id == seam_id)
                    .unwrap_or(SeamMetadata {
                        id: seam_id,
                        version: "unknown".into(),
                        injects: vec![],
                        capabilities: req_caps,
                    });
                let elapsed_ms = start.elapsed().as_millis() as u64;
                let _ = self.store.append_event(
                    "watchdog-system",
                    HarnessEventKind::Telemetry,
                    json!({
                        "event": "hot_swap_committed",
                        "seam_id": meta.id,
                        "snapshot_id": snapshot.id,
                        "elapsed_ms": elapsed_ms,
                    }),
                );
                Ok(HotSwapOutcome::Committed {
                    seam: meta,
                    snapshot,
                    elapsed_ms,
                })
            }
            Ok(Err(reason)) => {
                self.execute_rollback(&seam_id, &snapshot.id, reason, previous_factory)
            }
            Err(_) => self.execute_rollback(
                &seam_id,
                &snapshot.id,
                "probation probe panicked".to_string(),
                previous_factory,
            ),
        }
    }

    fn execute_rollback(
        &self,
        seam_id: &str,
        snapshot_id: &str,
        reason: String,
        previous_factory: Option<SeamFactory>,
    ) -> Result<HotSwapOutcome, WatchdogError> {
        let rb_start = Instant::now();
        let _ = self.registry.unmount(seam_id);

        let restored_version = if let Some(factory) = previous_factory {
            let prev = factory();
            let ver = prev.version().to_string();
            self.registry
                .mount(prev)
                .map_err(|e| WatchdogError::RollbackFailed(e.to_string()))?;
            Some(ver)
        } else {
            None
        };

        let rollback_ms = rb_start.elapsed().as_millis() as u64;
        let _ = self.store.append_event(
            "watchdog-system",
            HarnessEventKind::Error,
            json!({
                "event": "watchdog_auto_rollback",
                "seam_id": seam_id,
                "snapshot_id": snapshot_id,
                "reason": reason,
                "rollback_ms": rollback_ms,
                "timestamp_ms": now_ms(),
            }),
        );

        Ok(HotSwapOutcome::RolledBack(RollbackReport {
            seam_id: seam_id.to_string(),
            snapshot_id: snapshot_id.to_string(),
            reason,
            rollback_ms,
            restored_version,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use layer0_core::{EffectGuard, SeamContext};
    use std::sync::atomic::{AtomicBool, Ordering};

    struct VersionedSeam {
        version: &'static str,
        unmounted: Arc<AtomicBool>,
    }

    impl CapabilitySeam for VersionedSeam {
        fn id(&self) -> &str {
            "layer1.opt.test"
        }
        fn version(&self) -> &str {
            self.version
        }
        fn declares_inject(&self) -> &[&str] {
            &["event_log"]
        }
        fn required_capabilities(&self) -> CapabilityEnvelope {
            CapabilityEnvelope::restricted(self.id(), vec![])
        }
        fn mount(&self, _ctx: &SeamContext) -> Result<EffectGuard, SeamError> {
            let mut g = EffectGuard::new();
            let flag = Arc::clone(&self.unmounted);
            g.defer(move || {
                flag.store(true, Ordering::SeqCst);
            });
            Ok(g)
        }
    }

    #[test]
    fn watchdog_rolls_back_broken_layer1_seam_under_1s() {
        let registry = SeamRegistry::new("/tmp/ws");
        let store = EventStore::open_in_memory().unwrap();
        let guard = ImmutableCoreGuard::for_workspace(PathBuf::from("/tmp/ws").as_path());
        let watchdog = Watchdog::new(registry.clone(), store, guard);

        let v1_unmounted = Arc::new(AtomicBool::new(false));
        let v1_factory: SeamFactory = {
            let flag = Arc::clone(&v1_unmounted);
            Arc::new(move || {
                Arc::new(VersionedSeam {
                    version: "1.0.0",
                    unmounted: Arc::clone(&flag),
                })
            })
        };
        registry.mount(v1_factory()).unwrap();

        let v2_unmounted = Arc::new(AtomicBool::new(false));
        let v2_factory: SeamFactory = {
            let flag = Arc::clone(&v2_unmounted);
            Arc::new(move || {
                Arc::new(VersionedSeam {
                    version: "2.0.0-broken",
                    unmounted: Arc::clone(&flag),
                })
            })
        };

        let envelope = CapabilityEnvelope::restricted("layer1.opt.test", vec![]);
        let outcome = watchdog
            .hot_swap_seam(
                &[PathBuf::from("crates/layer1-router/src/lib.rs")],
                &envelope,
                Some(v1_factory),
                v2_factory,
                |_reg| Err("simulated IPC hang during probation".into()),
            )
            .unwrap();

        match outcome {
            HotSwapOutcome::RolledBack(report) => {
                assert!(
                    report.rollback_ms < 1000,
                    "rollback took {}ms",
                    report.rollback_ms
                );
                assert_eq!(report.restored_version.as_deref(), Some("1.0.0"));
                assert!(v2_unmounted.load(Ordering::SeqCst));
                let active = registry.list().unwrap();
                assert_eq!(active.len(), 1);
                assert_eq!(active[0].version, "1.0.0");
            }
            other => panic!("expected rollback, got {other:?}"),
        }
    }

    #[test]
    fn watchdog_blocks_layer0_write_in_policy_audit() {
        let registry = SeamRegistry::new("/tmp/ws");
        let store = EventStore::open_in_memory().unwrap();
        let guard = ImmutableCoreGuard::for_workspace(PathBuf::from("/tmp/ws").as_path());
        let watchdog = Watchdog::new(registry, store, guard);

        let dummy_factory: SeamFactory = Arc::new(|| {
            Arc::new(VersionedSeam {
                version: "1.0.0",
                unmounted: Arc::new(AtomicBool::new(false)),
            })
        });
        let envelope = CapabilityEnvelope::restricted("layer1.opt.test", vec![]);
        let res = watchdog.hot_swap_seam(
            &[PathBuf::from("/tmp/ws/crates/layer0-core/src/lib.rs")],
            &envelope,
            None,
            dummy_factory,
            |_| Ok(()),
        );
        assert!(matches!(res, Err(WatchdogError::PolicyRejected(_))));
    }
}

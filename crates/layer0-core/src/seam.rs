use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use layer0_contracts::CapabilityEnvelope;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SeamError {
    #[error("missing required dependency seam '{0}'")]
    MissingDependency(String),
    #[error("seam '{0}' mount failed: {1}")]
    MountFailed(String, String),
    #[error("seam '{0}' health check failed: {1}")]
    HealthFailed(String, String),
    #[error("seam '{0}' not found")]
    NotFound(String),
    #[error("internal lock poisoned")]
    LockPoisoned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleHook {
    TurnStart,
    SystemPromptAssemble,
    ToolsPreExecute,
    ToolsExecute,
    TurnEnd,
    SessionEvent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeamMetadata {
    pub id: String,
    pub version: String,
    pub injects: Vec<String>,
    pub capabilities: CapabilityEnvelope,
}

/// Mirrors Cordis `ctx.effect()`: dropping `EffectGuard` runs cleanups in LIFO order
/// so seams never leak listeners, timers, IPC handles, or UI slots on unload/rollback.
#[derive(Default)]
pub struct EffectGuard {
    cleanups: std::sync::Mutex<Vec<Box<dyn FnOnce() + Send>>>,
}

impl EffectGuard {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn defer<F>(&mut self, cleanup: F)
    where
        F: FnOnce() + Send + 'static,
    {
        if let Ok(mut list) = self.cleanups.lock() {
            list.push(Box::new(cleanup));
        }
    }

    pub fn len(&self) -> usize {
        self.cleanups.lock().map(|l| l.len()).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Drop for EffectGuard {
    fn drop(&mut self) {
        if let Ok(mut list) = self.cleanups.lock() {
            while let Some(cleanup) = list.pop() {
                cleanup();
            }
        }
    }
}

/// Typed runtime context handed to seams during mount.
#[derive(Debug, Clone, Default)]
pub struct SeamContext {
    pub workspace_root: String,
    pub active_services: Vec<String>,
}

/// Capability seam contract implemented by Layer-1 plugins and adapters.
pub trait CapabilitySeam: Send + Sync {
    fn id(&self) -> &str;
    fn version(&self) -> &str;
    fn declares_inject(&self) -> &[&str];
    fn required_capabilities(&self) -> CapabilityEnvelope;
    fn mount(&self, ctx: &SeamContext) -> Result<EffectGuard, SeamError>;
    fn health_check(&self) -> Result<(), String> {
        Ok(())
    }
}

struct MountedEntry {
    seam: Arc<dyn CapabilitySeam>,
    _guard: EffectGuard,
    metadata: SeamMetadata,
}

/// Layer-0 thread-safe registry managing mounted seams and deterministic teardown.
#[derive(Clone)]
pub struct SeamRegistry {
    workspace_root: String,
    entries: Arc<RwLock<HashMap<String, MountedEntry>>>,
    services: Arc<RwLock<HashSet<String>>>,
}

impl SeamRegistry {
    pub fn new(workspace_root: impl Into<String>) -> Self {
        let mut base_services = HashSet::new();
        base_services.insert("event_log".to_string());
        base_services.insert("sandbox".to_string());
        base_services.insert("workspace_index".to_string());
        Self {
            workspace_root: workspace_root.into(),
            entries: Arc::new(RwLock::new(HashMap::new())),
            services: Arc::new(RwLock::new(base_services)),
        }
    }

    pub fn register_service(&self, name: impl Into<String>) -> Result<(), SeamError> {
        let mut services = self.services.write().map_err(|_| SeamError::LockPoisoned)?;
        services.insert(name.into());
        Ok(())
    }

    pub fn context(&self) -> Result<SeamContext, SeamError> {
        let services = self.services.read().map_err(|_| SeamError::LockPoisoned)?;
        Ok(SeamContext {
            workspace_root: self.workspace_root.clone(),
            active_services: services.iter().cloned().collect(),
        })
    }

    /// Mounts a seam after verifying declared `inject` dependencies.
    /// If already mounted, old `EffectGuard` is dropped before mounting replacement.
    pub fn mount(&self, seam: Arc<dyn CapabilitySeam>) -> Result<SeamMetadata, SeamError> {
        let ctx = self.context()?;
        let available: HashSet<String> = ctx.active_services.iter().cloned().collect();
        for dep in seam.declares_inject() {
            if !available.contains(*dep) {
                return Err(SeamError::MissingDependency((*dep).to_string()));
            }
        }

        // Unmount previous version first so resources release cleanly before re-mount.
        self.unmount(seam.id())?;

        let guard = seam.mount(&ctx)?;
        if let Err(reason) = seam.health_check() {
            drop(guard);
            return Err(SeamError::HealthFailed(seam.id().to_string(), reason));
        }

        let metadata = SeamMetadata {
            id: seam.id().to_string(),
            version: seam.version().to_string(),
            injects: seam
                .declares_inject()
                .iter()
                .map(|s| (*s).to_string())
                .collect(),
            capabilities: seam.required_capabilities(),
        };

        let mut entries = self.entries.write().map_err(|_| SeamError::LockPoisoned)?;
        entries.insert(
            seam.id().to_string(),
            MountedEntry {
                seam,
                _guard: guard,
                metadata: metadata.clone(),
            },
        );
        Ok(metadata)
    }

    pub fn unmount(&self, id: &str) -> Result<bool, SeamError> {
        let mut entries = self.entries.write().map_err(|_| SeamError::LockPoisoned)?;
        Ok(entries.remove(id).is_some())
    }

    pub fn health_check_all(&self) -> Result<Vec<(String, Result<(), String>)>, SeamError> {
        let entries = self.entries.read().map_err(|_| SeamError::LockPoisoned)?;
        Ok(entries
            .iter()
            .map(|(id, entry)| (id.clone(), entry.seam.health_check()))
            .collect())
    }

    pub fn list(&self) -> Result<Vec<SeamMetadata>, SeamError> {
        let entries = self.entries.read().map_err(|_| SeamError::LockPoisoned)?;
        let mut list: Vec<_> = entries.values().map(|e| e.metadata.clone()).collect();
        list.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(list)
    }

    pub fn get_seam(&self, id: &str) -> Result<Option<Arc<dyn CapabilitySeam>>, SeamError> {
        let entries = self.entries.read().map_err(|_| SeamError::LockPoisoned)?;
        Ok(entries.get(id).map(|e| Arc::clone(&e.seam)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;

    struct DummySeam {
        id: &'static str,
        version: &'static str,
        log: Arc<Mutex<Vec<String>>>,
        healthy: bool,
    }

    impl CapabilitySeam for DummySeam {
        fn id(&self) -> &str {
            self.id
        }
        fn version(&self) -> &str {
            self.version
        }
        fn declares_inject(&self) -> &[&str] {
            &["event_log"]
        }
        fn required_capabilities(&self) -> CapabilityEnvelope {
            CapabilityEnvelope::restricted(self.id, vec![])
        }
        fn mount(&self, _ctx: &SeamContext) -> Result<EffectGuard, SeamError> {
            let mut guard = EffectGuard::new();
            let log1 = Arc::clone(&self.log);
            let log2 = Arc::clone(&self.log);
            let ver1 = self.version.to_string();
            let ver2 = self.version.to_string();
            guard.defer(move || {
                log1.lock().unwrap().push(format!("first:{ver1}"));
            });
            guard.defer(move || {
                log2.lock().unwrap().push(format!("second:{ver2}"));
            });
            Ok(guard)
        }
        fn health_check(&self) -> Result<(), String> {
            if self.healthy {
                Ok(())
            } else {
                Err("unhealthy".into())
            }
        }
    }

    #[test]
    fn effect_guard_runs_lifo_on_unmount() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let registry = SeamRegistry::new("/tmp/ws");

        registry
            .mount(Arc::new(DummySeam {
                id: "opt.test",
                version: "1.0.0",
                log: Arc::clone(&log),
                healthy: true,
            }))
            .unwrap();

        assert_eq!(registry.list().unwrap().len(), 1);
        registry.unmount("opt.test").unwrap();
        assert_eq!(
            *log.lock().unwrap(),
            vec!["second:1.0.0".to_string(), "first:1.0.0".to_string()]
        );
    }

    #[test]
    fn missing_dependency_fails_mount() {
        struct BadDepSeam(Arc<AtomicUsize>);
        impl CapabilitySeam for BadDepSeam {
            fn id(&self) -> &str {
                "bad"
            }
            fn version(&self) -> &str {
                "0.1"
            }
            fn declares_inject(&self) -> &[&str] {
                &["nonexistent_service"]
            }
            fn required_capabilities(&self) -> CapabilityEnvelope {
                CapabilityEnvelope::restricted("bad", vec![])
            }
            fn mount(&self, _ctx: &SeamContext) -> Result<EffectGuard, SeamError> {
                self.0.fetch_add(1, Ordering::SeqCst);
                Ok(EffectGuard::new())
            }
        }

        let count = Arc::new(AtomicUsize::new(0));
        let registry = SeamRegistry::new("/tmp/ws");
        let res = registry.mount(Arc::new(BadDepSeam(Arc::clone(&count))));
        assert!(matches!(res, Err(SeamError::MissingDependency(_))));
        assert_eq!(count.load(Ordering::SeqCst), 0);
    }
}

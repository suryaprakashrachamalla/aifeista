use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use layer0_contracts::CapabilityEnvelope;
use layer0_core::{
    CapabilitySeam, EffectGuard, SearchMatch, SeamContext, SeamError, WorkspaceIndex,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpServerEntry {
    pub id: String,
    pub transport: String,
    pub endpoint: String,
    pub tools: Vec<String>,
    pub healthy: bool,
}

/// Warm persistent MCP / ACP connection pool with strict `mcp__server__tool`
/// namespacing so registering or restarting one server never busts another's schema.
#[derive(Clone, Default)]
pub struct WarmMcpPool {
    servers: Arc<RwLock<HashMap<String, McpServerEntry>>>,
}

impl WarmMcpPool {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_server(&self, mut entry: McpServerEntry) {
        let server_id = entry.id.clone();
        entry.tools = entry
            .tools
            .into_iter()
            .map(|t| {
                if t.starts_with("mcp__") {
                    t
                } else {
                    format!("mcp__{server_id}__{t}")
                }
            })
            .collect();
        if let Ok(mut map) = self.servers.write() {
            map.insert(server_id, entry);
        }
    }

    pub fn unregister_server(&self, server_id: &str) -> bool {
        self.servers
            .write()
            .map(|mut map| map.remove(server_id).is_some())
            .unwrap_or(false)
    }

    pub fn list(&self) -> Vec<McpServerEntry> {
        let mut items: Vec<_> = self
            .servers
            .read()
            .map(|m| m.values().cloned().collect())
            .unwrap_or_default();
        items.sort_by(|a, b| a.id.cmp(&b.id));
        items
    }
}

/// Adapter wrapping core workspace capabilities:
/// resident workspace index, warm MCP pool, and Layer-0 CapabilitySeam mounting.
#[derive(Clone)]
pub struct OpenBitFunAdapter {
    pub core_root: PathBuf,
    pub index: WorkspaceIndex,
    pub mcp_pool: WarmMcpPool,
}

impl OpenBitFunAdapter {
    pub fn new(repo_root: &Path, index: WorkspaceIndex) -> Self {
        let pool = WarmMcpPool::new();
        pool.register_server(McpServerEntry {
            id: "openbitfun_core".into(),
            transport: "stdio-warm".into(),
            endpoint: "crates/layer0-core".into(),
            tools: vec!["workspace_search".into(), "session_replay".into()],
            healthy: true,
        });
        Self {
            core_root: repo_root.join("crates/layer0-core"),
            index,
            mcp_pool: pool,
        }
    }

    pub fn is_present(&self) -> bool {
        self.core_root.join("Cargo.toml").exists()
    }

    pub fn search(&self, query: &str, max_results: usize) -> Vec<SearchMatch> {
        self.index.search(query, max_results)
    }
}

impl CapabilitySeam for OpenBitFunAdapter {
    fn id(&self) -> &str {
        "adapter.openbitfun"
    }

    fn version(&self) -> &str {
        "1.0.12-bridge"
    }

    fn declares_inject(&self) -> &[&str] {
        &["event_log", "workspace_index"]
    }

    fn required_capabilities(&self) -> CapabilityEnvelope {
        CapabilityEnvelope::restricted(
            "adapter.openbitfun",
            vec![self.core_root.display().to_string()],
        )
    }

    fn mount(&self, _ctx: &SeamContext) -> Result<EffectGuard, SeamError> {
        self.index.rebuild();
        let pool = self.mcp_pool.clone();
        let mut guard = EffectGuard::new();
        guard.defer(move || {
            pool.unregister_server("openbitfun_core");
        });
        Ok(guard)
    }

    fn health_check(&self) -> Result<(), String> {
        if self.mcp_pool.list().iter().all(|s| s.healthy) {
            Ok(())
        } else {
            Err("unhealthy warm mcp server".into())
        }
    }
}

use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use layer0_contracts::CapabilityEnvelope;
use layer0_core::{default_workspace_roots, ImmutableCoreGuard, SeamRegistry, WorkspaceIndex};
use layer0_sandbox::PrivacyGuard;
use layer0_store::EventStore;
use layer0_watchdog::{HotSwapOutcome, SeamFactory, Watchdog};
use layer1_adapters::{
    spawn_bridge_servers, AgentManager, AppState, OverlayState, RemoteHandoffManager,
    StudioRecipes, WorkspaceCoreAdapter,
};
use layer1_cordis_bus::{CordisBus, CordisConfig};
use layer1_router::OptimizationMiddleware;
use serde_json::json;

struct CliArgs {
    workspace: PathBuf,
    db_path: PathBuf,
    companion_port: u16,
    bridge_port: u16,
    self_check: bool,
}

fn parse_args() -> CliArgs {
    let args: Vec<String> = std::env::args().collect();
    let mut workspace = PathBuf::from(".");
    let mut db_path: Option<PathBuf> = None;
    let mut companion_port: u16 = 8787;
    let mut bridge_port: u16 = 32123;
    let mut self_check = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--workspace" if i + 1 < args.len() => {
                workspace = PathBuf::from(&args[i + 1]);
                i += 2;
            }
            "--db" if i + 1 < args.len() => {
                db_path = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            }
            "--companion-port" if i + 1 < args.len() => {
                companion_port = args[i + 1].parse().unwrap_or(8787);
                i += 2;
            }
            "--bridge-port" if i + 1 < args.len() => {
                bridge_port = args[i + 1].parse().unwrap_or(32123);
                i += 2;
            }
            "--self-check" => {
                self_check = true;
                i += 1;
            }
            _ => {
                i += 1;
            }
        }
    }

    let workspace = workspace.canonicalize().unwrap_or(workspace);
    let db_path = db_path.unwrap_or_else(|| workspace.join("carnival.db"));

    CliArgs {
        workspace,
        db_path,
        companion_port,
        bridge_port,
        self_check,
    }
}

async fn run_self_check(repo_root: &Path) -> Result<(), String> {
    let registry = SeamRegistry::new(repo_root.display().to_string());
    let store = EventStore::open_in_memory().map_err(|e| e.to_string())?;
    let guard = ImmutableCoreGuard::for_workspace(repo_root);
    let watchdog = Watchdog::new(registry.clone(), store.clone(), guard);

    let index = WorkspaceIndex::new(default_workspace_roots(repo_root));
    let core = WorkspaceCoreAdapter::new(repo_root, index);
    let core_factory: SeamFactory = {
        let adapter = core.clone();
        Arc::new(move || Arc::new(adapter.clone()))
    };

    let outcome = watchdog
        .hot_swap_seam(
            &[PathBuf::from("crates/layer1-adapters/src/workspace_core.rs")],
            &CapabilityEnvelope::restricted(
                "adapter.workspace_core",
                vec![repo_root.join("crates/layer0-core").display().to_string()],
            ),
            None,
            core_factory,
            |reg| {
                let list = reg.list().map_err(|e| e.to_string())?;
                if list.is_empty() {
                    return Err("expected workspace_core seam mounted".into());
                }
                Ok(())
            },
        )
        .map_err(|e| e.to_string())?;

    if !matches!(outcome, HotSwapOutcome::Committed { .. }) {
        return Err("self-check hot-swap did not commit".into());
    }

    let cordis = CordisBus::new(CordisConfig::default_for_repo(repo_root));
    let discovered = cordis
        .request("discover_harnesses", json!({}))
        .await
        .map_err(|e| e.to_string())?;
    let count = discovered["harnesses"]
        .as_array()
        .map(|a| a.len())
        .unwrap_or(0);
    cordis.shutdown().await.map_err(|e| e.to_string())?;

    println!(
        "Carnival self-check PASSED: sqlite_wal={}, indexed_files={}, discovered_modules={count}",
        store.journal_mode().unwrap_or_default(),
        core.index.file_count(),
    );
    Ok(())
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let cli = parse_args();

    if cli.self_check {
        if let Err(err) = run_self_check(&cli.workspace).await {
            eprintln!("self-check FAILED: {err}");
            std::process::exit(1);
        }
        return;
    }

    let registry = SeamRegistry::new(cli.workspace.display().to_string());
    let store = EventStore::open(&cli.db_path).expect("failed to open SQLite WAL store");
    let index = WorkspaceIndex::new(default_workspace_roots(&cli.workspace));
    let workspace_core = WorkspaceCoreAdapter::new(&cli.workspace, index);
    registry
        .mount(Arc::new(workspace_core.clone()))
        .expect("failed to mount workspace_core adapter");

    let cordis = CordisBus::new(CordisConfig::default_for_repo(&cli.workspace));
    let optimizer = OptimizationMiddleware::default();
    let privacy_guard = PrivacyGuard::new();
    let cua_envelope = CapabilityEnvelope {
        subject: "carnival-cua".into(),
        fs_read: vec![cli.workspace.display().to_string()],
        fs_write: vec![cli.workspace.join("crates").display().to_string()],
        network_allow: false,
        cua_apps: vec!["Xcode".into(), "Figma".into(), "Visual Studio Code".into()],
        allow_vision: true,
        allow_audio: true,
    };

    let state = AppState {
        registry: registry.clone(),
        store,
        cordis: cordis.clone(),
        optimizer,
        workspace_core: workspace_core.clone(),
        privacy_guard,
        cua_envelope,
        overlay_state: Arc::new(RwLock::new(OverlayState::default())),
        window_tx: None,
        agent_manager: AgentManager::new(&cli.workspace, Some(workspace_core)),
        studio_recipes: StudioRecipes::default_location(),
        remote_handoff: RemoteHandoffManager::default_location(&cli.workspace),
    };

    let bound = spawn_bridge_servers(state, cli.companion_port, cli.bridge_port)
        .await
        .expect("failed to bind bridge servers");

    tracing::info!(
        companion_endpoint = %bound.companion_addr,
        bridge_endpoint = %bound.bridge_addr,
        "Carnival daemon ready (Companion :8787 + Bridge/Telemetry :32123)"
    );

    let _ = tokio::signal::ctrl_c().await;
    tracing::info!("shutting down Carnival daemon...");
    let _ = cordis.shutdown().await;
    let _ = registry.unmount("adapter.workspace_core");
    bound.companion_handle.abort();
    bound.bridge_handle.abort();
}

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use layer0_contracts::CapabilityEnvelope;
use layer0_core::{default_workspace_roots, SeamRegistry, WorkspaceIndex};
use layer0_sandbox::PrivacyGuard;
use layer0_store::EventStore;
use layer1_adapters::{
    spawn_bridge_servers, AgentManager, AppState, BoundBridges, OverlayState,
    RemoteHandoffManager, StudioRecipes, WindowCommand, WorkspaceCoreAdapter,
};
use layer1_cordis_bus::{CordisBus, CordisConfig};
use layer1_router::OptimizationMiddleware;
#[cfg(target_os = "macos")]
use tauri::TitleBarStyle;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent};

struct ShellRuntime {
    registry: SeamRegistry,
    cordis: CordisBus,
    agent_manager: AgentManager,
    bridges: Mutex<Option<BoundBridges>>,
    runtime_child: Arc<Mutex<Option<Child>>>,
    shutdown_lock: tokio::sync::Mutex<()>,
    shutdown_done: std::sync::atomic::AtomicBool,
}

fn finish_sync_cleanup(rt: &ShellRuntime) {
    if rt.shutdown_done.load(std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    let stopped_agents = rt.agent_manager.stop_all();
    if stopped_agents > 0 {
        tracing::info!(count = stopped_agents, "Stopped active agent PTYs on exit");
    }
    if let Ok(mut child_guard) = rt.runtime_child.lock() {
        if let Some(child) = child_guard.take() {
            kill_child_process_group(child);
        }
    }
    let _ = rt.registry.unmount("adapter.workspace_core");
    if let Ok(mut guard) = rt.bridges.lock() {
        if let Some(b) = guard.take() {
            b.companion_handle.abort();
            b.bridge_handle.abort();
        }
    }
    rt.shutdown_done.store(true, std::sync::atomic::Ordering::SeqCst);
}

fn shutdown_shell_runtime(app_handle: &AppHandle) {
    if let Some(rt) = app_handle.try_state::<ShellRuntime>() {
        let _guard = rt.shutdown_lock.blocking_lock();
        if rt.shutdown_done.load(std::sync::atomic::Ordering::SeqCst) {
            return;
        }
        finish_sync_cleanup(&rt);
        let _ = tauri::async_runtime::block_on(rt.cordis.shutdown());
    }
}

async fn shutdown_shell_runtime_async(app_handle: &AppHandle) {
    if let Some(rt) = app_handle.try_state::<ShellRuntime>() {
        let _guard = rt.shutdown_lock.lock().await;
        if rt.shutdown_done.load(std::sync::atomic::Ordering::SeqCst) {
            return;
        }
        let am = rt.agent_manager.clone();
        let _ = tokio::task::spawn_blocking(move || am.stop_all()).await;
        let child_opt = rt.runtime_child.lock().ok().and_then(|mut g| g.take());
        if let Some(child) = child_opt {
            let _ = tokio::task::spawn_blocking(move || kill_child_process_group(child)).await;
        }
        let _ = rt.cordis.shutdown().await;
        let _ = rt.registry.unmount("adapter.workspace_core");
        if let Ok(mut guard) = rt.bridges.lock() {
            if let Some(b) = guard.take() {
                b.companion_handle.abort();
                b.bridge_handle.abort();
            }
        }
        rt.shutdown_done.store(true, std::sync::atomic::Ordering::SeqCst);
    }
}

fn bundle_resources_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let contents = exe.parent()?.parent()?;
    let res = contents.join("Resources");
    if res.join("runtime/lib/bin.js").exists() || res.join("runtime/apps/cli/lib/bin.js").exists() {
        Some(res)
    } else {
        None
    }
}

fn find_repo_root() -> Option<PathBuf> {
    if let Ok(env_root) = std::env::var("CARNIVAL_ROOT") {
        let p = PathBuf::from(env_root);
        if p.join("crates").exists() && p.join("runtime").exists() {
            return Some(p);
        }
    }
    let mut cur = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    for _ in 0..6 {
        if cur.join("crates").exists() && cur.join("runtime").exists() {
            return Some(cur);
        }
        if !cur.pop() {
            break;
        }
    }
    let fallback = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .ok()?;
    if fallback.join("crates").exists() && fallback.join("runtime").exists() {
        Some(fallback)
    } else {
        None
    }
}

fn resolve_workspace_root(repo_root: Option<&Path>, resources_dir: Option<&Path>) -> PathBuf {
    if let Some(r) = repo_root {
        return r.to_path_buf();
    }
    if let Ok(home) = std::env::var("HOME") {
        let docs = PathBuf::from(&home).join("Documents");
        if docs.exists() {
            return docs;
        }
        return PathBuf::from(home);
    }
    resources_dir
        .map(|r| r.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}

fn resolve_node_bin(resources_dir: Option<&Path>) -> String {
    if let Some(res) = resources_dir {
        let bundled = res.join("bin/node");
        if bundled.exists() {
            return bundled.display().to_string();
        }
    }
    for candidate in [
        "/opt/homebrew/bin/node",
        "/usr/local/bin/node",
        "/usr/bin/node",
    ] {
        if Path::new(candidate).exists() {
            return candidate.to_string();
        }
    }
    "node".to_string()
}

fn augmented_path(resources_dir: Option<&Path>) -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    let mut parts = Vec::new();
    if let Some(res) = resources_dir {
        parts.push(res.join("bin").display().to_string());
    }
    parts.push("/opt/homebrew/bin".to_string());
    parts.push("/usr/local/bin".to_string());
    if !home.is_empty() {
        parts.push(format!("{home}/.cargo/bin"));
    }
    if let Ok(existing) = std::env::var("PATH") {
        if !existing.is_empty() {
            parts.push(existing);
        }
    } else {
        parts.push("/usr/bin:/bin:/usr/sbin:/sbin".to_string());
    }
    parts.join(":")
}

fn pick_free_web_port() -> u16 {
    if let Ok(listener) = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 3080))) {
        drop(listener);
        return 3080;
    }
    if let Ok(listener) = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0))) {
        let port = listener.local_addr().map(|a| a.port()).unwrap_or(3081);
        drop(listener);
        return port;
    }
    3080
}

fn is_web_runtime_ready(addr: SocketAddr) -> bool {
    TcpStream::connect_timeout(&addr, Duration::from_millis(200)).is_ok()
}

/// Checks whether an existing Carnival desktop bridge is already listening on :32123.
/// If so, focuses its main window cleanly so double-launching does not spawn duplicates.
fn focus_existing_instance_if_running() -> bool {
    let addr = SocketAddr::from(([127, 0, 0, 1], 32123));
    let Ok(mut stream) = TcpStream::connect_timeout(&addr, Duration::from_millis(250)) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_millis(350)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(350)));
    let req = "GET /health HTTP/1.1\r\nHost: 127.0.0.1:32123\r\nConnection: close\r\n\r\n";
    if stream.write_all(req.as_bytes()).is_err() {
        return false;
    }
    let mut resp = String::new();
    let _ = stream.read_to_string(&mut resp);
    if !resp.contains("200") || !resp.contains("\"ok\":true") {
        return false;
    }

    if let Ok(mut focus_stream) = TcpStream::connect_timeout(&addr, Duration::from_millis(250)) {
        let body = r#"{"action":"focus"}"#;
        let post_req = format!(
            "POST /window/main HTTP/1.1\r\nHost: 127.0.0.1:32123\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        let _ = focus_stream.write_all(post_req.as_bytes());
    }
    true
}

fn spawn_runtime_web(
    node_bin: &str,
    harness_dir: &Path,
    path_env: &str,
    port_str: &str,
    web_port: u16,
    web_url_slot: Arc<Mutex<Option<String>>>,
    last_err_slot: Arc<Mutex<Option<String>>>,
) -> Option<Child> {
    let deployed_bin = harness_dir.join("lib/bin.js");
    let built_bin = harness_dir.join("apps/cli/lib/bin.js");
    let args: Vec<&str> = if deployed_bin.exists() {
        vec!["lib/bin.js", "web", "--no-open", "--port", port_str]
    } else if built_bin.exists() {
        vec!["apps/cli/lib/bin.js", "web", "--no-open", "--port", port_str]
    } else {
        vec![
            "--import",
            "tsx/esm",
            "apps/cli/src/bin.ts",
            "web",
            "--no-open",
            "--port",
            port_str,
        ]
    };
    let mut cmd = Command::new(node_bin);
    cmd.args(args)
        .current_dir(harness_dir)
        .env("PATH", path_env)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    #[cfg(unix)]
    cmd.process_group(0);

    match cmd.spawn() {
        Ok(mut child) => {
            if let Some(stdout) = child.stdout.take() {
                let url_slot = Arc::clone(&web_url_slot);
                let url_prefix = format!("http://127.0.0.1:{web_port}");
                std::thread::spawn(move || {
                    let reader = BufReader::new(stdout);
                    for line in reader.lines().map_while(Result::ok) {
                        println!("{line}");
                        if let Some(idx) = line.find(&url_prefix) {
                            let url = line[idx..].trim().to_string();
                            if let Ok(mut g) = url_slot.lock() {
                                *g = Some(url);
                            }
                        }
                    }
                });
            }
            if let Some(stderr) = child.stderr.take() {
                let err_slot = Arc::clone(&last_err_slot);
                std::thread::spawn(move || {
                    let reader = BufReader::new(stderr);
                    for line in reader.lines().map_while(Result::ok) {
                        tracing::warn!(target: "dsh_web", "{line}");
                        if !line.trim().is_empty() {
                            if let Ok(mut g) = err_slot.lock() {
                                *g = Some(line);
                            }
                        }
                    }
                });
            }
            Some(child)
        }
        Err(err) => {
            tracing::error!(error = %err, "Failed to spawn Carnival workspace server");
            if let Ok(mut g) = last_err_slot.lock() {
                *g = Some(err.to_string());
            }
            None
        }
    }
}

fn kill_child_process_group(mut child: Child) {
    let pid = child.id();
    #[cfg(unix)]
    {
        let pgid_arg = format!("-{pid}");
        let _ = Command::new("/bin/kill").args(["-TERM", &pgid_arg]).status();
        let deadline = Instant::now() + Duration::from_millis(850);
        while Instant::now() < deadline {
            if let Ok(Some(_)) = child.try_wait() {
                return;
            }
            std::thread::sleep(Duration::from_millis(40));
        }
        let _ = Command::new("/bin/kill").args(["-KILL", &pgid_arg]).status();
    }
    let _ = child.kill();
    let _ = child.wait();
}

#[tauri::command]
fn toggle_spotlight(app: AppHandle) -> Result<bool, String> {
    let win = app
        .get_webview_window("spotlight")
        .ok_or_else(|| "spotlight window not found".to_string())?;
    let visible = win.is_visible().map_err(|e| e.to_string())?;
    let next = !visible;
    if next {
        let _ = win.center();
        win.show().map_err(|e| e.to_string())?;
        let _ = win.set_focus();
    } else {
        win.hide().map_err(|e| e.to_string())?;
    }
    Ok(next)
}

#[tauri::command]
fn toggle_companion_overlay(app: AppHandle) -> Result<bool, String> {
    let window = app
        .get_webview_window("companion-overlay")
        .ok_or_else(|| "companion-overlay window not found".to_string())?;
    let currently_visible = window.is_visible().map_err(|e| e.to_string())?;
    let next_visible = !currently_visible;
    if next_visible {
        window.show().map_err(|e| e.to_string())?;
    } else {
        window.hide().map_err(|e| e.to_string())?;
    }
    Ok(next_visible)
}

#[tauri::command]
fn set_overlay_click_through(app: AppHandle, ignore: bool) -> Result<(), String> {
    let window = app
        .get_webview_window("companion-overlay")
        .ok_or_else(|| "companion-overlay window not found".to_string())?;
    window
        .set_ignore_cursor_events(ignore)
        .map_err(|e| e.to_string())
}

fn dispatch_window_command(app: &AppHandle, cmd: WindowCommand) {
    match cmd {
        WindowCommand::ToggleSpotlight => {
            if let Some(win) = app.get_webview_window("spotlight") {
                if win.is_visible().unwrap_or(false) {
                    let _ = win.hide();
                } else {
                    let _ = win.center();
                    let _ = win.show();
                    let _ = win.set_focus();
                }
            }
        }
        WindowCommand::ShowSpotlight => {
            if let Some(win) = app.get_webview_window("spotlight") {
                let _ = win.center();
                let _ = win.show();
                let _ = win.set_focus();
            }
        }
        WindowCommand::HideSpotlight => {
            if let Some(win) = app.get_webview_window("spotlight") {
                let _ = win.hide();
            }
        }
        WindowCommand::ToggleOverlay => {
            if let Some(win) = app.get_webview_window("companion-overlay") {
                if win.is_visible().unwrap_or(false) {
                    let _ = win.hide();
                } else {
                    let _ = win.show();
                }
            }
        }
        WindowCommand::SetOverlayVisible(visible) => {
            if let Some(win) = app.get_webview_window("companion-overlay") {
                if visible {
                    let _ = win.show();
                } else {
                    let _ = win.hide();
                }
            }
        }
        WindowCommand::SetOverlayClickThrough(ignore) => {
            if let Some(win) = app.get_webview_window("companion-overlay") {
                let _ = win.set_ignore_cursor_events(ignore);
            }
        }
        WindowCommand::FocusMain => {
            if let Some(win) = app.get_webview_window("main") {
                let _ = win.show();
                let _ = win.unminimize();
                let _ = win.set_focus();
            }
        }
    }
}

pub fn run() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .try_init();

    if focus_existing_instance_if_running() {
        tracing::info!("Existing Carnival instance focused; exiting duplicate launch cleanly.");
        return;
    }

    tauri::Builder::default()
        .setup(|app| {
            let resources_dir = bundle_resources_dir();
            let repo_root_opt = find_repo_root();
            let workspace_root =
                resolve_workspace_root(repo_root_opt.as_deref(), resources_dir.as_deref());
            let runtime_dir = resources_dir
                .as_ref()
                .map(|r| r.join("runtime"))
                .filter(|p| p.exists())
                .or_else(|| repo_root_opt.as_ref().map(|r| r.join("runtime")))
                .unwrap_or_else(|| workspace_root.join("runtime"));
            let node_bin = resolve_node_bin(resources_dir.as_deref());
            let path_env = augmented_path(resources_dir.as_deref());

            let registry = SeamRegistry::new(workspace_root.display().to_string());
            let db_path = match app.path().app_data_dir() {
                Ok(dir) => {
                    let _ = std::fs::create_dir_all(&dir);
                    dir.join("carnival.db")
                }
                Err(_) => workspace_root.join("carnival.db"),
            };
            let store = EventStore::open(&db_path).expect("failed to open SQLite WAL store");

            let mut index_roots = default_workspace_roots(&workspace_root);
            if let Some(res) = resources_dir.as_ref() {
                if repo_root_opt.is_none() {
                    index_roots = vec![res.join("sidecars"), res.join("runtime/packages")];
                }
            }
            let index = WorkspaceIndex::new(index_roots);
            let adapter_anchor = if repo_root_opt.is_some() {
                workspace_root.clone()
            } else {
                resources_dir.clone().unwrap_or_else(|| workspace_root.clone())
            };
            let workspace_core = WorkspaceCoreAdapter::new(&adapter_anchor, index);
            registry
                .mount(Arc::new(workspace_core.clone()))
                .expect("failed to mount workspace_core adapter");

            let mut cordis_cfg = CordisConfig::default_for_repo(&workspace_root);
            cordis_cfg.node_bin = node_bin.clone();
            if let Some(res) = resources_dir.as_ref() {
                let bundled_script = res.join("sidecars/cordis-bridge/index.mjs");
                if bundled_script.exists() {
                    cordis_cfg.script_path = bundled_script;
                }
            }
            let cordis = CordisBus::new(cordis_cfg);
            let optimizer = OptimizationMiddleware::default();
            let privacy_guard = PrivacyGuard::new();
            let cua_envelope = CapabilityEnvelope {
                subject: "carnival-cua".into(),
                fs_read: vec![workspace_root.display().to_string()],
                fs_write: vec![workspace_root.display().to_string()],
                network_allow: false,
                cua_apps: vec!["Xcode".into(), "Figma".into(), "Visual Studio Code".into()],
                allow_vision: true,
                allow_audio: true,
            };

            let (window_tx, mut window_rx) = tokio::sync::mpsc::unbounded_channel();
            let agent_manager = AgentManager::new(&workspace_root, Some(workspace_core.clone()));

            let state = AppState {
                registry: registry.clone(),
                store,
                cordis: cordis.clone(),
                optimizer,
                workspace_core,
                privacy_guard,
                cua_envelope,
                overlay_state: Arc::new(RwLock::new(OverlayState::with_saved_prefs())),
                window_tx: Some(window_tx),
                agent_manager: agent_manager.clone(),
                studio_recipes: StudioRecipes::default_location(),
                remote_handoff: RemoteHandoffManager::default_location(&workspace_root),
            };

            let runtime_handle = tauri::async_runtime::spawn(async move {
                for attempt in 0..6 {
                    match spawn_bridge_servers(state.clone(), 8787, 32123).await {
                        Ok(bound) => return Ok(bound),
                        Err(err) => {
                            if attempt == 5 {
                                tracing::warn!(error = %err, "Retrying bridge on ephemeral companion port");
                                return spawn_bridge_servers(state.clone(), 0, 32123).await;
                            }
                            tokio::time::sleep(Duration::from_millis(160)).await;
                        }
                    }
                }
                spawn_bridge_servers(state, 0, 32123).await
            });

            let bound = tauri::async_runtime::block_on(runtime_handle)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?
                .ok();

            let web_port = pick_free_web_port();
            let web_addr = SocketAddr::from(([127, 0, 0, 1], web_port));
            let web_url_slot: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
            let last_err_slot: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
            let port_str = web_port.to_string();

            tracing::info!(
                dir = %runtime_dir.display(),
                node = %node_bin,
                port = web_port,
                "Launching Carnival workspace server"
            );

            let runtime_child = spawn_runtime_web(
                &node_bin,
                &runtime_dir,
                &path_env,
                &port_str,
                web_port,
                Arc::clone(&web_url_slot),
                Arc::clone(&last_err_slot),
            );

            let main_builder = WebviewWindowBuilder::new(
                app,
                "main",
                WebviewUrl::App("index.html".into()),
            )
            .title("Carnival")
            .inner_size(1360.0, 880.0)
            .transparent(true)
            .resizable(true)
            .initialization_script(include_str!("carnival_shell.js"));

            #[cfg(target_os = "macos")]
            let main_builder = main_builder
                .title_bar_style(TitleBarStyle::Overlay)
                .hidden_title(true);

            let main_win = main_builder.build()?;

            #[cfg(target_os = "macos")]
            {
                let main_handle = main_win.clone();
                let app_for_close = app.handle().clone();
                main_win.on_window_event(move |event| {
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = main_handle.hide();
                        if let Some(sp) = app_for_close.get_webview_window("spotlight") {
                            let _ = sp.hide();
                        }
                    }
                });
            }

            #[cfg(target_os = "macos")]
            {
                use window_vibrancy::{
                    apply_liquid_glass, apply_vibrancy, LiquidGlassOptions,
                    NSGlassEffectViewStyle, NSVisualEffectMaterial, NSVisualEffectState,
                };
                let main_glass = LiquidGlassOptions::new(NSGlassEffectViewStyle::Clear)
                    .tint_color((6, 6, 8, 96))
                    .radius(22.0)
                    .interactive(true);
                if let Err(err) = apply_liquid_glass(&main_win, main_glass) {
                    tracing::warn!(
                        error = %err,
                        "apply_liquid_glass unavailable on main, falling back to NSVisualEffectView"
                    );
                    let _ = apply_vibrancy(
                        &main_win,
                        NSVisualEffectMaterial::HudWindow,
                        Some(NSVisualEffectState::Active),
                        Some(22.0),
                    );
                }

                if let Some(spot_win) = app.get_webview_window("spotlight") {
                    let spot_handle = spot_win.clone();
                    spot_win.on_window_event(move |event| match event {
                        WindowEvent::CloseRequested { api, .. } => {
                            api.prevent_close();
                            let _ = spot_handle.hide();
                        }
                        WindowEvent::Focused(false) => {
                            let _ = spot_handle.hide();
                        }
                        _ => {}
                    });

                    let spot_glass = LiquidGlassOptions::new(NSGlassEffectViewStyle::Clear)
                        .tint_color((8, 8, 10, 108))
                        .radius(26.0)
                        .interactive(true);
                    if let Err(err) = apply_liquid_glass(&spot_win, spot_glass) {
                        tracing::warn!(
                            error = %err,
                            "apply_liquid_glass unavailable on spotlight, falling back to NSVisualEffectView"
                        );
                        let _ = apply_vibrancy(
                            &spot_win,
                            NSVisualEffectMaterial::HudWindow,
                            Some(NSVisualEffectState::Active),
                            Some(28.0),
                        );
                    }
                }
            }

            let shared_runtime = Arc::new(Mutex::new(runtime_child));
            app.manage(ShellRuntime {
                registry,
                cordis,
                agent_manager,
                bridges: Mutex::new(bound),
                runtime_child: Arc::clone(&shared_runtime),
                shutdown_lock: tokio::sync::Mutex::new(()),
                shutdown_done: std::sync::atomic::AtomicBool::new(false),
            });

            #[cfg(unix)]
            {
                let sig_app = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    use tokio::signal::unix::{signal, SignalKind};
                    let mut term = signal(SignalKind::terminate()).ok();
                    let mut int = signal(SignalKind::interrupt()).ok();
                    tokio::select! {
                        _ = async {
                            if let Some(ref mut s) = term { s.recv().await; }
                            else { std::future::pending::<()>().await; }
                        } => {}
                        _ = async {
                            if let Some(ref mut s) = int { s.recv().await; }
                            else { std::future::pending::<()>().await; }
                        } => {}
                    }
                    tracing::info!("Received termination signal; shutting down Carnival cleanly");
                    shutdown_shell_runtime_async(&sig_app).await;
                    std::process::exit(0);
                });
            }

            if let Some(overlay) = app.get_webview_window("companion-overlay") {
                let _ = overlay.set_ignore_cursor_events(true);
            }

            let win_app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                while let Some(cmd) = window_rx.recv().await {
                    dispatch_window_command(&win_app_handle, cmd);
                }
            });

            // Boot supervisor: monitors workspace server child (with 1 automatic retry) and navigates main cleanly
            let nav_app_handle = app.handle().clone();
            let nav_url_slot = Arc::clone(&web_url_slot);
            let nav_err_slot = Arc::clone(&last_err_slot);
            let nav_runtime = Arc::clone(&shared_runtime);
            tauri::async_runtime::spawn(async move {
                let mut retried = false;
                for tick in 0..240 {
                    // Restart workspace server once if it exits prematurely during boot
                    if let Ok(mut dst) = nav_runtime.lock() {
                        if let Some(child) = dst.as_mut() {
                            if let Ok(Some(status)) = child.try_wait() {
                                tracing::warn!(?status, "Carnival workspace server exited prematurely");
                                *dst = None;
                                if !retried {
                                    retried = true;
                                    tracing::info!("Retrying Carnival workspace server launch...");
                                    let replacement = spawn_runtime_web(
                                        &node_bin,
                                        &runtime_dir,
                                        &path_env,
                                        &port_str,
                                        web_port,
                                        Arc::clone(&nav_url_slot),
                                        Arc::clone(&nav_err_slot),
                                    );
                                    *dst = replacement;
                                } else if let Some(main_win) = nav_app_handle.get_webview_window("main") {
                                    let err_msg = nav_err_slot
                                        .lock()
                                        .ok()
                                        .and_then(|g| g.clone())
                                        .unwrap_or_else(|| format!("Workspace exited with {status}"));
                                    let quoted = serde_json::to_string(&err_msg).unwrap_or_else(|_| "\"Startup error\"".into());
                                    let _ = main_win.eval(&format!("window.setBootError && window.setBootError({quoted});"));
                                    break;
                                }
                            }
                        }
                    }

                    let target_url = nav_url_slot.lock().ok().and_then(|g| g.clone());
                    if let Some(url_str) = target_url {
                        if is_web_runtime_ready(web_addr) {
                            if let Some(main_win) = nav_app_handle.get_webview_window("main") {
                                let base_url_str = format!("http://127.0.0.1:{web_port}/");
                                if let Ok(base_url) = tauri::Url::parse(&base_url_str) {
                                    tracing::info!(url = %base_url, "Entering Carnival origin for same-site auth");
                                    let _ = main_win.navigate(base_url);
                                }
                                let quoted_url = serde_json::to_string(&url_str)
                                    .unwrap_or_else(|_| "\"\"".to_string());
                                let js = format!(
                                    "if (document.title === 'Connect to this Host' || document.querySelector('dsh-device-pairing')) {{ window.location.replace({quoted_url}); }}"
                                );
                                for _ in 0..24 {
                                    tokio::time::sleep(Duration::from_millis(250)).await;
                                    let _ = main_win.eval(&js);
                                }
                            }
                            break;
                        }
                    } else if tick == 16 {
                        if let Some(main_win) = nav_app_handle.get_webview_window("main") {
                            let _ = main_win.eval("window.setBootStatus && window.setBootStatus('Warming local runtime…');");
                        }
                    }
                    tokio::time::sleep(Duration::from_millis(250)).await;
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            toggle_spotlight,
            toggle_companion_overlay,
            set_overlay_click_through
        ])
        .build(tauri::generate_context!())
        .expect("error while building Carnival Tauri application")
        .run(|app_handle, event| {
            match event {
                #[cfg(target_os = "macos")]
                tauri::RunEvent::Reopen { .. } => {
                    if let Some(main_win) = app_handle.get_webview_window("main") {
                        let _ = main_win.show();
                        let _ = main_win.unminimize();
                        let _ = main_win.set_focus();
                    }
                }
                tauri::RunEvent::Exit => {
                    shutdown_shell_runtime(app_handle);
                }
                _ => {}
            }
        });
}

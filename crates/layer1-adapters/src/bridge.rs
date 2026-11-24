use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Instant;

use axum::body::Body;
use axum::extract::{Path, Query, Request, State};
use axum::http::header::{
    ACCESS_CONTROL_ALLOW_HEADERS, ACCESS_CONTROL_ALLOW_METHODS, ACCESS_CONTROL_ALLOW_ORIGIN,
    CACHE_CONTROL, CONTENT_TYPE,
};
use axum::http::{HeaderValue, Method, StatusCode};
use axum::middleware::{self, Next};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use layer0_contracts::{
    now_ms, CapabilityEnvelope, CuaActionRequest, HarnessEventKind, LaneKind, OptimizationFlags,
    ScreenContext, SpatialCommand, TelemetrySnapshot, TurnRequest,
};
use layer0_core::{
    CapabilitySeam, EffectGuard, ImmutableCoreGuard, SeamContext, SeamError, SeamRegistry,
};
use layer0_sandbox::{validate_cua_action, PrivacyGuard, PrivacyScope};
use layer0_store::EventStore;
use layer0_watchdog::{HotSwapOutcome, SeamFactory, Watchdog};
use layer1_cordis_bus::CordisBus;
use layer1_router::OptimizationMiddleware;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;
use uuid::Uuid;

use crate::agent_manager::{AgentManager, AgentViewMode, SpawnAgentRequest};
use crate::openbitfun::OpenBitFunAdapter;
use crate::remote_handoff::{
    AddPeerRequest, DispatchHandoffRequest, ReceiveHandoffRequest, RemoteHandoffManager,
};
use crate::studio_recipes::{SaveRecipeRequest, StudioRecipes};

const AGENT_MANAGER_HTML: &str = include_str!("../../../desktop/web-shell/agent-manager.html");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverlayState {
    pub visible: bool,
    pub click_through: bool,
    pub speech_text: Option<String>,
    pub commands: Vec<SpatialCommand>,
    pub glass_opacity: f32,
    pub glass_tint: String,
    pub updated_at_ms: i64,
}

impl Default for OverlayState {
    fn default() -> Self {
        Self {
            visible: false,
            click_through: true,
            speech_text: None,
            commands: Vec::new(),
            glass_opacity: 0.80,
            glass_tint: "obsidian".to_string(),
            updated_at_ms: now_ms(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UiPrefs {
    pub glass_opacity: f32,
    pub glass_tint: String,
}

impl Default for UiPrefs {
    fn default() -> Self {
        Self {
            glass_opacity: 0.80,
            glass_tint: "obsidian".to_string(),
        }
    }
}

pub fn normalize_glass_tint(raw: &str) -> String {
    match raw.trim().to_lowercase().as_str() {
        "jet" | "jet-black" => "jet".to_string(),
        "warm-smoke" | "warm" | "warm_smoke" => "warm-smoke".to_string(),
        _ => "obsidian".to_string(),
    }
}

pub fn clamp_glass_opacity(op: f32) -> f32 {
    if !op.is_finite() {
        0.80
    } else {
        op.clamp(0.08, 0.96)
    }
}

pub fn default_ui_prefs_path() -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".config/ai-carnival/ui_prefs.json")
    } else {
        PathBuf::from("/tmp/ai-carnival-ui-prefs.json")
    }
}

pub fn load_ui_prefs_from(path: &std::path::Path) -> UiPrefs {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return UiPrefs::default();
    };
    let Ok(mut prefs) = serde_json::from_str::<UiPrefs>(&raw) else {
        return UiPrefs::default();
    };
    prefs.glass_opacity = clamp_glass_opacity(prefs.glass_opacity);
    prefs.glass_tint = normalize_glass_tint(&prefs.glass_tint);
    prefs
}

pub fn save_ui_prefs_to(path: &std::path::Path, prefs: &UiPrefs) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let normalized = UiPrefs {
        glass_opacity: clamp_glass_opacity(prefs.glass_opacity),
        glass_tint: normalize_glass_tint(&prefs.glass_tint),
    };
    if let Ok(json_str) = serde_json::to_string_pretty(&normalized) {
        let _ = std::fs::write(path, json_str);
    }
}

impl OverlayState {
    pub fn with_saved_prefs() -> Self {
        let prefs = load_ui_prefs_from(&default_ui_prefs_path());
        Self {
            glass_opacity: prefs.glass_opacity,
            glass_tint: prefs.glass_tint,
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone)]
pub enum WindowCommand {
    ToggleSpotlight,
    ShowSpotlight,
    HideSpotlight,
    ToggleOverlay,
    SetOverlayVisible(bool),
    SetOverlayClickThrough(bool),
    FocusMain,
}

#[derive(Clone)]
pub struct AppState {
    pub registry: SeamRegistry,
    pub store: EventStore,
    pub cordis: CordisBus,
    pub optimizer: OptimizationMiddleware,
    pub openbitfun: OpenBitFunAdapter,
    pub privacy_guard: PrivacyGuard,
    pub cua_envelope: CapabilityEnvelope,
    pub overlay_state: Arc<RwLock<OverlayState>>,
    pub window_tx: Option<tokio::sync::mpsc::UnboundedSender<WindowCommand>>,
    pub agent_manager: AgentManager,
    pub studio_recipes: StudioRecipes,
    pub remote_handoff: RemoteHandoffManager,
}

pub struct BoundBridges {
    pub clicky_addr: SocketAddr,
    pub bridge_addr: SocketAddr,
    pub clicky_handle: tokio::task::JoinHandle<()>,
    pub bridge_handle: tokio::task::JoinHandle<()>,
}

async fn cors_middleware(req: Request, next: Next) -> Response {
    if req.method() == Method::OPTIONS {
        let mut resp = Response::new(Body::empty());
        *resp.status_mut() = StatusCode::NO_CONTENT;
        attach_cors_headers(&mut resp);
        return resp;
    }
    let mut resp = next.run(req).await;
    attach_cors_headers(&mut resp);
    resp
}

fn attach_cors_headers(resp: &mut Response) {
    let headers = resp.headers_mut();
    headers.insert(ACCESS_CONTROL_ALLOW_ORIGIN, HeaderValue::from_static("*"));
    headers.insert(
        ACCESS_CONTROL_ALLOW_METHODS,
        HeaderValue::from_static("GET, POST, DELETE, OPTIONS"),
    );
    headers.insert(
        ACCESS_CONTROL_ALLOW_HEADERS,
        HeaderValue::from_static("Content-Type, Authorization, x-llm-provider"),
    );
}

/// Starts both the companion `/chat` compatibility server (`clicky_port`, default 8787)
/// and the Carnival UI/telemetry bridge (`bridge_port`, default 32123).
pub async fn spawn_bridge_servers(
    state: AppState,
    clicky_port: u16,
    bridge_port: u16,
) -> std::io::Result<BoundBridges> {
    let clicky_router = Router::new()
        .route("/chat", post(handle_clicky_chat))
        .route("/health", get(handle_health))
        .layer(middleware::from_fn(cors_middleware))
        .with_state(state.clone());

    let bridge_router = Router::new()
        .route("/health", get(handle_health))
        .route("/telemetry", get(handle_telemetry))
        .route("/optimizations", post(handle_set_optimizations))
        .route("/turn", post(handle_unified_turn))
        .route("/cua/action", post(handle_cua_action))
        .route("/search", get(handle_search))
        .route("/privacy/grant", post(handle_issue_privacy_grant))
        .route("/overlay/state", get(handle_get_overlay_state))
        .route("/overlay/command", post(handle_post_overlay_command))
        .route("/sessions", get(handle_list_sessions))
        .route("/sessions/:id/events", get(handle_session_events))
        .route("/sessions/:id/fork", post(handle_fork_session))
        .route("/memory", get(handle_list_memory).post(handle_upsert_memory))
        .route("/cordis/discover", get(handle_cordis_discover))
        .route("/cordis/shutdown", post(handle_cordis_shutdown))
        .route("/watchdog/test-rollback", post(handle_watchdog_test_rollback))
        .route("/window/spotlight", post(handle_window_spotlight))
        .route("/window/overlay", post(handle_window_overlay))
        .route("/window/main", post(handle_window_main))
        .route("/ui/glass", post(handle_ui_glass))
        .route("/favicon.ico", get(handle_favicon_no_content))
        // Agent Manager UI & multi-agent runtime
        .route("/agent-manager", get(handle_agent_manager_html))
        .route("/agents/presets", get(handle_agent_presets))
        .route("/agents", get(handle_list_agents).post(handle_spawn_agent))
        .route("/agents/demo-crew", post(handle_spawn_demo_crew))
        .route("/agents/broadcast", post(handle_broadcast_agents))
        .route("/agents/events", get(handle_agent_events))
        .route("/agents/:id", get(handle_get_agent).delete(handle_delete_agent))
        .route("/agents/:id/input", post(handle_agent_input))
        .route("/agents/:id/approve", post(handle_agent_approve))
        .route("/agents/:id/view-mode", post(handle_agent_view_mode))
        .route("/agents/:id/resize", post(handle_agent_resize))
        .route("/agents/:id/stop", post(handle_agent_stop))
        .route("/agents/:id/diff", get(handle_agent_diff))
        // Disk-backed Studio Layouts, Mods & Shape UI
        .route("/studio/recipes", get(handle_list_studio_recipes).post(handle_save_studio_recipe))
        .route("/studio/active", post(handle_set_studio_active))
        .route("/studio/shape", post(handle_shape_studio_ui))
        .route("/studio/rollback", post(handle_rollback_studio_ui))
        // Remote Handoff via SSH / Tailscale Peer Compute
        .route("/remote/peers", get(handle_remote_state).post(handle_add_remote_peer))
        .route("/remote/peers/:id", delete(handle_delete_remote_peer))
        .route("/remote/peers/:id/probe", post(handle_probe_remote_peer))
        .route("/remote/handoff", post(handle_dispatch_remote_handoff))
        .route("/remote/handoff/:handoff_id/receive", post(handle_receive_remote_handoff))
        .layer(middleware::from_fn(cors_middleware))
        .with_state(state);

    let clicky_listener =
        TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], clicky_port))).await?;
    let clicky_addr = clicky_listener.local_addr()?;

    let bridge_listener =
        TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], bridge_port))).await?;
    let bridge_addr = bridge_listener.local_addr()?;

    let clicky_handle = tokio::spawn(async move {
        let _ = axum::serve(clicky_listener, clicky_router).await;
    });
    let bridge_handle = tokio::spawn(async move {
        let _ = axum::serve(bridge_listener, bridge_router).await;
    });

    Ok(BoundBridges {
        clicky_addr,
        bridge_addr,
        clicky_handle,
        bridge_handle,
    })
}

#[derive(Debug, Deserialize)]
struct ClickyChatRequest {
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default)]
    messages: Vec<ClickyMessage>,
    #[serde(default)]
    prompt: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ClickyMessage {
    role: String,
    content: Value,
}

fn extract_prompt_from_clicky(req: &ClickyChatRequest) -> String {
    if let Some(p) = &req.prompt {
        if !p.trim().is_empty() {
            return p.clone();
        }
    }
    for msg in req.messages.iter().rev() {
        if msg.role == "user" {
            if let Some(s) = msg.content.as_str() {
                return s.to_string();
            }
            if let Some(arr) = msg.content.as_array() {
                let texts: Vec<&str> = arr
                    .iter()
                    .filter_map(|part| part.get("text").and_then(|t| t.as_str()))
                    .collect();
                if !texts.is_empty() {
                    return texts.join(" ");
                }
            }
        }
    }
    "Help me with what is on my screen.".to_string()
}

/// Companion-compatible `POST /chat` endpoint emitting Anthropic-style SSE
/// (`content_block_delta` + `message_stop`) backed by SQLite WAL + SkillTree + CordisBus.
async fn handle_clicky_chat(
    State(state): State<AppState>,
    Json(payload): Json<ClickyChatRequest>,
) -> Response {
    let prompt = extract_prompt_from_clicky(&payload);
    let session_id = payload
        .session_id
        .unwrap_or_else(|| "companion-live".to_string());

    let turn_req = TurnRequest {
        request_id: Uuid::new_v4().to_string(),
        session_id,
        lane: LaneKind::ClickyAmbient,
        prompt,
        screen_context: Some(ScreenContext::default()),
        optimizations: None,
    };

    match execute_unified_turn(&state, turn_req).await {
        Ok(res) => {
            let chunk1 = json!({
                "type": "content_block_delta",
                "index": 0,
                "delta": { "type": "text_delta", "text": res.text }
            });
            let stop = json!({ "type": "message_stop" });
            let sse_body = format!(
                "event: content_block_delta\ndata: {chunk1}\n\nevent: message_stop\ndata: {stop}\n\n"
            );

            Response::builder()
                .status(StatusCode::OK)
                .header(CONTENT_TYPE, "text/event-stream")
                .header(CACHE_CONTROL, "no-cache")
                .body(Body::from(sse_body))
                .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
        }
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": err })),
        )
            .into_response(),
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct UnifiedTurnResponse {
    request_id: String,
    session_id: String,
    text: String,
    spatial_command: Option<Value>,
    prefix_hash: String,
    cache_hit: bool,
    domain_router: String,
    loaded_skills: usize,
    total_skills: usize,
    latency_ms: u64,
}

async fn execute_unified_turn(
    state: &AppState,
    req: TurnRequest,
) -> Result<UnifiedTurnResponse, String> {
    let start = Instant::now();
    state
        .store
        .ensure_session(&req.session_id, "Unified Session")
        .map_err(|e| e.to_string())?;

    state
        .store
        .append_event(
            &req.session_id,
            HarnessEventKind::TextDelta,
            json!({ "role": "user", "prompt": req.prompt, "lane": req.lane }),
        )
        .map_err(|e| e.to_string())?;

    let prepared = state.optimizer.prepare_turn(&req);

    let rpc_res = state
        .cordis
        .request(
            "route_turn",
            json!({
                "request_id": req.request_id,
                "session_id": req.session_id,
                "lane": req.lane,
                "prompt": req.prompt,
                "screen_context": req.screen_context,
                "prefix_hash": prepared.prefix_hash,
                "domain_router": prepared.route.domain_router,
            }),
        )
        .await
        .map_err(|e| e.to_string())?;

    let text = rpc_res
        .get("text")
        .and_then(|v| v.as_str())
        .unwrap_or("Unified harness completed turn.")
        .to_string();
    let spatial = rpc_res
        .get("spatial_command")
        .filter(|v| !v.is_null())
        .cloned();

    let latency_ms = start.elapsed().as_millis() as u64;
    state.optimizer.record_latency(latency_ms);

    if let Some(sp) = &spatial {
        let _ = state.store.append_event(
            &req.session_id,
            HarnessEventKind::SpatialCommand,
            sp.clone(),
        );
        if let Ok(parsed_cmd) = serde_json::from_value::<SpatialCommand>(sp.clone()) {
            if let Ok(mut overlay) = state.overlay_state.write() {
                overlay.commands.push(parsed_cmd);
                overlay.speech_text = Some(text.clone());
                overlay.visible = true;
                overlay.updated_at_ms = now_ms();
            }
            if let Some(tx) = &state.window_tx {
                let _ = tx.send(WindowCommand::SetOverlayVisible(true));
            }
        }
    }

    let _ = state.store.append_event(
        &req.session_id,
        HarnessEventKind::TurnComplete,
        json!({
            "request_id": req.request_id,
            "text": text,
            "prefix_hash": prepared.prefix_hash,
            "cache_hit": prepared.cache_hit,
            "latency_ms": latency_ms,
        }),
    );

    Ok(UnifiedTurnResponse {
        request_id: req.request_id,
        session_id: req.session_id,
        text,
        spatial_command: spatial,
        prefix_hash: prepared.prefix_hash,
        cache_hit: prepared.cache_hit,
        domain_router: prepared.route.domain_router,
        loaded_skills: prepared.route.loaded_skills.len(),
        total_skills: prepared.route.total_catalog_skills,
        latency_ms,
    })
}

async fn handle_unified_turn(
    State(state): State<AppState>,
    Json(req): Json<TurnRequest>,
) -> Response {
    match execute_unified_turn(&state, req).await {
        Ok(res) => (StatusCode::OK, Json(res)).into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": err })),
        )
            .into_response(),
    }
}

async fn handle_health(State(state): State<AppState>) -> impl IntoResponse {
    let seams = state.registry.list().unwrap_or_default();
    let cordis = state.cordis.status().await;
    let (glass_opacity, glass_tint) = state
        .overlay_state
        .read()
        .map(|g| (g.glass_opacity, g.glass_tint.clone()))
        .unwrap_or((0.80, "obsidian".to_string()));
    let active_agents = state.agent_manager.list().len();
    Json(json!({
        "ok": true,
        "timestamp_ms": now_ms(),
        "seams_mounted": seams.len(),
        "openbitfun_present": state.openbitfun.is_present(),
        "indexed_files": state.openbitfun.index.file_count(),
        "active_agents": active_agents,
        "glass_opacity": glass_opacity,
        "glass_tint": glass_tint,
        "cordis": cordis,
    }))
}

async fn handle_telemetry(State(state): State<AppState>) -> impl IntoResponse {
    let cordis = state.cordis.status().await;
    let (hits, misses, hit_rate, last_loaded, last_tier, last_latency) =
        state.optimizer.metrics();
    let (active_sessions, total_events_logged) = state.store.stats().unwrap_or((0, 0));

    let snapshot = TelemetrySnapshot {
        cordis_state: cordis.state,
        cordis_pid: cordis.pid,
        cordis_request_count: cordis.request_count,
        cordis_restart_count: cordis.restart_count,
        prefix_cache_hits: hits,
        prefix_cache_misses: misses,
        prefix_cache_hit_rate: hit_rate,
        total_skills: state.optimizer.total_skills(),
        last_loaded_skills: last_loaded,
        active_sessions,
        total_events_logged,
        last_model_tier: last_tier,
        last_latency_ms: last_latency,
        optimizations: state.optimizer.flags(),
    };
    Json(snapshot)
}

async fn handle_set_optimizations(
    State(state): State<AppState>,
    Json(flags): Json<OptimizationFlags>,
) -> impl IntoResponse {
    state.optimizer.set_flags(flags.clone());
    Json(json!({ "ok": true, "optimizations": flags }))
}

async fn handle_cua_action(
    State(state): State<AppState>,
    Json(req): Json<CuaActionRequest>,
) -> Response {
    let decision = validate_cua_action(&state.cua_envelope, &req);
    let allowed = decision.is_ok();
    let reason = decision.as_ref().err().map(|e| e.to_string());

    let _ = state.store.append_event(
        "cua-audit",
        HarnessEventKind::CuaDecision,
        json!({
            "action_id": req.action_id,
            "target_app": req.target_app,
            "action": req.action,
            "allowed": allowed,
            "reason": reason,
        }),
    );

    match decision {
        Ok(()) => (
            StatusCode::OK,
            Json(json!({
                "ok": true,
                "action_id": req.action_id,
                "target_app": req.target_app,
                "executed_via": "enigo_capability_gate",
            })),
        )
            .into_response(),
        Err(err) => (
            StatusCode::FORBIDDEN,
            Json(json!({
                "ok": false,
                "error": err.to_string(),
            })),
        )
            .into_response(),
    }
}

#[derive(Debug, Deserialize)]
struct SearchParams {
    q: String,
    #[serde(default = "default_max_results")]
    max: usize,
}

fn default_max_results() -> usize {
    10
}

async fn handle_search(
    State(state): State<AppState>,
    Query(params): Query<SearchParams>,
) -> impl IntoResponse {
    let start = Instant::now();
    let matches = state.openbitfun.search(&params.q, params.max);
    Json(json!({
        "query": params.q,
        "elapsed_us": start.elapsed().as_micros(),
        "matches": matches,
    }))
}

#[derive(Debug, Deserialize)]
struct PrivacyGrantRequest {
    scope: PrivacyScope,
    #[serde(default = "default_ttl_ms")]
    ttl_ms: i64,
}

fn default_ttl_ms() -> i64 {
    5000
}

async fn handle_issue_privacy_grant(
    State(state): State<AppState>,
    Json(req): Json<PrivacyGrantRequest>,
) -> impl IntoResponse {
    let token = state
        .privacy_guard
        .issue_explicit_grant(req.scope, req.ttl_ms);
    Json(json!({ "token": token, "scope": req.scope, "ttl_ms": req.ttl_ms }))
}

async fn handle_get_overlay_state(State(state): State<AppState>) -> impl IntoResponse {
    let snapshot = state
        .overlay_state
        .read()
        .map(|g| g.clone())
        .unwrap_or_default();
    Json(snapshot)
}

#[derive(Debug, Deserialize)]
struct OverlayCommandRequest {
    #[serde(default)]
    command: Option<SpatialCommand>,
    #[serde(default)]
    speech_text: Option<String>,
    #[serde(default)]
    clear: Option<bool>,
    #[serde(default)]
    visible: Option<bool>,
    #[serde(default)]
    click_through: Option<bool>,
}

async fn handle_post_overlay_command(
    State(state): State<AppState>,
    Json(req): Json<OverlayCommandRequest>,
) -> impl IntoResponse {
    let snapshot = if let Ok(mut overlay) = state.overlay_state.write() {
        if req.clear.unwrap_or(false) {
            overlay.commands.clear();
            overlay.speech_text = None;
        }
        if let Some(cmd) = req.command {
            if matches!(cmd, SpatialCommand::Clear) {
                overlay.commands.clear();
            } else {
                overlay.commands.push(cmd);
            }
        }
        if let Some(text) = req.speech_text {
            overlay.speech_text = Some(text);
        }
        if let Some(vis) = req.visible {
            overlay.visible = vis;
        }
        if let Some(ct) = req.click_through {
            overlay.click_through = ct;
        }
        overlay.updated_at_ms = now_ms();
        overlay.clone()
    } else {
        OverlayState::default()
    };
    Json(snapshot)
}

async fn handle_list_sessions(State(state): State<AppState>) -> Response {
    match state.store.list_sessions() {
        Ok(sessions) => (StatusCode::OK, Json(json!({ "sessions": sessions }))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}


async fn handle_session_events(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Response {
    match state.store.resume_session(&id) {
        Ok((session, events)) => (
            StatusCode::OK,
            Json(json!({ "session": session, "events": events })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

#[derive(Debug, Deserialize)]
struct ForkSessionRequest {
    up_to_seq: i64,
    #[serde(default)]
    new_session_id: Option<String>,
    #[serde(default)]
    new_title: Option<String>,
}

async fn handle_fork_session(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<ForkSessionRequest>,
) -> Response {
    let new_id = req
        .new_session_id
        .unwrap_or_else(|| format!("{id}-fork-{}", &Uuid::new_v4().to_string()[..6]));
    match state
        .store
        .fork_session(&id, req.up_to_seq, &new_id, req.new_title.as_deref())
    {
        Ok(info) => (StatusCode::OK, Json(info)).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn handle_list_memory(State(state): State<AppState>) -> Response {
    match state.store.list_memory() {
        Ok(entries) => (StatusCode::OK, Json(json!({ "entries": entries }))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}


#[derive(Debug, Deserialize)]
struct UpsertMemoryRequest {
    key: String,
    value: Value,
    #[serde(default)]
    pinned: bool,
}

async fn handle_upsert_memory(
    State(state): State<AppState>,
    Json(req): Json<UpsertMemoryRequest>,
) -> Response {
    match state.store.upsert_memory(&req.key, req.value, req.pinned) {
        Ok(()) => (StatusCode::OK, Json(json!({ "ok": true, "key": req.key }))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}


async fn handle_cordis_discover(State(state): State<AppState>) -> Response {
    match state.cordis.request("discover_harnesses", json!({})).await {
        Ok(res) => (StatusCode::OK, Json(res)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}


async fn handle_cordis_shutdown(State(state): State<AppState>) -> Response {
    let _ = state.cordis.shutdown().await;
    let status = state.cordis.status().await;
    (StatusCode::OK, Json(json!({ "ok": true, "cordis": status }))).into_response()
}

struct DrillSeam {
    version: &'static str,
    unmounted: Arc<AtomicBool>,
}

impl CapabilitySeam for DrillSeam {
    fn id(&self) -> &str {
        "layer1.opt.drill"
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

/// Runs a real isolated Layer-0 Watchdog hot-swap drill with a simulated broken
/// Layer-1 plugin, returning the actual `<1s` RollbackReport.
async fn handle_watchdog_test_rollback(State(state): State<AppState>) -> Response {
    let sim_registry = SeamRegistry::new("watchdog-drill");
    let sim_guard = ImmutableCoreGuard::for_workspace(&PathBuf::from("/tmp/carnival-drill"));
    let watchdog = Watchdog::new(sim_registry.clone(), state.store.clone(), sim_guard);

    let v1_unmounted = Arc::new(AtomicBool::new(false));
    let v1_factory: SeamFactory = {
        let flag = Arc::clone(&v1_unmounted);
        Arc::new(move || {
            Arc::new(DrillSeam {
                version: "1.0.0-stable",
                unmounted: Arc::clone(&flag),
            })
        })
    };
    if let Err(e) = sim_registry.mount(v1_factory()) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response();
    }

    let v2_unmounted = Arc::new(AtomicBool::new(false));
    let v2_factory: SeamFactory = {
        let flag = Arc::clone(&v2_unmounted);
        Arc::new(move || {
            Arc::new(DrillSeam {
                version: "2.0.0-broken",
                unmounted: Arc::clone(&flag),
            })
        })
    };

    let envelope = CapabilityEnvelope::restricted("layer1.opt.drill", vec![]);
    match watchdog.hot_swap_seam(
        &[PathBuf::from("crates/layer1-router/src/lib.rs")],
        &envelope,
        Some(v1_factory),
        v2_factory,
        |_| Err("Simulated IPC crash during 30s probation health gate".to_string()),
    ) {
        Ok(HotSwapOutcome::RolledBack(report)) => (StatusCode::OK, Json(report)).into_response(),
        Ok(HotSwapOutcome::Committed { .. }) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "expected rollback" })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

#[derive(Debug, Deserialize)]
struct WindowActionRequest {
    action: String,
    #[serde(default)]
    click_through: Option<bool>,
}

async fn handle_window_spotlight(
    State(state): State<AppState>,
    Json(req): Json<WindowActionRequest>,
) -> impl IntoResponse {
    let cmd = match req.action.as_str() {
        "show" => WindowCommand::ShowSpotlight,
        "hide" => WindowCommand::HideSpotlight,
        _ => WindowCommand::ToggleSpotlight,
    };
    if let Some(tx) = &state.window_tx {
        let _ = tx.send(cmd);
    }
    Json(json!({ "ok": true, "window": "spotlight", "action": req.action }))
}

async fn handle_window_overlay(
    State(state): State<AppState>,
    Json(req): Json<WindowActionRequest>,
) -> impl IntoResponse {
    if let Some(ct) = req.click_through {
        if let Ok(mut overlay) = state.overlay_state.write() {
            overlay.click_through = ct;
            overlay.updated_at_ms = now_ms();
        }
        if let Some(tx) = &state.window_tx {
            let _ = tx.send(WindowCommand::SetOverlayClickThrough(ct));
        }
    }

    let next_visible = if let Ok(mut overlay) = state.overlay_state.write() {
        let v = match req.action.as_str() {
            "show" => true,
            "hide" => false,
            _ => !overlay.visible,
        };
        overlay.visible = v;
        overlay.updated_at_ms = now_ms();
        v
    } else {
        false
    };

    if let Some(tx) = &state.window_tx {
        let cmd = match req.action.as_str() {
            "show" => WindowCommand::SetOverlayVisible(true),
            "hide" => WindowCommand::SetOverlayVisible(false),
            _ => WindowCommand::ToggleOverlay,
        };
        let _ = tx.send(cmd);
    }

    Json(json!({ "ok": true, "window": "clicky-overlay", "visible": next_visible }))
}

async fn handle_window_main(
    State(state): State<AppState>,
    Json(req): Json<WindowActionRequest>,
) -> impl IntoResponse {
    if let Some(tx) = &state.window_tx {
        let _ = tx.send(WindowCommand::FocusMain);
    }
    Json(json!({ "ok": true, "window": "main", "action": req.action }))
}

#[derive(Debug, Deserialize)]
struct UiGlassRequest {
    #[serde(default)]
    opacity: Option<f32>,
    #[serde(default)]
    tint: Option<String>,
}

async fn handle_favicon_no_content() -> StatusCode {
    StatusCode::NO_CONTENT
}

async fn handle_ui_glass(
    State(state): State<AppState>,
    Json(req): Json<UiGlassRequest>,
) -> impl IntoResponse {
    let mut out_opacity = 0.80_f32;
    let mut out_tint = "obsidian".to_string();
    if let Ok(mut overlay) = state.overlay_state.write() {
        if let Some(op) = req.opacity {
            overlay.glass_opacity = clamp_glass_opacity(op);
        }
        if let Some(tint) = req.tint {
            let trimmed = tint.trim().to_lowercase();
            if !trimmed.is_empty() {
                overlay.glass_tint = normalize_glass_tint(&trimmed);
            }
        }
        overlay.updated_at_ms = now_ms();
        out_opacity = overlay.glass_opacity;
        out_tint = overlay.glass_tint.clone();
    }
    save_ui_prefs_to(
        &default_ui_prefs_path(),
        &UiPrefs {
            glass_opacity: out_opacity,
            glass_tint: out_tint.clone(),
        },
    );
    Json(json!({
        "ok": true,
        "glass_opacity": out_opacity,
        "glass_tint": out_tint
    }))
}

async fn handle_agent_manager_html() -> Html<&'static str> {
    Html(AGENT_MANAGER_HTML)
}

async fn handle_agent_presets(State(state): State<AppState>) -> impl IntoResponse {
    Json(json!({ "presets": state.agent_manager.presets() }))
}

async fn handle_list_agents(State(state): State<AppState>) -> impl IntoResponse {
    let agents = state.agent_manager.list();
    let needs_you = agents
        .iter()
        .filter(|a| matches!(a.status, crate::agent_manager::AgentStatus::NeedsYou))
        .count();
    Json(json!({ "agents": agents, "needs_you": needs_you }))
}

async fn handle_spawn_agent(
    State(state): State<AppState>,
    Json(req): Json<SpawnAgentRequest>,
) -> Response {
    match state.agent_manager.spawn(req) {
        Ok(detail) => (StatusCode::OK, Json(json!({ "ok": true, "agent": detail }))).into_response(),
        Err(err) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "ok": false, "error": err })),
        )
            .into_response(),
    }
}

async fn handle_spawn_demo_crew(State(state): State<AppState>) -> Response {
    match state.agent_manager.spawn_demo_crew() {
        Ok(agents) => (StatusCode::OK, Json(json!({ "ok": true, "agents": agents }))).into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "ok": false, "error": err })),
        )
            .into_response(),
    }
}

#[derive(Debug, Deserialize)]
struct BroadcastRequest {
    #[serde(default)]
    ids: Vec<String>,
    prompt: String,
}

async fn handle_broadcast_agents(
    State(state): State<AppState>,
    Json(req): Json<BroadcastRequest>,
) -> impl IntoResponse {
    let results = state.agent_manager.broadcast(req.ids, &req.prompt);
    let ok_count = results.iter().filter(|r| r.is_ok()).count();
    Json(json!({ "ok": true, "delivered": ok_count }))
}

async fn handle_agent_events(
    State(state): State<AppState>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, std::convert::Infallible>>> {
    let rx = state.agent_manager.subscribe_events();
    let stream = BroadcastStream::new(rx).filter_map(|msg| match msg {
        Ok(v) => Some(Ok(Event::default().data(v.to_string()))),
        Err(_) => None,
    });
    Sse::new(stream).keep_alive(KeepAlive::default())
}

async fn handle_get_agent(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Response {
    match state.agent_manager.get(&id) {
        Some(detail) => (StatusCode::OK, Json(json!({ "ok": true, "agent": detail }))).into_response(),
        None => (StatusCode::NOT_FOUND, Json(json!({ "ok": false, "error": "agent not found" }))).into_response(),
    }
}

async fn handle_delete_agent(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Response {
    match state.agent_manager.remove(&id) {
        Ok(()) => (StatusCode::OK, Json(json!({ "ok": true, "id": id }))).into_response(),
        Err(err) => (StatusCode::BAD_REQUEST, Json(json!({ "ok": false, "error": err }))).into_response(),
    }
}

#[derive(Debug, Deserialize)]
struct AgentInputRequest {
    text: String,
    #[serde(default = "default_true_flag")]
    newline: bool,
}

fn default_true_flag() -> bool {
    true
}

async fn handle_agent_input(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<AgentInputRequest>,
) -> Response {
    match state.agent_manager.send_input(&id, &req.text, req.newline) {
        Ok(detail) => (StatusCode::OK, Json(json!({ "ok": true, "agent": detail }))).into_response(),
        Err(err) => (StatusCode::BAD_REQUEST, Json(json!({ "ok": false, "error": err }))).into_response(),
    }
}

#[derive(Debug, Deserialize)]
struct AgentApproveRequest {
    allow: bool,
}

async fn handle_agent_approve(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<AgentApproveRequest>,
) -> Response {
    match state.agent_manager.approve(&id, req.allow) {
        Ok(detail) => (StatusCode::OK, Json(json!({ "ok": true, "agent": detail }))).into_response(),
        Err(err) => (StatusCode::BAD_REQUEST, Json(json!({ "ok": false, "error": err }))).into_response(),
    }
}

#[derive(Debug, Deserialize)]
struct AgentViewModeRequest {
    view_mode: AgentViewMode,
}

async fn handle_agent_view_mode(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<AgentViewModeRequest>,
) -> Response {
    match state.agent_manager.set_view_mode(&id, req.view_mode) {
        Ok(detail) => (StatusCode::OK, Json(json!({ "ok": true, "agent": detail }))).into_response(),
        Err(err) => (StatusCode::BAD_REQUEST, Json(json!({ "ok": false, "error": err }))).into_response(),
    }
}

#[derive(Debug, Deserialize)]
struct AgentResizeRequest {
    cols: u16,
    rows: u16,
}

async fn handle_agent_resize(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<AgentResizeRequest>,
) -> Response {
    match state.agent_manager.resize(&id, req.cols, req.rows) {
        Ok(()) => (StatusCode::OK, Json(json!({ "ok": true }))).into_response(),
        Err(err) => (StatusCode::BAD_REQUEST, Json(json!({ "ok": false, "error": err }))).into_response(),
    }
}

async fn handle_agent_stop(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Response {
    match state.agent_manager.stop(&id) {
        Ok(detail) => (StatusCode::OK, Json(json!({ "ok": true, "agent": detail }))).into_response(),
        Err(err) => (StatusCode::BAD_REQUEST, Json(json!({ "ok": false, "error": err }))).into_response(),
    }
}

async fn handle_agent_diff(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Response {
    match state.agent_manager.git_diff(&id) {
        Ok(diff) => (StatusCode::OK, Json(json!({ "ok": true, "diff": diff }))).into_response(),
        Err(err) => (StatusCode::BAD_REQUEST, Json(json!({ "ok": false, "error": err }))).into_response(),
    }
}

async fn handle_list_studio_recipes(State(state): State<AppState>) -> impl IntoResponse {
    Json(state.studio_recipes.list_state())
}

async fn handle_save_studio_recipe(
    State(state): State<AppState>,
    Json(req): Json<SaveRecipeRequest>,
) -> Response {
    match state.studio_recipes.save_recipe(req) {
        Ok((recipe, st)) => (StatusCode::OK, Json(json!({ "ok": true, "recipe": recipe, "studio": st }))).into_response(),
        Err(err) => (StatusCode::BAD_REQUEST, Json(json!({ "ok": false, "error": err }))).into_response(),
    }
}

#[derive(Debug, Deserialize)]
struct StudioActiveRequest {
    #[serde(default)]
    active_layout: Option<String>,
    #[serde(default)]
    enabled_mods: Option<Vec<String>>,
}

async fn handle_set_studio_active(
    State(state): State<AppState>,
    Json(req): Json<StudioActiveRequest>,
) -> Response {
    match state.studio_recipes.set_active(req.active_layout, req.enabled_mods) {
        Ok(st) => (StatusCode::OK, Json(json!({ "ok": true, "studio": st }))).into_response(),
        Err(err) => (StatusCode::BAD_REQUEST, Json(json!({ "ok": false, "error": err }))).into_response(),
    }
}

#[derive(Debug, Deserialize)]
struct StudioShapeRequest {
    prompt: String,
}

async fn handle_shape_studio_ui(
    State(state): State<AppState>,
    Json(req): Json<StudioShapeRequest>,
) -> Response {
    match state.studio_recipes.shape_ui(&req.prompt) {
        Ok((recipe, snapshot, st)) => (
            StatusCode::OK,
            Json(json!({
                "ok": true,
                "recipe": recipe,
                "snapshot": snapshot,
                "studio": st,
            })),
        )
            .into_response(),
        Err(err) => (StatusCode::BAD_REQUEST, Json(json!({ "ok": false, "error": err }))).into_response(),
    }
}

async fn handle_rollback_studio_ui(State(state): State<AppState>) -> Response {
    match state.studio_recipes.rollback() {
        Ok((snapshot, st)) => (
            StatusCode::OK,
            Json(json!({ "ok": true, "snapshot": snapshot, "studio": st })),
        )
            .into_response(),
        Err(err) => (StatusCode::BAD_REQUEST, Json(json!({ "ok": false, "error": err }))).into_response(),
    }
}

async fn handle_remote_state(State(state): State<AppState>) -> impl IntoResponse {
    Json(state.remote_handoff.state())
}

async fn handle_add_remote_peer(
    State(state): State<AppState>,
    Json(req): Json<AddPeerRequest>,
) -> Response {
    match state.remote_handoff.add_peer(req) {
        Ok(peer) => (StatusCode::OK, Json(json!({ "ok": true, "peer": peer }))).into_response(),
        Err(err) => (StatusCode::BAD_REQUEST, Json(json!({ "ok": false, "error": err }))).into_response(),
    }
}

async fn handle_delete_remote_peer(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Response {
    match state.remote_handoff.remove_peer(&id) {
        Ok(()) => (StatusCode::OK, Json(json!({ "ok": true, "id": id }))).into_response(),
        Err(err) => (StatusCode::BAD_REQUEST, Json(json!({ "ok": false, "error": err }))).into_response(),
    }
}

async fn handle_probe_remote_peer(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Response {
    match state.remote_handoff.probe_peer(&id) {
        Ok(peer) => (StatusCode::OK, Json(json!({ "ok": true, "peer": peer }))).into_response(),
        Err(err) => (StatusCode::BAD_REQUEST, Json(json!({ "ok": false, "error": err }))).into_response(),
    }
}

async fn handle_dispatch_remote_handoff(
    State(state): State<AppState>,
    Json(req): Json<DispatchHandoffRequest>,
) -> Response {
    match state.remote_handoff.dispatch_handoff(&state.agent_manager, req) {
        Ok(handoff) => (StatusCode::OK, Json(json!({ "ok": true, "handoff": handoff }))).into_response(),
        Err(err) => (StatusCode::BAD_REQUEST, Json(json!({ "ok": false, "error": err }))).into_response(),
    }
}

async fn handle_receive_remote_handoff(
    State(state): State<AppState>,
    Path(handoff_id): Path<String>,
    req: Option<Json<ReceiveHandoffRequest>>,
) -> Response {
    let body = req.map(|j| j.0).unwrap_or(ReceiveHandoffRequest { apply_locally: false });
    match state.remote_handoff.receive_handoff(&handoff_id, body) {
        Ok(handoff) => (StatusCode::OK, Json(json!({ "ok": true, "handoff": handoff }))).into_response(),
        Err(err) => (StatusCode::BAD_REQUEST, Json(json!({ "ok": false, "error": err }))).into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_prefs_persist_and_normalize_cleanly() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ui_prefs.json");

        assert_eq!(load_ui_prefs_from(&path), UiPrefs::default());

        save_ui_prefs_to(
            &path,
            &UiPrefs {
                glass_opacity: 1.5,
                glass_tint: "Warm-Smoke".into(),
            },
        );
        let loaded = load_ui_prefs_from(&path);
        assert!((loaded.glass_opacity - 0.96).abs() < 0.001);
        assert_eq!(loaded.glass_tint, "warm-smoke");

        save_ui_prefs_to(
            &path,
            &UiPrefs {
                glass_opacity: 0.01,
                glass_tint: "unknown-rainbow".into(),
            },
        );
        let clamped = load_ui_prefs_from(&path);
        assert!((clamped.glass_opacity - 0.08).abs() < 0.001);
        assert_eq!(clamped.glass_tint, "obsidian");
    }
}


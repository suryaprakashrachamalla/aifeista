use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Identifies one of the three integrated upstream harnesses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HarnessId {
    OpenBitFun,
    DeepSeekHarness,
    Clicky,
}

/// Identifies which Layer-2 experience lane originated or consumes a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LaneKind {
    ClickyAmbient,
    BuilderWorkspace,
    OptimizationDeck,
}

/// Lifecycle state of the lazy Cordis Node.js integration bus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CordisState {
    Stopped,
    Starting,
    Running,
    Degraded,
}

/// Model routing tier selected by Layer-1 TieredModelRouter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelTier {
    LocalTriage,
    DeepSeekV4,
    ByokCloud,
}

/// One-click toggles surfaced in the Optimization Control Deck.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptimizationFlags {
    pub prefix_cache_lock: bool,
    pub skill_tree_pruning: bool,
    pub ptc_batching: bool,
    pub local_triage: bool,
}

impl Default for OptimizationFlags {
    fn default() -> Self {
        Self {
            prefix_cache_lock: true,
            skill_tree_pruning: true,
            ptc_batching: true,
            local_triage: true,
        }
    }
}

/// Fail-closed capability envelope enforced by Layer 0.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityEnvelope {
    pub subject: String,
    pub fs_read: Vec<String>,
    pub fs_write: Vec<String>,
    pub network_allow: bool,
    pub cua_apps: Vec<String>,
    pub allow_vision: bool,
    pub allow_audio: bool,
}

impl CapabilityEnvelope {
    /// Minimal read-only workspace envelope with no network or CUA access.
    pub fn restricted(subject: impl Into<String>, workspace_read: Vec<String>) -> Self {
        Self {
            subject: subject.into(),
            fs_read: workspace_read,
            fs_write: Vec::new(),
            network_allow: false,
            cua_apps: Vec::new(),
            allow_vision: false,
            allow_audio: false,
        }
    }
}

/// Spatial overlay command emitted for Hey Clicky / Layer 2 visual lane.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SpatialCommand {
    Point {
        x: i32,
        y: i32,
        label: String,
        #[serde(default)]
        screen: Option<u32>,
    },
    Draw {
        shape: DrawShape,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        #[serde(default)]
        caption: Option<String>,
    },
    Highlight {
        ax_token: String,
    },
    Clear,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DrawShape {
    Rect,
    Arrow,
    Circle,
    Freehand,
}

/// On-demand desktop/screen context attached only when explicitly triggered.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScreenContext {
    #[serde(default)]
    pub active_app: Option<String>,
    #[serde(default)]
    pub window_title: Option<String>,
    #[serde(default)]
    pub screenshot_ref: Option<String>,
    #[serde(default)]
    pub ax_tree_digest: Option<String>,
    #[serde(default)]
    pub cursor_x: Option<i32>,
    #[serde(default)]
    pub cursor_y: Option<i32>,
}

/// Unified request payload entering the harness from any lane or orchestrator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnRequest {
    pub request_id: String,
    pub session_id: String,
    pub lane: LaneKind,
    pub prompt: String,
    #[serde(default)]
    pub screen_context: Option<ScreenContext>,
    #[serde(default)]
    pub optimizations: Option<OptimizationFlags>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CuaActionKind {
    Click,
    DoubleClick,
    TypeText,
    KeyCombo,
    Scroll,
}

/// Permission-controlled desktop action routed through ClickyX Bridge.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CuaActionRequest {
    pub action_id: String,
    pub target_app: String,
    pub action: CuaActionKind,
    pub params: Value,
    #[serde(default)]
    pub background: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HarnessEventKind {
    TextDelta,
    TtsChunk,
    SpatialCommand,
    CuaDecision,
    ToolStatus,
    Telemetry,
    CordisLifecycle,
    TurnComplete,
    Error,
}

/// Append-only event emitted by the harness and persisted in SQLite WAL.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HarnessEvent {
    pub seq: i64,
    pub session_id: String,
    pub timestamp_ms: i64,
    pub kind: HarnessEventKind,
    pub payload: Value,
}

/// Live telemetry displayed in the Optimization Control Deck.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TelemetrySnapshot {
    pub cordis_state: CordisState,
    pub cordis_pid: Option<u32>,
    pub cordis_request_count: u64,
    pub cordis_restart_count: u64,
    pub prefix_cache_hits: u64,
    pub prefix_cache_misses: u64,
    pub prefix_cache_hit_rate: f64,
    pub total_skills: usize,
    pub last_loaded_skills: usize,
    pub active_sessions: usize,
    pub total_events_logged: u64,
    pub last_model_tier: Option<String>,
    pub last_latency_ms: Option<u64>,
    pub optimizations: OptimizationFlags,
}

/// JSON-RPC 2.0 request envelope sent over stdio to the lazy Cordis sidecar.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CordisRpcRequest {
    pub jsonrpc: String,
    pub id: String,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

impl CordisRpcRequest {
    pub fn new(id: impl Into<String>, method: impl Into<String>, params: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: id.into(),
            method: method.into(),
            params,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CordisRpcError {
    pub code: i32,
    pub message: String,
    #[serde(default)]
    pub data: Option<Value>,
}

/// JSON-RPC 2.0 response envelope returned from the lazy Cordis sidecar.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CordisRpcResponse {
    pub jsonrpc: String,
    pub id: String,
    #[serde(default)]
    pub result: Option<Value>,
    #[serde(default)]
    pub error: Option<CordisRpcError>,
}

/// Atomic snapshot record stored by Layer 0 Watchdog for <1s rollback.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotRecord {
    pub id: String,
    pub label: String,
    pub graph_json: String,
    pub created_at_ms: i64,
    pub is_good: bool,
}

/// Returns current UNIX epoch milliseconds.
pub fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

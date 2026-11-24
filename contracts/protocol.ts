/**
 * Shared Carnival Unified Harness Protocol v1.0.0
 * Single source of truth for the Orchestrator, Lazy Cordis Sidecar,
 * spatial companion bridge, and Tauri v2 desktop UI.
 */

export type HarnessId = "openbitfun" | "deepseek_harness" | "clicky";

export type LaneKind =
  | "clicky_ambient"
  | "builder_workspace"
  | "optimization_deck";

export type CordisState = "stopped" | "starting" | "running" | "degraded";

export type ModelTier = "local_triage" | "deepseek_v4" | "byok_cloud";

export interface OptimizationFlags {
  prefix_cache_lock: boolean;
  skill_tree_pruning: boolean;
  ptc_batching: boolean;
  local_triage: boolean;
}

export interface CapabilityEnvelope {
  subject: string;
  fs_read: string[];
  fs_write: string[];
  network_allow: boolean;
  cua_apps: string[];
  allow_vision: boolean;
  allow_audio: boolean;
}

export type SpatialCommand =
  | {
      type: "point";
      x: number;
      y: number;
      label: string;
      screen?: number | null;
    }
  | {
      type: "draw";
      shape: "rect" | "arrow" | "circle" | "freehand";
      x: number;
      y: number;
      width: number;
      height: number;
      caption?: string | null;
    }
  | {
      type: "highlight";
      ax_token: string;
    }
  | {
      type: "clear";
    };

export interface ScreenContext {
  active_app?: string | null;
  window_title?: string | null;
  screenshot_ref?: string | null;
  ax_tree_digest?: string | null;
  cursor_x?: number | null;
  cursor_y?: number | null;
}

export interface TurnRequest {
  request_id: string;
  session_id: string;
  lane: LaneKind;
  prompt: string;
  screen_context?: ScreenContext | null;
  optimizations?: OptimizationFlags | null;
}

export type CuaActionKind =
  | "click"
  | "double_click"
  | "type_text"
  | "key_combo"
  | "scroll";

export interface CuaActionRequest {
  action_id: string;
  target_app: string;
  action: CuaActionKind;
  params: Record<string, unknown>;
  background?: boolean;
}

export type HarnessEventKind =
  | "text_delta"
  | "tts_chunk"
  | "spatial_command"
  | "cua_decision"
  | "tool_status"
  | "telemetry"
  | "cordis_lifecycle"
  | "turn_complete"
  | "error";

export interface HarnessEvent {
  seq: number;
  session_id: string;
  timestamp_ms: number;
  kind: HarnessEventKind;
  payload: Record<string, unknown>;
}

export interface TelemetrySnapshot {
  cordis_state: CordisState;
  cordis_pid?: number | null;
  cordis_request_count: number;
  cordis_restart_count: number;
  prefix_cache_hits: number;
  prefix_cache_misses: number;
  prefix_cache_hit_rate: number;
  total_skills: number;
  last_loaded_skills: number;
  active_sessions: number;
  total_events_logged: number;
  last_model_tier?: ModelTier | string | null;
  last_latency_ms?: number | null;
  optimizations: OptimizationFlags;
}

export interface CordisRpcRequest<P = Record<string, unknown>> {
  jsonrpc: "2.0";
  id: string;
  method: "ping" | "discover_harnesses" | "route_turn" | "shutdown";
  params?: P;
}

export interface CordisRpcError {
  code: number;
  message: string;
  data?: unknown;
}

export interface CordisRpcResponse<R = Record<string, unknown>> {
  jsonrpc: "2.0";
  id: string;
  result?: R;
  error?: CordisRpcError;
}

export interface SnapshotRecord {
  id: string;
  label: string;
  graph_json: string;
  created_at_ms: number;
  is_good: boolean;
}

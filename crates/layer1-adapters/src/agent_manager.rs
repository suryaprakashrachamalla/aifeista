use std::collections::{HashMap, VecDeque};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::workspace_core::WorkspaceCoreAdapter;

const MAX_RAW_LOG_BYTES: usize = 256 * 1024;
const MAX_MASKED_TURNS: usize = 120;
const MAX_TRAIL_EVENTS: usize = 80;

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn augmented_path() -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    let mut parts = vec![
        "/opt/homebrew/bin".to_string(),
        "/usr/local/bin".to_string(),
        "/usr/bin".to_string(),
        "/bin".to_string(),
    ];
    if !home.is_empty() {
        parts.insert(0, format!("{home}/.local/bin"));
        parts.insert(1, format!("{home}/.npm-global/bin"));
        parts.insert(2, format!("{home}/.cargo/bin"));
    }
    if let Ok(existing) = std::env::var("PATH") {
        for seg in existing.split(':') {
            if !seg.is_empty() && !parts.iter().any(|p| p == seg) {
                parts.push(seg.to_string());
            }
        }
    }
    parts.join(":")
}

fn resolve_binary(bin: &str) -> Option<String> {
    if bin.contains('/') && Path::new(bin).exists() {
        return Some(bin.to_string());
    }
    for dir in augmented_path().split(':') {
        let candidate = Path::new(dir).join(bin);
        if candidate.is_file() {
            return Some(candidate.display().to_string());
        }
    }
    None
}

fn detect_git_branch(cwd: &Path) -> String {
    let out = StdCommand::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(cwd)
        .output();
    match out {
        Ok(o) if o.status.success() => {
            let b = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if b.is_empty() {
                "detached".to_string()
            } else {
                b
            }
        }
        _ => "main".to_string(),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    NeedsYou,
    Working,
    Done,
    Idle,
    Stopped,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentViewMode {
    Harness,
    Terminal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentPreset {
    pub id: String,
    pub label: String,
    pub binary: String,
    pub installed: bool,
    pub resolved_path: Option<String>,
    pub description: String,
    pub badge_color: String,
    pub is_pty: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaskedTurn {
    pub id: String,
    pub role: String, // "user" | "assistant" | "tool" | "approval" | "system"
    pub title: String,
    pub content: String,
    pub tool_name: Option<String>,
    pub timestamp_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrailEvent {
    pub id: String,
    pub timestamp_ms: i64,
    pub kind: String, // "spawn" | "tool" | "status" | "approval" | "input"
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteNodeTag {
    pub peer_id: String,
    pub peer_name: String,
    pub transport: String, // "ssh" | "tailscale" | "loopback"
    pub host: String,
    pub remote_path: String,
    pub handoff_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagedAgentSummary {
    pub id: String,
    pub name: String,
    pub engine: String,
    pub engine_label: String,
    pub cwd: String,
    pub branch: String,
    pub command: String,
    pub status: AgentStatus,
    pub view_mode: AgentViewMode,
    pub is_pty: bool,
    pub pid: Option<u32>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
    pub elapsed_ms: i64,
    pub latest_summary: String,
    pub turn_count: usize,
    pub approval_prompt: Option<String>,
    pub raw_line_count: usize,
    pub remote_node: Option<RemoteNodeTag>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagedAgentDetail {
    #[serde(flatten)]
    pub summary: ManagedAgentSummary,
    pub raw_log: String,
    pub masked_turns: Vec<MaskedTurn>,
    pub trail: Vec<TrailEvent>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SpawnAgentRequest {
    #[serde(default)]
    pub name: Option<String>,
    pub engine: String,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub prompt: Option<String>,
    #[serde(default)]
    pub custom_command: Option<String>,
    #[serde(default)]
    pub view_mode: Option<AgentViewMode>,
    #[serde(default)]
    pub remote_node: Option<RemoteNodeTag>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentDiffResult {
    pub agent_id: String,
    pub cwd: String,
    pub branch: String,
    pub status_lines: Vec<String>,
    pub diff_stat: String,
    pub diff_patch: String,
}

struct AgentHandle {
    writer: Option<Box<dyn Write + Send>>,
    master: Option<Box<dyn MasterPty + Send>>,
    child: Option<Box<dyn Child + Send + Sync>>,
}

struct AgentRecord {
    id: String,
    name: String,
    engine: String,
    engine_label: String,
    cwd: String,
    branch: String,
    command: String,
    status: AgentStatus,
    view_mode: AgentViewMode,
    is_pty: bool,
    pid: Option<u32>,
    created_at_ms: i64,
    updated_at_ms: i64,
    raw_log: String,
    pending_line: String,
    masked_turns: VecDeque<MaskedTurn>,
    trail: VecDeque<TrailEvent>,
    approval_prompt: Option<String>,
    remote_node: Option<RemoteNodeTag>,
    handle: AgentHandle,
}

impl AgentRecord {
    fn summary(&self) -> ManagedAgentSummary {
        let now = now_ms();
        let latest_summary = self
            .approval_prompt
            .clone()
            .or_else(|| {
                self.masked_turns
                    .back()
                    .map(|t| format!("{}: {}", t.title, t.content))
            })
            .unwrap_or_else(|| format!("{} ready in {}", self.engine_label, self.branch));

        ManagedAgentSummary {
            id: self.id.clone(),
            name: self.name.clone(),
            engine: self.engine.clone(),
            engine_label: self.engine_label.clone(),
            cwd: self.cwd.clone(),
            branch: self.branch.clone(),
            command: self.command.clone(),
            status: self.status.clone(),
            view_mode: self.view_mode.clone(),
            is_pty: self.is_pty,
            pid: self.pid,
            created_at_ms: self.created_at_ms,
            updated_at_ms: self.updated_at_ms,
            elapsed_ms: (now - self.created_at_ms).max(0),
            latest_summary: truncate_chars(&latest_summary, 140),
            turn_count: self.masked_turns.len(),
            approval_prompt: self.approval_prompt.clone(),
            raw_line_count: self.raw_log.lines().count(),
            remote_node: self.remote_node.clone(),
        }
    }

    fn detail(&self) -> ManagedAgentDetail {
        ManagedAgentDetail {
            summary: self.summary(),
            raw_log: self.raw_log.clone(),
            masked_turns: self.masked_turns.iter().cloned().collect(),
            trail: self.trail.iter().cloned().collect(),
        }
    }

    fn push_turn(&mut self, turn: MaskedTurn) {
        if let Some(last) = self.masked_turns.back_mut() {
            if last.role == "assistant"
                && turn.role == "assistant"
                && (turn.timestamp_ms - last.timestamp_ms) < 2500
                && last.content.len() < 1200
            {
                if !last.content.is_empty() {
                    last.content.push('\n');
                }
                last.content.push_str(&turn.content);
                last.timestamp_ms = turn.timestamp_ms;
                return;
            }
        }
        if self.masked_turns.len() >= MAX_MASKED_TURNS {
            self.masked_turns.pop_front();
        }
        self.masked_turns.push_back(turn);
    }

    fn push_trail(&mut self, kind: &str, summary: impl Into<String>) {
        if self.trail.len() >= MAX_TRAIL_EVENTS {
            self.trail.pop_front();
        }
        self.trail.push_back(TrailEvent {
            id: Uuid::new_v4().to_string(),
            timestamp_ms: now_ms(),
            kind: kind.to_string(),
            summary: summary.into(),
        });
    }
}

fn truncate_chars(s: &str, max_chars: usize) -> String {
    let mut out: String = s.chars().take(max_chars).collect();
    if s.chars().count() > max_chars {
        out.push('…');
    }
    out
}

/// Strips ANSI CSI/OSC escape sequences and extracts Carnival OSC 7501 status updates.
pub fn parse_osc_7501_and_strip_ansi(input: &str) -> (String, Option<(AgentStatus, Option<String>)>) {
    let mut cleaned = String::with_capacity(input.len());
    let mut detected_status: Option<(AgentStatus, Option<String>)> = None;
    let bytes = input.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == 0x1b {
            if i + 1 < bytes.len() && bytes[i + 1] == b'[' {
                // CSI sequence: ESC [ ... final byte 0x40..0x7E
                i += 2;
                while i < bytes.len() && !(0x40..=0x7E).contains(&bytes[i]) {
                    i += 1;
                }
                if i < bytes.len() {
                    i += 1;
                }
                continue;
            } else if i + 1 < bytes.len() && bytes[i + 1] == b']' {
                // OSC sequence: ESC ] ... (BEL 0x07 or ST ESC \)
                i += 2;
                let start = i;
                while i < bytes.len() {
                    if bytes[i] == 0x07 {
                        let osc = String::from_utf8_lossy(&bytes[start..i]).to_string();
                        if let Some(parsed) = parse_osc_payload(&osc) {
                            detected_status = Some(parsed);
                        }
                        i += 1;
                        break;
                    }
                    if bytes[i] == 0x1b && i + 1 < bytes.len() && bytes[i + 1] == b'\\' {
                        let osc = String::from_utf8_lossy(&bytes[start..i]).to_string();
                        if let Some(parsed) = parse_osc_payload(&osc) {
                            detected_status = Some(parsed);
                        }
                        i += 2;
                        break;
                    }
                    i += 1;
                }
                continue;
            } else {
                i += 2;
                continue;
            }
        }
        let rest = &input[i..];
        if let Some(ch) = rest.chars().next() {
            if ch == '\r' {
                // skip lone carriage return; \r\n leaves \n cleanly
            } else if !ch.is_control() || ch == '\n' || ch == '\t' {
                cleaned.push(ch);
            }
            i += ch.len_utf8();
        } else {
            i += 1;
        }
    }

    (cleaned, detected_status)
}

fn parse_osc_payload(osc: &str) -> Option<(AgentStatus, Option<String>)> {
    let rest = osc.strip_prefix("7501;")?;
    let mut status: Option<AgentStatus> = None;
    let mut msg: Option<String> = None;
    for part in rest.split(';') {
        if let Some((k, v)) = part.split_once('=') {
            match k.trim() {
                "status" => {
                    status = match v.trim().to_lowercase().as_str() {
                        "working" | "running" | "busy" => Some(AgentStatus::Working),
                        "needs_you" | "blocked" | "approval" | "waiting" => Some(AgentStatus::NeedsYou),
                        "done" | "complete" | "completed" => Some(AgentStatus::Done),
                        "idle" | "ready" => Some(AgentStatus::Idle),
                        _ => None,
                    };
                }
                "msg" | "message" | "prompt" => {
                    let trimmed = v.trim();
                    if !trimmed.is_empty() {
                        msg = Some(trimmed.to_string());
                    }
                }
                _ => {}
            }
        }
    }
    status.map(|s| (s, msg))
}

fn classify_line_turn(line: &str) -> MaskedTurn {
    let trimmed = line.trim();
    let lower = trimmed.to_lowercase();
    let ts = now_ms();

    if lower.contains("[y/n]")
        || lower.contains("(y/n)")
        || lower.contains("allow this")
        || lower.contains("permission")
        || lower.contains("approve")
        || lower.contains("confirm before")
        || lower.contains("needs your input")
    {
        return MaskedTurn {
            id: Uuid::new_v4().to_string(),
            role: "approval".to_string(),
            title: "Approval Required".to_string(),
            content: trimmed.to_string(),
            tool_name: Some("PermissionGate".to_string()),
            timestamp_ms: ts,
        };
    }

    for (prefix, tool) in [
        ("read(", "Read"),
        ("edit(", "Edit"),
        ("write(", "Write"),
        ("bash(", "Bash"),
        ("grep(", "FlashGrep"),
        ("git ", "Git"),
        ("cargo ", "Cargo"),
        ("pnpm ", "Build"),
        ("npm ", "Build"),
        ("▸ tool:", "Tool"),
    ] {
        if lower.starts_with(prefix) || lower.contains(&format!("▸ {prefix}")) {
            return MaskedTurn {
                id: Uuid::new_v4().to_string(),
                role: "tool".to_string(),
                title: format!("{tool} Invocation"),
                content: trimmed.to_string(),
                tool_name: Some(tool.to_string()),
                timestamp_ms: ts,
            };
        }
    }

    MaskedTurn {
        id: Uuid::new_v4().to_string(),
        role: "assistant".to_string(),
        title: "Agent Output".to_string(),
        content: trimmed.to_string(),
        tool_name: None,
        timestamp_ms: ts,
    }
}

struct ManagerInner {
    default_workspace: PathBuf,
    workspace_core: Option<WorkspaceCoreAdapter>,
    agents: RwLock<HashMap<String, Arc<Mutex<AgentRecord>>>>,
    order: RwLock<Vec<String>>,
    events_tx: broadcast::Sender<Value>,
}

#[derive(Clone)]
pub struct AgentManager {
    inner: Arc<ManagerInner>,
}

impl AgentManager {
    pub fn new(default_workspace: impl Into<PathBuf>, workspace_core: Option<WorkspaceCoreAdapter>) -> Self {
        let (events_tx, _) = broadcast::channel(256);
        Self {
            inner: Arc::new(ManagerInner {
                default_workspace: default_workspace.into(),
                workspace_core,
                agents: RwLock::new(HashMap::new()),
                order: RwLock::new(Vec::new()),
                events_tx,
            }),
        }
    }

    pub fn subscribe_events(&self) -> broadcast::Receiver<Value> {
        self.inner.events_tx.subscribe()
    }

    fn emit(&self, kind: &str, payload: Value) {
        let _ = self.inner.events_tx.send(json!({
            "kind": kind,
            "timestamp_ms": now_ms(),
            "payload": payload,
        }));
    }

    pub fn presets(&self) -> Vec<AgentPreset> {
        let specs = [
            (
                "claude-code",
                "Claude Code",
                "claude",
                "Anthropic CLI coding agent wrapped into live Harness UI + PTY",
                "#f59e0b",
                true,
            ),
            (
                "codex",
                "Codex CLI",
                "codex",
                "OpenAI Codex CLI coding agent with sandbox diff tracking",
                "#38bdf8",
                true,
            ),
            (
                "gemini-cli",
                "Gemini CLI",
                "gemini",
                "Google Gemini CLI coding agent inside multi-pane studio",
                "#a78bfa",
                true,
            ),
            (
                "opencode",
                "OpenCode",
                "opencode",
                "OpenCode terminal coding agent with OSC 7501 status support",
                "#34d399",
                true,
            ),
            (
                "cursor-agent",
                "Cursor Agent",
                "cursor-agent",
                "Cursor headless CLI agent managed in persistent workspace",
                "#60a5fa",
                true,
            ),
            (
                "carnival-orchestrator",
                "AI Carnival Orchestrator",
                "internal",
                "Built-in AI Carnival Orchestrator worker backed by FlashGrep + Seatbelt",
                "#f472b6",
                false,
            ),
            (
                "shell",
                "Interactive Shell / Custom CLI",
                "zsh",
                "Persistent macOS zsh pseudo-terminal or custom command runner",
                "#94a3b8",
                true,
            ),
        ];

        specs
            .into_iter()
            .map(|(id, label, binary, description, badge_color, is_pty)| {
                let resolved_path = if is_pty {
                    resolve_binary(binary)
                } else {
                    Some("carnival://orchestrator".to_string())
                };
                AgentPreset {
                    id: id.to_string(),
                    label: label.to_string(),
                    binary: binary.to_string(),
                    installed: resolved_path.is_some(),
                    resolved_path,
                    description: description.to_string(),
                    badge_color: badge_color.to_string(),
                    is_pty,
                }
            })
            .collect()
    }

    pub fn list(&self) -> Vec<ManagedAgentSummary> {
        let order = self.inner.order.read().map(|o| o.clone()).unwrap_or_default();
        let map = self.inner.agents.read();
        let Ok(agents) = map else { return Vec::new() };
        order
            .iter()
            .filter_map(|id| agents.get(id))
            .filter_map(|rec| rec.lock().ok().map(|g| g.summary()))
            .collect()
    }

    pub fn get(&self, id: &str) -> Option<ManagedAgentDetail> {
        let agents = self.inner.agents.read().ok()?;
        let rec = agents.get(id)?.clone();
        drop(agents);
        let detail = rec.lock().ok().map(|g| g.detail());
        detail
    }

    pub fn spawn(&self, req: SpawnAgentRequest) -> Result<ManagedAgentDetail, String> {
        let presets = self.presets();
        let preset = presets.iter().find(|p| p.id == req.engine);
        let engine_label = preset
            .map(|p| p.label.clone())
            .unwrap_or_else(|| req.engine.clone());

        let cwd_path = req
            .cwd
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| self.inner.default_workspace.clone());
        let cwd_str = cwd_path.display().to_string();
        let branch = detect_git_branch(&cwd_path);

        let existing_count = self.inner.order.read().map(|o| o.len()).unwrap_or(0) + 1;
        let callsign = req
            .name
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|| format!("{} #{existing_count}", req.engine));

        let id = format!("ag-{}", &Uuid::new_v4().to_string()[..8]);
        let initial_view = req.view_mode.unwrap_or(AgentViewMode::Harness);
        let remote_node = req.remote_node.clone();

        // Internal AI Carnival Orchestrator worker
        if req.engine == "carnival-orchestrator" && remote_node.is_none() {
            return self.spawn_internal_orchestrator(id, callsign, engine_label, cwd_str, branch, initial_view, req.prompt);
        }

        // Determine shell/command to run inside real PTY
        let is_shell_only = req.engine == "shell" && req.custom_command.as_deref().unwrap_or("").trim().is_empty();
        let custom_cmd = req
            .custom_command
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty());

        let (prog, args, display_cmd): (String, Vec<String>, String) = if let Some(cmd) = custom_cmd {
            let shell = resolve_binary("zsh").unwrap_or_else(|| "/bin/zsh".to_string());
            (
                shell,
                vec!["-lc".to_string(), cmd.to_string()],
                cmd.to_string(),
            )
        } else if is_shell_only {
            let shell = resolve_binary("zsh").unwrap_or_else(|| "/bin/zsh".to_string());
            (shell.clone(), vec!["-i".to_string()], "zsh -i".to_string())
        } else {
            let bin = preset.map(|p| p.binary.as_str()).unwrap_or(&req.engine);
            let resolved = resolve_binary(bin).ok_or_else(|| {
                format!(
                    "CLI binary '{}' is not installed on PATH. Install it or choose 'Carnival Orchestrator' / 'Interactive Shell' or pass a custom command.",
                    bin
                )
            })?;
            let mut argv = Vec::new();
            if let Some(p) = req.prompt.as_deref().filter(|s| !s.trim().is_empty()) {
                argv.push(p.to_string());
            }
            let disp = if argv.is_empty() {
                bin.to_string()
            } else {
                format!("{bin} {:?}", argv[0])
            };
            (resolved, argv, disp)
        };

        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: 30,
                cols: 110,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| format!("failed to open PTY: {e}"))?;

        let mut cmd_builder = CommandBuilder::new(&prog);
        for a in &args {
            cmd_builder.arg(a);
        }
        cmd_builder.cwd(&cwd_path);
        cmd_builder.env("PATH", augmented_path());
        cmd_builder.env("TERM", "xterm-256color");
        cmd_builder.env("AI_CARNIVAL_AGENT_ID", &id);
        cmd_builder.env("AI_CARNIVAL_OSC_STATUS", "7501");

        let child = pair
            .slave
            .spawn_command(cmd_builder)
            .map_err(|e| format!("failed to spawn PTY child: {e}"))?;
        let pid = child.process_id();
        drop(pair.slave);

        let reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| format!("failed to clone PTY reader: {e}"))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|e| format!("failed to take PTY writer: {e}"))?;

        let created = now_ms();
        let mut initial_turns = VecDeque::new();
        initial_turns.push_back(MaskedTurn {
            id: Uuid::new_v4().to_string(),
            role: "system".to_string(),
            title: "Session Started".to_string(),
            content: format!("Mounted {engine_label} in {cwd_str} ({branch})"),
            tool_name: Some("RuntimePTY".to_string()),
            timestamp_ms: created,
        });
        if let Some(p) = req.prompt.as_deref().filter(|s| !s.trim().is_empty()) {
            initial_turns.push_back(MaskedTurn {
                id: Uuid::new_v4().to_string(),
                role: "user".to_string(),
                title: "Initial Prompt".to_string(),
                content: p.to_string(),
                tool_name: None,
                timestamp_ms: created + 1,
            });
        }

        let mut initial_trail = VecDeque::new();
        initial_trail.push_back(TrailEvent {
            id: Uuid::new_v4().to_string(),
            timestamp_ms: created,
            kind: "spawn".to_string(),
            summary: format!("Spawned {callsign} ({display_cmd})"),
        });

        let rec = Arc::new(Mutex::new(AgentRecord {
            id: id.clone(),
            name: callsign,
            engine: req.engine.clone(),
            engine_label,
            cwd: cwd_str,
            branch,
            command: display_cmd,
            status: if custom_cmd.is_some() || req.prompt.is_some() {
                AgentStatus::Working
            } else {
                AgentStatus::Idle
            },
            view_mode: initial_view,
            is_pty: true,
            pid,
            created_at_ms: created,
            updated_at_ms: created,
            raw_log: String::new(),
            pending_line: String::new(),
            masked_turns: initial_turns,
            trail: initial_trail,
            approval_prompt: None,
            remote_node,
            handle: AgentHandle {
                writer: Some(writer),
                master: Some(pair.master),
                child: Some(child),
            },
        }));

        if let Ok(mut agents) = self.inner.agents.write() {
            agents.insert(id.clone(), rec.clone());
        }
        if let Ok(mut order) = self.inner.order.write() {
            order.push(id.clone());
        }

        // If shell mode + initial prompt without custom command, write prompt into PTY stdin
        if is_shell_only {
            if let Some(p) = req.prompt.as_deref().filter(|s| !s.trim().is_empty()) {
                let _ = self.write_raw_to_record(&rec, format!("{p}\n").as_bytes());
            }
        }

        self.start_pty_reader(id.clone(), rec.clone(), reader);

        let detail = rec.lock().map(|g| g.detail()).map_err(|e| e.to_string())?;
        self.emit("agent_spawned", json!(detail.summary));
        Ok(detail)
    }

    fn spawn_internal_orchestrator(
        &self,
        id: String,
        name: String,
        engine_label: String,
        cwd: String,
        branch: String,
        view_mode: AgentViewMode,
        prompt: Option<String>,
    ) -> Result<ManagedAgentDetail, String> {
        let created = now_ms();
        let initial_prompt = prompt
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "Inspect workspace & coordinate Orchestrator lanes.".to_string());

        let mut masked_turns = VecDeque::new();
        masked_turns.push_back(MaskedTurn {
            id: Uuid::new_v4().to_string(),
            role: "system".to_string(),
            title: "Orchestrator Worker Mounted".to_string(),
            content: format!("Connected to FlashGrep + Seatbelt sandbox in {cwd} ({branch})."),
            tool_name: Some("Orchestrator".to_string()),
            timestamp_ms: created,
        });
        masked_turns.push_back(MaskedTurn {
            id: Uuid::new_v4().to_string(),
            role: "user".to_string(),
            title: "Directive".to_string(),
            content: initial_prompt.clone(),
            tool_name: None,
            timestamp_ms: created + 1,
        });

        let search_hits = self
            .inner
            .workspace_core
            .as_ref()
            .map(|core| core.index.search(&initial_prompt, 4))
            .unwrap_or_default();
        let hit_summary = if search_hits.is_empty() {
            "Ready — send prompts below to query workspace, stage plans, or inspect diffs.".to_string()
        } else {
            let paths: Vec<String> = search_hits
                .iter()
                .map(|m| format!("{}:{}", m.path, m.line_no))
                .collect();
            format!("FlashGrep pre-indexed matches: {}", paths.join(", "))
        };
        masked_turns.push_back(MaskedTurn {
            id: Uuid::new_v4().to_string(),
            role: "assistant".to_string(),
            title: "AI Carnival Orchestrator".to_string(),
            content: hit_summary.clone(),
            tool_name: Some("FlashGrep".to_string()),
            timestamp_ms: created + 2,
        });

        let mut trail = VecDeque::new();
        trail.push_back(TrailEvent {
            id: Uuid::new_v4().to_string(),
            timestamp_ms: created,
            kind: "spawn".to_string(),
            summary: format!("Started internal Orchestrator worker {name}"),
        });

        let raw_log = format!(
            "[AI Carnival Orchestrator]\n$ orchestrator attach --workspace \"{cwd}\" --branch {branch}\n> Directive: {initial_prompt}\n< {hit_summary}\n"
        );

        let rec = Arc::new(Mutex::new(AgentRecord {
            id: id.clone(),
            name,
            engine: "carnival-orchestrator".to_string(),
            engine_label,
            cwd,
            branch,
            command: "orchestrator-worker".to_string(),
            status: AgentStatus::Done,
            view_mode,
            is_pty: false,
            pid: None,
            created_at_ms: created,
            updated_at_ms: created,
            raw_log,
            pending_line: String::new(),
            masked_turns,
            trail,
            approval_prompt: None,
            remote_node: None,
            handle: AgentHandle {
                writer: None,
                master: None,
                child: None,
            },
        }));

        if let Ok(mut agents) = self.inner.agents.write() {
            agents.insert(id.clone(), rec.clone());
        }
        if let Ok(mut order) = self.inner.order.write() {
            order.push(id);
        }

        let detail = rec.lock().map(|g| g.detail()).map_err(|e| e.to_string())?;
        self.emit("agent_spawned", json!(detail.summary));
        Ok(detail)
    }

    fn write_raw_to_record(&self, rec: &Arc<Mutex<AgentRecord>>, bytes: &[u8]) -> Result<(), String> {
        let mut guard = rec.lock().map_err(|e| e.to_string())?;
        if let Some(writer) = guard.handle.writer.as_mut() {
            writer
                .write_all(bytes)
                .map_err(|e| format!("write to PTY failed: {e}"))?;
            let _ = writer.flush();
            Ok(())
        } else {
            Err("agent has no active PTY writer".to_string())
        }
    }

    fn start_pty_reader(
        &self,
        id: String,
        rec: Arc<Mutex<AgentRecord>>,
        mut reader: Box<dyn Read + Send>,
    ) {
        let manager = self.clone();
        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => {
                        let summary_opt = {
                            if let Ok(mut guard) = rec.lock() {
                                if !matches!(guard.status, AgentStatus::Stopped) {
                                    guard.status = AgentStatus::Done;
                                }
                                guard.updated_at_ms = now_ms();
                                guard.approval_prompt = None;
                                guard.push_trail("status", "PTY process completed stream");
                                Some(guard.summary())
                            } else {
                                None
                            }
                        };
                        if let Some(s) = summary_opt {
                            manager.emit("agent_updated", json!(s));
                        }
                        break;
                    }
                    Ok(n) => {
                        let chunk = String::from_utf8_lossy(&buf[..n]).to_string();
                        let summary_opt = {
                            if let Ok(mut guard) = rec.lock() {
                                guard.raw_log.push_str(&chunk);
                                if guard.raw_log.len() > MAX_RAW_LOG_BYTES {
                                    let drain = guard.raw_log.len() - MAX_RAW_LOG_BYTES;
                                    let valid = guard.raw_log.ceil_char_boundary(drain);
                                    guard.raw_log.drain(..valid);
                                }
                                guard.updated_at_ms = now_ms();

                                let (cleaned, osc_status) = parse_osc_7501_and_strip_ansi(&chunk);
                                if let Some((st, msg)) = osc_status {
                                    guard.status = st.clone();
                                    if matches!(st, AgentStatus::NeedsYou) {
                                        guard.approval_prompt = msg.clone().or_else(|| {
                                            Some("Agent requested approval via OSC 7501".to_string())
                                        });
                                    } else {
                                        guard.approval_prompt = None;
                                    }
                                    guard.push_trail(
                                        "status",
                                        format!("Status -> {:?}{}", st, msg.as_deref().map(|m| format!(": {m}")).unwrap_or_default()),
                                    );
                                }

                                guard.pending_line.push_str(&cleaned);
                                let full_lines = std::mem::take(&mut guard.pending_line);
                                let mut parts: Vec<&str> = full_lines.split('\n').collect();
                                if !full_lines.ends_with('\n') {
                                    if let Some(tail) = parts.pop() {
                                        guard.pending_line = tail.to_string();
                                    }
                                }

                                for line in parts {
                                    let trimmed = line.trim();
                                    if trimmed.is_empty() {
                                        continue;
                                    }
                                    let turn = classify_line_turn(trimmed);
                                    if turn.role == "approval" {
                                        guard.status = AgentStatus::NeedsYou;
                                        guard.approval_prompt = Some(turn.content.clone());
                                        guard.push_trail("approval", format!("Blocked: {}", turn.content));
                                    } else if turn.role == "tool" {
                                        guard.status = AgentStatus::Working;
                                        guard.push_trail("tool", turn.content.clone());
                                    }
                                    guard.push_turn(turn);
                                }

                                // If partial prompt line ends with ? / [y/N] without trailing newline, detect it too
                                let tail_trimmed = guard.pending_line.trim().to_string();
                                if !tail_trimmed.is_empty()
                                    && (tail_trimmed.to_lowercase().contains("[y/n]")
                                        || tail_trimmed.to_lowercase().contains("approve?")
                                        || tail_trimmed.to_lowercase().contains("allow?"))
                                {
                                    guard.status = AgentStatus::NeedsYou;
                                    guard.approval_prompt = Some(tail_trimmed.clone());
                                    let turn = classify_line_turn(&tail_trimmed);
                                    guard.push_turn(turn);
                                    guard.pending_line.clear();
                                }

                                Some(guard.summary())
                            } else {
                                None
                            }
                        };
                        if let Some(s) = summary_opt {
                            manager.emit("agent_output", json!({ "agent_id": id, "summary": s }));
                        }
                    }
                    Err(_) => break,
                }
            }
        });
    }

    pub fn send_input(&self, id: &str, text: &str, append_newline: bool) -> Result<ManagedAgentDetail, String> {
        let rec = self
            .inner
            .agents
            .read()
            .ok()
            .and_then(|m| m.get(id).cloned())
            .ok_or_else(|| format!("agent not found: {id}"))?;

        let is_pty = rec.lock().map(|g| g.is_pty).unwrap_or(true);
        if !is_pty {
            return self.handle_internal_turn(rec, text);
        }

        let payload = if append_newline && !text.ends_with('\n') && !text.ends_with('\r') {
            format!("{text}\n")
        } else {
            text.to_string()
        };

        self.write_raw_to_record(&rec, payload.as_bytes())?;

        if let Ok(mut guard) = rec.lock() {
            let trimmed = text.trim();
            if !trimmed.is_empty() && !trimmed.starts_with('\x03') {
                guard.push_turn(MaskedTurn {
                    id: Uuid::new_v4().to_string(),
                    role: "user".to_string(),
                    title: "Sent to Agent".to_string(),
                    content: trimmed.to_string(),
                    tool_name: None,
                    timestamp_ms: now_ms(),
                });
                guard.push_trail("input", format!("Sent input: {}", truncate_chars(trimmed, 70)));
            }
            guard.status = AgentStatus::Working;
            guard.approval_prompt = None;
            guard.updated_at_ms = now_ms();
        }

        let detail = rec.lock().map(|g| g.detail()).map_err(|e| e.to_string())?;
        self.emit("agent_updated", json!(detail.summary));
        Ok(detail)
    }

    fn handle_internal_turn(
        &self,
        rec: Arc<Mutex<AgentRecord>>,
        prompt: &str,
    ) -> Result<ManagedAgentDetail, String> {
        let trimmed = prompt.trim();
        let ts = now_ms();
        let matches = self
            .inner
            .workspace_core
            .as_ref()
            .map(|core| core.index.search(trimmed, 5))
            .unwrap_or_default();

        let response_text = if matches.is_empty() {
            format!("Orchestrator queued & indexed directive: \"{trimmed}\". Workspace tree clean.")
        } else {
            let refs: Vec<String> = matches
                .iter()
                .map(|m| format!("• {}:{} — {}", m.path, m.line_no, truncate_chars(m.snippet.trim(), 68)))
                .collect();
            format!(
                "FlashGrep matched {} relevant symbols for \"{trimmed}\":\n{}",
                matches.len(),
                refs.join("\n")
            )
        };

        if let Ok(mut guard) = rec.lock() {
            guard.push_turn(MaskedTurn {
                id: Uuid::new_v4().to_string(),
                role: "user".to_string(),
                title: "User Prompt".to_string(),
                content: trimmed.to_string(),
                tool_name: None,
                timestamp_ms: ts,
            });
            guard.push_turn(MaskedTurn {
                id: Uuid::new_v4().to_string(),
                role: "assistant".to_string(),
                title: "AI Carnival Orchestrator".to_string(),
                content: response_text.clone(),
                tool_name: Some("FlashGrep".to_string()),
                timestamp_ms: ts + 1,
            });
            guard.raw_log.push_str(&format!("\n> {trimmed}\n< {response_text}\n"));
            guard.push_trail("tool", format!("Orchestrator responded ({} FlashGrep hits)", matches.len()));
            guard.status = AgentStatus::Done;
            guard.approval_prompt = None;
            guard.updated_at_ms = now_ms();
        }

        let detail = rec.lock().map(|g| g.detail()).map_err(|e| e.to_string())?;
        self.emit("agent_updated", json!(detail.summary));
        Ok(detail)
    }

    pub fn approve(&self, id: &str, allow: bool) -> Result<ManagedAgentDetail, String> {
        let reply = if allow { "y" } else { "n" };
        let rec = self
            .inner
            .agents
            .read()
            .ok()
            .and_then(|m| m.get(id).cloned())
            .ok_or_else(|| format!("agent not found: {id}"))?;
        let is_pty = rec.lock().map(|g| g.is_pty).unwrap_or(false);
        if is_pty {
            self.write_raw_to_record(&rec, format!("{reply}\n").as_bytes())?;
        }
        if let Ok(mut guard) = rec.lock() {
            guard.push_turn(MaskedTurn {
                id: Uuid::new_v4().to_string(),
                role: "system".to_string(),
                title: if allow { "Approved Gate" } else { "Denied Gate" }.to_string(),
                content: if allow {
                    "Sent approval ('y') to agent.".to_string()
                } else {
                    "Denied permission ('n').".to_string()
                },
                tool_name: Some("PermissionGate".to_string()),
                timestamp_ms: now_ms(),
            });
            guard.push_trail("approval", format!("User {} gate", if allow { "approved" } else { "denied" }));
            guard.approval_prompt = None;
            guard.status = if allow { AgentStatus::Working } else { AgentStatus::Idle };
            guard.updated_at_ms = now_ms();
        }
        let detail = rec.lock().map(|g| g.detail()).map_err(|e| e.to_string())?;
        self.emit("agent_updated", json!(detail.summary));
        Ok(detail)
    }

    pub fn set_view_mode(&self, id: &str, mode: AgentViewMode) -> Result<ManagedAgentDetail, String> {
        let rec = self
            .inner
            .agents
            .read()
            .ok()
            .and_then(|m| m.get(id).cloned())
            .ok_or_else(|| format!("agent not found: {id}"))?;
        if let Ok(mut guard) = rec.lock() {
            guard.view_mode = mode;
            guard.updated_at_ms = now_ms();
        }
        let detail = rec.lock().map(|g| g.detail()).map_err(|e| e.to_string())?;
        self.emit("agent_updated", json!(detail.summary));
        Ok(detail)
    }

    pub fn resize(&self, id: &str, cols: u16, rows: u16) -> Result<(), String> {
        let rec = self
            .inner
            .agents
            .read()
            .ok()
            .and_then(|m| m.get(id).cloned())
            .ok_or_else(|| format!("agent not found: {id}"))?;
        let guard = rec.lock().map_err(|e| e.to_string())?;
        if let Some(master) = guard.handle.master.as_ref() {
            let _ = master.resize(PtySize {
                rows: rows.max(10),
                cols: cols.max(30),
                pixel_width: 0,
                pixel_height: 0,
            });
        }
        Ok(())
    }

    pub fn stop(&self, id: &str) -> Result<ManagedAgentDetail, String> {
        let rec = self
            .inner
            .agents
            .read()
            .ok()
            .and_then(|m| m.get(id).cloned())
            .ok_or_else(|| format!("agent not found: {id}"))?;
        if let Ok(mut guard) = rec.lock() {
            if let Some(mut child) = guard.handle.child.take() {
                let _ = child.kill();
            }
            guard.handle.writer = None;
            guard.handle.master = None;
            guard.status = AgentStatus::Stopped;
            guard.approval_prompt = None;
            guard.updated_at_ms = now_ms();
            guard.push_trail("status", "Agent stopped by user");
        }
        let detail = rec.lock().map(|g| g.detail()).map_err(|e| e.to_string())?;
        self.emit("agent_updated", json!(detail.summary));
        Ok(detail)
    }

    pub fn remove(&self, id: &str) -> Result<(), String> {
        let _ = self.stop(id);
        if let Ok(mut agents) = self.inner.agents.write() {
            agents.remove(id);
        }
        if let Ok(mut order) = self.inner.order.write() {
            order.retain(|x| x != id);
        }
        self.emit("agent_removed", json!({ "id": id }));
        Ok(())
    }

    /// Terminates every active PTY agent cleanly (used on desktop app exit).
    pub fn stop_all(&self) -> usize {
        let ids = self
            .inner
            .order
            .read()
            .map(|o| o.clone())
            .unwrap_or_default();
        let mut stopped = 0;
        for id in ids {
            let rec_opt = self
                .inner
                .agents
                .read()
                .ok()
                .and_then(|m| m.get(&id).cloned());
            if let Some(rec) = rec_opt {
                if let Ok(mut guard) = rec.lock() {
                    if let Some(mut child) = guard.handle.child.take() {
                        let _ = child.kill();
                        let _ = child.wait();
                        stopped += 1;
                    }
                    guard.handle.writer = None;
                    guard.handle.master = None;
                    if !matches!(guard.status, AgentStatus::Done) {
                        guard.status = AgentStatus::Stopped;
                    }
                    guard.approval_prompt = None;
                    guard.updated_at_ms = now_ms();
                }
            }
        }
        stopped
    }

    pub fn broadcast(&self, ids: Vec<String>, prompt: &str) -> Vec<Result<String, String>> {
        let targets = if ids.is_empty() {
            self.inner.order.read().map(|o| o.clone()).unwrap_or_default()
        } else {
            ids
        };
        targets
            .into_iter()
            .map(|id| self.send_input(&id, prompt, true).map(|d| d.summary.id))
            .collect()
    }

    pub fn git_diff(&self, id: &str) -> Result<AgentDiffResult, String> {
        let detail = self.get(id).ok_or_else(|| format!("agent not found: {id}"))?;
        let cwd = Path::new(&detail.summary.cwd);
        let status_out = StdCommand::new("git")
            .args(["status", "--short"])
            .current_dir(cwd)
            .output();
        let stat_out = StdCommand::new("git")
            .args(["diff", "--stat"])
            .current_dir(cwd)
            .output();
        let patch_out = StdCommand::new("git")
            .args(["diff", "--no-color"])
            .current_dir(cwd)
            .output();

        let status_lines = status_out
            .ok()
            .filter(|o| o.status.success())
            .map(|o| {
                String::from_utf8_lossy(&o.stdout)
                    .lines()
                    .take(40)
                    .map(|l| l.to_string())
                    .collect()
            })
            .unwrap_or_default();

        let diff_stat = stat_out
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_default();

        let diff_patch = patch_out
            .ok()
            .filter(|o| o.status.success())
            .map(|o| {
                let s = String::from_utf8_lossy(&o.stdout);
                truncate_chars(&s, 8000)
            })
            .unwrap_or_default();

        Ok(AgentDiffResult {
            agent_id: id.to_string(),
            cwd: detail.summary.cwd,
            branch: detail.summary.branch,
            status_lines,
            diff_stat,
            diff_patch,
        })
    }

    pub fn handoff_context(&self, id: &str) -> Option<(String, String, String, String)> {
        let detail = self.get(id)?;
        let recent: Vec<String> = detail
            .masked_turns
            .iter()
            .rev()
            .take(6)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .map(|t| format!("[{}] {}: {}", t.role, t.title, t.content))
            .collect();
        let prompt = if recent.is_empty() {
            format!("Continue agent '{}' task in {}", detail.summary.name, detail.summary.cwd)
        } else {
            format!(
                "Handoff from local agent '{}' ({}):\n{}",
                detail.summary.name,
                detail.summary.branch,
                recent.join("\n")
            )
        };
        Some((detail.summary.cwd, detail.summary.engine, detail.summary.name, prompt))
    }

    /// Spawns 3 real local agents (Orchestrator + 2 real PTY runners) showing Working, Needs You, and Done states.
    pub fn spawn_demo_crew(&self) -> Result<Vec<ManagedAgentSummary>, String> {
        let mut created = Vec::new();

        // 1. Real PTY interactive Permission Gate agent ("Needs You")
        let gate_script = concat!(
            "printf '\\033]7501;status=working;msg=Analyzing auth middleware\\007'; ",
            "printf 'Read(packages/client/ui-agent-preset/src/client/locales.ts)\\n'; ",
            "sleep 0.25; ",
            "printf '\\033]7501;status=needs_you;msg=Allow running smoke build in sandbox? [y/N]\\007'; ",
            "printf 'PermissionGate: Allow running cargo check & sandbox build? [y/N] '; ",
            "read ans; ",
            "if [ \"$ans\" = \"y\" ] || [ \"$ans\" = \"Y\" ]; then ",
            "  printf '\\033]7501;status=working;msg=Running verified checks\\007'; ",
            "  printf 'Cargo build clean — 0 warnings!\\n'; ",
            "  printf '\\033]7501;status=done\\007'; ",
            "else ",
            "  printf 'Cancelled by user.\\n'; ",
            "  printf '\\033]7501;status=idle\\007'; ",
            "fi"
        );
        if let Ok(a1) = self.spawn(SpawnAgentRequest {
            name: Some("UI-Gatekeeper (Claude PTY)".into()),
            engine: "shell".into(),
            cwd: None,
            prompt: Some("Audit liquid glass & Orchestrator preset seams".into()),
            custom_command: Some(gate_script.into()),
            view_mode: Some(AgentViewMode::Harness),
            remote_node: None,
        }) {
            created.push(a1.summary);
        }

        // 2. Real PTY streaming worker ("Working" -> "Done")
        let worker_script = concat!(
            "printf '\\033]7501;status=working;msg=Streaming live build telemetry\\007'; ",
            "printf 'Grep(crates/layer1-adapters/src/bridge.rs)\\n'; ",
            "sleep 0.3; ",
            "printf 'Edit(desktop/web-shell/agent-manager.html)\\n'; ",
            "sleep 0.35; ",
            "printf 'Verified Apple Liquid Glass shader & 6 layouts synced!\\n'; ",
            "printf '\\033]7501;status=done\\007'; ",
            "sleep 45"
        );
        if let Ok(a2) = self.spawn(SpawnAgentRequest {
            name: Some("Glassmith-Worker (Codex PTY)".into()),
            engine: "shell".into(),
            cwd: None,
            prompt: Some("Wire multi-layout studio switcher & live terminal stream".into()),
            custom_command: Some(worker_script.into()),
            view_mode: Some(AgentViewMode::Terminal),
            remote_node: None,
        }) {
            created.push(a2.summary);
        }

        // 3. Internal AI Carnival Orchestrator worker ("Done" / Interactive)
        if let Ok(a3) = self.spawn(SpawnAgentRequest {
            name: Some("Ringmaster-Orchestrator".into()),
            engine: "carnival-orchestrator".into(),
            cwd: None,
            prompt: Some("Orchestrator".into()),
            custom_command: None,
            view_mode: Some(AgentViewMode::Harness),
            remote_node: None,
        }) {
            created.push(a3.summary);
        }

        // Allow initial OSC 7501 bytes from the PTYs to flush cleanly
        std::thread::sleep(Duration::from_millis(450));
        Ok(created)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn osc_7501_and_ansi_stripping_work() {
        let raw = "\x1b[32mRead(src/lib.rs)\x1b[0m\n\x1b]7501;status=needs_you;msg=Approve deploy? [y/N]\x07Permission needed [y/N]";
        let (clean, status) = parse_osc_7501_and_strip_ansi(raw);
        assert!(clean.contains("Read(src/lib.rs)"));
        assert!(!clean.contains("\x1b[32m"));
        assert!(!clean.contains("7501;"));
        let (st, msg) = status.expect("expected OSC 7501 status");
        assert_eq!(st, AgentStatus::NeedsYou);
        assert_eq!(msg.as_deref(), Some("Approve deploy? [y/N]"));
    }

    #[test]
    fn spawns_real_pty_and_internal_orchestrator_and_reads_output() {
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap();
        let manager = AgentManager::new(&repo_root, None);

        // Internal orchestrator agent
        let orch = manager
            .spawn(SpawnAgentRequest {
                name: Some("orch-test".into()),
                engine: "carnival-orchestrator".into(),
                cwd: None,
                prompt: Some("Check Orchestrator routes".into()),
                custom_command: None,
                view_mode: Some(AgentViewMode::Harness),
                remote_node: None,
            })
            .unwrap();
        assert_eq!(orch.summary.name, "orch-test");
        assert_eq!(orch.summary.status, AgentStatus::Done);

        // Real PTY agent with OSC 7501 status + interactive approval
        let pty_agent = manager
            .spawn(SpawnAgentRequest {
                name: Some("pty-test".into()),
                engine: "shell".into(),
                cwd: None,
                prompt: None,
                custom_command: Some(
                    "printf '\\033]7501;status=needs_you;msg=Allow step? [y/N]\\007'; read ans; printf \"GOT:$ans\\n\"; printf '\\033]7501;status=done\\007'"
                        .into(),
                ),
                view_mode: Some(AgentViewMode::Terminal),
                remote_node: None,
            })
            .unwrap();

        let id = pty_agent.summary.id.clone();
        std::thread::sleep(Duration::from_millis(350));
        let mid = manager.get(&id).unwrap();
        assert_eq!(mid.summary.status, AgentStatus::NeedsYou);

        let approved = manager.approve(&id, true).unwrap();
        assert_eq!(approved.summary.view_mode, AgentViewMode::Terminal);

        std::thread::sleep(Duration::from_millis(250));
        let after = manager.get(&id).unwrap();
        assert!(after.raw_log.contains("GOT:y"));
        let _ = manager.stop(&id);
    }

    #[test]
    fn stop_all_terminates_live_ptys_cleanly() {
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap();
        let manager = AgentManager::new(&repo_root, None);
        let a1 = manager
            .spawn(SpawnAgentRequest {
                name: Some("sleeper-1".into()),
                engine: "shell".into(),
                cwd: None,
                prompt: None,
                custom_command: Some("sleep 30".into()),
                view_mode: Some(AgentViewMode::Terminal),
                remote_node: None,
            })
            .unwrap();
        assert_eq!(manager.list().len(), 1);
        let killed = manager.stop_all();
        assert_eq!(killed, 1);
        let post = manager.get(&a1.summary.id).unwrap();
        assert_eq!(post.summary.status, AgentStatus::Stopped);

        // Broadcast to internal orchestrator & verify clean remove()
        let o1 = manager
            .spawn(SpawnAgentRequest {
                name: Some("bcast-1".into()),
                engine: "carnival-orchestrator".into(),
                cwd: None,
                prompt: None,
                custom_command: None,
                view_mode: Some(AgentViewMode::Harness),
                remote_node: None,
            })
            .unwrap();
        let results = manager.broadcast(vec![o1.summary.id.clone()], "Find Bridge");
        assert_eq!(results.len(), 1);
        assert!(results[0].is_ok());

        manager.remove(&a1.summary.id).unwrap();
        manager.remove(&o1.summary.id).unwrap();
        assert!(manager.list().is_empty());
    }
}

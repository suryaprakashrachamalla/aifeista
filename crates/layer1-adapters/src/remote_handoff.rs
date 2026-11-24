use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::agent_manager::{AgentManager, AgentViewMode, RemoteNodeTag, SpawnAgentRequest};

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn default_config_root() -> PathBuf {
    if let Ok(custom) = std::env::var("AI_CARNIVAL_CONFIG_DIR") {
        if !custom.trim().is_empty() {
            return PathBuf::from(custom);
        }
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".config/ai-carnival")
}

/// Safe single-quote escaping for remote POSIX shells.
pub fn shell_quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 4);
    out.push('\'');
    for ch in s.chars() {
        if ch == '\'' {
            out.push_str("'\"'\"'");
        } else {
            out.push(ch);
        }
    }
    out.push('\'');
    out
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RemoteTransport {
    Ssh,
    Tailscale,
    Loopback,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PeerStatus {
    Online,
    Offline,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemotePeer {
    pub id: String,
    pub name: String,
    pub transport: RemoteTransport,
    pub host: String,
    pub user: String,
    pub port: u16,
    pub remote_dir: String,
    pub specs_label: String,
    pub status: PeerStatus,
    pub latency_ms: Option<u64>,
    pub available_engines: Vec<String>,
    pub last_probed_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredPeerHint {
    pub name: String,
    pub transport: RemoteTransport,
    pub host: String,
    pub user: String,
    pub port: u16,
    pub source: String, // "ssh_config" | "tailscale"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandoffRecord {
    pub handoff_id: String,
    pub agent_id: String,
    pub peer_id: String,
    pub peer_name: String,
    pub transport: RemoteTransport,
    pub remote_path: String,
    pub local_cwd: String,
    pub engine: String,
    pub task_prompt: String,
    pub sync_state: String, // "dispatched" | "running" | "ready_to_pull" | "received" | "failed"
    pub received_files: Vec<String>,
    pub received_patch_preview: String,
    pub patch_file: Option<String>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalHostInfo {
    pub username: String,
    pub hostname: String,
    pub tailscale_ip: Option<String>,
    pub share_ssh_cmd: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteStateResponse {
    pub peers: Vec<RemotePeer>,
    pub discovered: Vec<DiscoveredPeerHint>,
    pub handoffs: Vec<HandoffRecord>,
    pub local_host: LocalHostInfo,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AddPeerRequest {
    pub name: String,
    #[serde(default = "default_transport_ssh")]
    pub transport: RemoteTransport,
    pub host: String,
    #[serde(default)]
    pub user: Option<String>,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub remote_dir: Option<String>,
    #[serde(default)]
    pub specs_label: Option<String>,
}

fn default_transport_ssh() -> RemoteTransport {
    RemoteTransport::Ssh
}

#[derive(Debug, Clone, Deserialize)]
pub struct DispatchHandoffRequest {
    pub peer_id: String,
    #[serde(default = "default_engine_claude")]
    pub engine: String,
    #[serde(default)]
    pub agent_name: Option<String>,
    #[serde(default)]
    pub task_prompt: Option<String>,
    #[serde(default)]
    pub local_cwd: Option<String>,
    #[serde(default)]
    pub source_agent_id: Option<String>,
    #[serde(default)]
    pub sync_workspace: Option<bool>,
    #[serde(default)]
    pub view_mode: Option<AgentViewMode>,
}

fn default_engine_claude() -> String {
    "claude-code".to_string()
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReceiveHandoffRequest {
    #[serde(default)]
    pub apply_locally: bool,
}

#[derive(Debug, Clone)]
pub struct RemoteHandoffManager {
    root: PathBuf,
    default_workspace: PathBuf,
    lock: Arc<Mutex<()>>,
}

impl RemoteHandoffManager {
    pub fn default_location(default_workspace: impl Into<PathBuf>) -> Self {
        Self::new(default_config_root(), default_workspace)
    }

    pub fn new(root: impl Into<PathBuf>, default_workspace: impl Into<PathBuf>) -> Self {
        let mgr = Self {
            root: root.into(),
            default_workspace: default_workspace.into(),
            lock: Arc::new(Mutex::new(())),
        };
        let _ = mgr.ensure_seeded();
        mgr
    }

    fn peers_file(&self) -> PathBuf {
        self.root.join("peers.json")
    }

    fn handoffs_file(&self) -> PathBuf {
        self.root.join("handoffs.json")
    }

    fn handoffs_dir(&self) -> PathBuf {
        self.root.join("handoffs")
    }

    fn ensure_seeded(&self) -> Result<(), String> {
        fs::create_dir_all(&self.root).map_err(|e| e.to_string())?;
        fs::create_dir_all(self.handoffs_dir()).map_err(|e| e.to_string())?;

        if !self.peers_file().exists() {
            let demo_loopback = RemotePeer {
                id: "peer-loopback-studio".to_string(),
                name: "Friend's Mac Studio (M4 Ultra · Simulated Peer)".to_string(),
                transport: RemoteTransport::Loopback,
                host: "127.0.0.1".to_string(),
                user: std::env::var("USER").unwrap_or_else(|_| "developer".to_string()),
                port: 22,
                remote_dir: self
                    .root
                    .join("remote-sandbox/friend-mac-studio")
                    .display()
                    .to_string(),
                specs_label: "M4 Ultra · 128 GB RAM · Local Loopback Test Peer".to_string(),
                status: PeerStatus::Online,
                latency_ms: Some(1),
                available_engines: vec![
                    "claude".into(),
                    "codex".into(),
                    "gemini".into(),
                    "git".into(),
                ],
                last_probed_ms: Some(now_ms()),
            };
            self.save_peers_locked(&[demo_loopback])?;
        }
        if !self.handoffs_file().exists() {
            self.save_handoffs_locked(&[])?;
        }
        Ok(())
    }

    fn load_peers_locked(&self) -> Vec<RemotePeer> {
        fs::read_to_string(self.peers_file())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    fn save_peers_locked(&self, peers: &[RemotePeer]) -> Result<(), String> {
        fs::write(
            self.peers_file(),
            serde_json::to_string_pretty(peers).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())
    }

    fn load_handoffs_locked(&self) -> Vec<HandoffRecord> {
        fs::read_to_string(self.handoffs_file())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    fn save_handoffs_locked(&self, handoffs: &[HandoffRecord]) -> Result<(), String> {
        fs::write(
            self.handoffs_file(),
            serde_json::to_string_pretty(handoffs).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())
    }

    pub fn local_host_info(&self) -> LocalHostInfo {
        let username = std::env::var("USER").unwrap_or_else(|_| "user".to_string());
        let hostname = StdCommand::new("hostname")
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "macbook.local".to_string());
        let tailscale_ip = StdCommand::new("tailscale")
            .args(["ip", "-4"])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .and_then(|o| {
                String::from_utf8_lossy(&o.stdout)
                    .lines()
                    .next()
                    .map(|l| l.trim().to_string())
            })
            .filter(|s| !s.is_empty());

        let connect_host = tailscale_ip.as_deref().unwrap_or(&hostname);
        let share_ssh_cmd = format!("ssh {username}@{connect_host}");
        LocalHostInfo {
            username,
            hostname,
            tailscale_ip,
            share_ssh_cmd,
        }
    }

    pub fn discover_local_peers(&self) -> Vec<DiscoveredPeerHint> {
        let mut hints = Vec::new();
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let ssh_cfg = PathBuf::from(&home).join(".ssh/config");
        if let Ok(raw) = fs::read_to_string(ssh_cfg) {
            hints.extend(parse_ssh_config(&raw));
        }
        // Check tailscale peers quickly if tailscale CLI is present
        if let Ok(out) = StdCommand::new("tailscale").args(["status", "--json"]).output() {
            if out.status.success() {
                if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&out.stdout) {
                    if let Some(peer_map) = v.get("Peer").and_then(|p| p.as_object()) {
                        let def_user = std::env::var("USER").unwrap_or_else(|_| "user".into());
                        for (_k, peer) in peer_map.iter().take(12) {
                            let host = peer
                                .get("DNSName")
                                .and_then(|s| s.as_str())
                                .map(|s| s.trim_end_matches('.').to_string())
                                .or_else(|| {
                                    peer.get("TailscaleIPs")
                                        .and_then(|a| a.as_array())
                                        .and_then(|a| a.first())
                                        .and_then(|s| s.as_str())
                                        .map(|s| s.to_string())
                                });
                            let name = peer
                                .get("HostName")
                                .and_then(|s| s.as_str())
                                .unwrap_or("Tailscale Node");
                            if let Some(h) = host {
                                hints.push(DiscoveredPeerHint {
                                    name: name.to_string(),
                                    transport: RemoteTransport::Tailscale,
                                    host: h,
                                    user: def_user.clone(),
                                    port: 22,
                                    source: "tailscale".to_string(),
                                });
                            }
                        }
                    }
                }
            }
        }
        hints
    }

    pub fn state(&self) -> RemoteStateResponse {
        let _guard = self.lock.lock();
        let peers = self.load_peers_locked();
        let handoffs = self.load_handoffs_locked();
        let discovered = self.discover_local_peers();
        RemoteStateResponse {
            peers,
            discovered,
            handoffs,
            local_host: self.local_host_info(),
        }
    }

    pub fn add_peer(&self, req: AddPeerRequest) -> Result<RemotePeer, String> {
        if req.host.trim().is_empty() {
            return Err("Host / Tailscale DNS cannot be empty".to_string());
        }
        let user = req
            .user
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|| std::env::var("USER").unwrap_or_else(|_| "developer".into()));

        let peer = RemotePeer {
            id: format!("peer-{}", &Uuid::new_v4().to_string()[..8]),
            name: if req.name.trim().is_empty() {
                req.host.trim().to_string()
            } else {
                req.name.trim().to_string()
            },
            transport: req.transport,
            host: req.host.trim().to_string(),
            user,
            port: req.port.unwrap_or(22),
            remote_dir: req
                .remote_dir
                .as_deref()
                .filter(|s| !s.trim().is_empty())
                .unwrap_or("~/ai-carnival-handoffs")
                .to_string(),
            specs_label: req
                .specs_label
                .as_deref()
                .filter(|s| !s.trim().is_empty())
                .unwrap_or("Remote Compute Peer")
                .to_string(),
            status: PeerStatus::Unknown,
            latency_ms: None,
            available_engines: Vec::new(),
            last_probed_ms: None,
        };

        let _guard = self.lock.lock().map_err(|e| e.to_string())?;
        let mut peers = self.load_peers_locked();
        peers.push(peer.clone());
        self.save_peers_locked(&peers)?;
        Ok(peer)
    }

    pub fn remove_peer(&self, id: &str) -> Result<(), String> {
        let _guard = self.lock.lock().map_err(|e| e.to_string())?;
        let mut peers = self.load_peers_locked();
        peers.retain(|p| p.id != id);
        self.save_peers_locked(&peers)
    }

    pub fn probe_peer(&self, id: &str) -> Result<RemotePeer, String> {
        let mut peer = {
            let _guard = self.lock.lock().map_err(|e| e.to_string())?;
            let peers = self.load_peers_locked();
            peers
                .into_iter()
                .find(|p| p.id == id)
                .ok_or_else(|| format!("peer not found: {id}"))?
        };

        if matches!(peer.transport, RemoteTransport::Loopback) {
            peer.status = PeerStatus::Online;
            peer.latency_ms = Some(1);
            peer.last_probed_ms = Some(now_ms());
            let _guard = self.lock.lock().map_err(|e| e.to_string())?;
            let mut peers = self.load_peers_locked();
            if let Some(p) = peers.iter_mut().find(|x| x.id == id) {
                *p = peer.clone();
            }
            self.save_peers_locked(&peers)?;
            return Ok(peer);
        }

        let target = format!("{}@{}", peer.user, peer.host);
        let probe_script = "uname -ms; (sysctl -n hw.ncpu 2>/dev/null || nproc 2>/dev/null || echo '?'); for b in claude codex gemini opencode cursor-agent git rsync; do command -v $b >/dev/null 2>&1 && echo ENG:$b; done";
        let start = Instant::now();
        let output = StdCommand::new("ssh")
            .args([
                "-o",
                "BatchMode=yes",
                "-o",
                "ConnectTimeout=4",
                "-p",
                &peer.port.to_string(),
                &target,
                probe_script,
            ])
            .output();
        let elapsed = start.elapsed().as_millis() as u64;

        match output {
            Ok(out) if out.status.success() => {
                let text = String::from_utf8_lossy(&out.stdout);
                let mut uname = String::new();
                let mut cores = String::new();
                let mut engines = Vec::new();
                for (idx, line) in text.lines().enumerate() {
                    let l = line.trim();
                    if let Some(eng) = l.strip_prefix("ENG:") {
                        engines.push(eng.to_string());
                    } else if idx == 0 {
                        uname = l.to_string();
                    } else if idx == 1 {
                        cores = format!("{l} cores");
                    }
                }
                peer.status = PeerStatus::Online;
                peer.latency_ms = Some(elapsed);
                peer.available_engines = engines;
                if !uname.is_empty() {
                    peer.specs_label = if cores.is_empty() {
                        uname
                    } else {
                        format!("{uname} · {cores}")
                    };
                }
            }
            _ => {
                peer.status = PeerStatus::Offline;
                peer.latency_ms = Some(elapsed);
            }
        }
        peer.last_probed_ms = Some(now_ms());

        let _guard = self.lock.lock().map_err(|e| e.to_string())?;
        let mut peers = self.load_peers_locked();
        if let Some(p) = peers.iter_mut().find(|x| x.id == id) {
            *p = peer.clone();
        }
        self.save_peers_locked(&peers)?;
        Ok(peer)
    }

    pub fn dispatch_handoff(
        &self,
        agent_manager: &AgentManager,
        req: DispatchHandoffRequest,
    ) -> Result<HandoffRecord, String> {
        let peer = {
            let _guard = self.lock.lock().map_err(|e| e.to_string())?;
            let peers = self.load_peers_locked();
            peers
                .into_iter()
                .find(|p| p.id == req.peer_id)
                .ok_or_else(|| format!("peer not found: {}", req.peer_id))?
        };

        // Resolve source task/cwd if handing off an existing local agent
        let (src_cwd, src_engine, src_name, src_prompt) = req
            .source_agent_id
            .as_deref()
            .and_then(|aid| agent_manager.handoff_context(aid))
            .map(|(c, e, n, p)| (Some(c), Some(e), Some(n), Some(p)))
            .unwrap_or((None, None, None, None));

        let local_cwd = req
            .local_cwd
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.to_string())
            .or(src_cwd)
            .unwrap_or_else(|| self.default_workspace.display().to_string());

        let engine = if req.engine.trim().is_empty() {
            src_engine.unwrap_or_else(|| "claude-code".to_string())
        } else {
            req.engine.clone()
        };

        let task_prompt = req
            .task_prompt
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.to_string())
            .or(src_prompt)
            .unwrap_or_else(|| "Run remote compute task and prepare diff for sync back.".to_string());

        let handoff_id = format!("hof-{}", &Uuid::new_v4().to_string()[..8]);
        let remote_path = format!("{}/{}", peer.remote_dir.trim_end_matches('/'), handoff_id);
        let callsign = req
            .agent_name
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.to_string())
            .or_else(|| src_name.map(|n| format!("{n} ↗ {}", peer.name)))
            .unwrap_or_else(|| format!("{} @ {}", engine, peer.name.split_whitespace().take(2).collect::<Vec<_>>().join(" ")));

        let remote_tag = RemoteNodeTag {
            peer_id: peer.id.clone(),
            peer_name: peer.name.clone(),
            transport: match peer.transport {
                RemoteTransport::Ssh => "ssh".into(),
                RemoteTransport::Tailscale => "tailscale".into(),
                RemoteTransport::Loopback => "loopback".into(),
            },
            host: peer.host.clone(),
            remote_path: remote_path.clone(),
            handoff_id: handoff_id.clone(),
        };

        let custom_command = if matches!(peer.transport, RemoteTransport::Loopback) {
            // Execute real local loopback workspace + git repo inside remote_path so Send -> Work -> Receive Back works 100% end-to-end offline
            let rdir_q = shell_quote(&remote_path);
            let task_q = shell_quote(&task_prompt);
            let eng_q = shell_quote(&engine);
            Some(format!(
                concat!(
                    "mkdir -p {rdir} && cd {rdir} && ",
                    "git init -q && git config user.email carnival@local && git config user.name 'AI Carnival Peer' && ",
                    "printf 'AI Carnival Remote Compute Task: %s\\nEngine: %s\\n' {task} {eng} > README_HANDOFF.md && ",
                    "git add README_HANDOFF.md && git commit -q -m 'Initial handoff snapshot' && ",
                    "printf '\\033]7501;status=working;msg=Running on peer compute node\\007'; ",
                    "printf 'Read(README_HANDOFF.md)\\n'; ",
                    "sleep 0.25; ",
                    "printf 'Edit(handoff_result.md)\\n'; ",
                    "printf '# Completed by Remote Peer\\n\\n- Task: %s\\n- Engine: %s\\n- Status: Verified & Ready to Receive Back\\n' {task} {eng} > handoff_result.md; ",
                    "sleep 0.25; ",
                    "printf 'Remote Peer completed task! Run Receive Back to pull handoff_result.md.\\n'; ",
                    "printf '\\033]7501;status=done\\007'; ",
                    "sleep 30"
                ),
                rdir = rdir_q,
                task = task_q,
                eng = eng_q,
            ))
        } else {
            // Real SSH/Tailscale compute handoff command: optionally rsyncs workspace, creates remote dir, and launches agent interactively via ssh -tt
            let target = format!("{}@{}", peer.user, peer.host);
            let cli_bin = match engine.as_str() {
                "claude-code" => "claude",
                "codex" => "codex",
                "gemini-cli" => "gemini",
                "opencode" => "opencode",
                "cursor-agent" => "cursor-agent",
                _ => "zsh",
            };
            let do_sync = req.sync_workspace.unwrap_or(true);
            let sync_step = if do_sync {
                format!(
                    "ssh -o BatchMode=yes -o ConnectTimeout=5 -p {} {} {} && rsync -az --delete --exclude '.git' --exclude 'target' --exclude 'node_modules' -e {} {}/ {}:{} && ",
                    peer.port,
                    shell_quote(&target),
                    shell_quote(&format!("mkdir -p {}", shell_quote(&remote_path))),
                    shell_quote(&format!("ssh -p {}", peer.port)),
                    shell_quote(local_cwd.trim_end_matches('/')),
                    shell_quote(&target),
                    shell_quote(&remote_path),
                )
            } else {
                String::new()
            };

            let remote_cmd = if cli_bin == "zsh" {
                format!(
                    "mkdir -p {rp} && cd {rp} && printf 'Remote Compute Handoff Task: %s\\n' {prompt} && exec zsh -i",
                    rp = shell_quote(&remote_path),
                    prompt = shell_quote(&task_prompt),
                )
            } else {
                format!(
                    "mkdir -p {rp} && cd {rp} && {bin} {prompt}",
                    rp = shell_quote(&remote_path),
                    bin = cli_bin,
                    prompt = shell_quote(&task_prompt),
                )
            };

            Some(format!(
                "{sync}ssh -tt -o ServerAliveInterval=15 -p {port} {target} {rcmd}",
                sync = sync_step,
                port = peer.port,
                target = shell_quote(&target),
                rcmd = shell_quote(&remote_cmd),
            ))
        };

        let spawned = agent_manager.spawn(SpawnAgentRequest {
            name: Some(callsign),
            engine: engine.clone(),
            cwd: Some(local_cwd.clone()),
            prompt: Some(task_prompt.clone()),
            custom_command,
            view_mode: Some(req.view_mode.unwrap_or(AgentViewMode::Harness)),
            remote_node: Some(remote_tag),
        })?;

        let now = now_ms();
        let rec = HandoffRecord {
            handoff_id,
            agent_id: spawned.summary.id,
            peer_id: peer.id,
            peer_name: peer.name,
            transport: peer.transport,
            remote_path,
            local_cwd,
            engine,
            task_prompt,
            sync_state: "running".to_string(),
            received_files: Vec::new(),
            received_patch_preview: String::new(),
            patch_file: None,
            created_at_ms: now,
            updated_at_ms: now,
        };

        let _guard = self.lock.lock().map_err(|e| e.to_string())?;
        let mut list = self.load_handoffs_locked();
        list.insert(0, rec.clone());
        self.save_handoffs_locked(&list)?;
        Ok(rec)
    }

    pub fn receive_handoff(
        &self,
        handoff_id: &str,
        req: ReceiveHandoffRequest,
    ) -> Result<HandoffRecord, String> {
        let (mut rec, peer) = {
            let _guard = self.lock.lock().map_err(|e| e.to_string())?;
            let handoffs = self.load_handoffs_locked();
            let rec = handoffs
                .into_iter()
                .find(|h| h.handoff_id == handoff_id)
                .ok_or_else(|| format!("handoff not found: {handoff_id}"))?;
            let peers = self.load_peers_locked();
            let peer = peers.into_iter().find(|p| p.id == rec.peer_id);
            (rec, peer)
        };

        let out_dir = self.handoffs_dir().join(handoff_id);
        fs::create_dir_all(&out_dir).map_err(|e| e.to_string())?;
        let patch_path = out_dir.join("received.patch");

        let (files, patch_text) = if matches!(rec.transport, RemoteTransport::Loopback) {
            let rp = Path::new(&rec.remote_path);
            let mut collected_files = Vec::new();
            let mut patch = String::new();
            if rp.exists() {
                let status = StdCommand::new("git")
                    .args(["status", "--short"])
                    .current_dir(rp)
                    .output();
                if let Ok(o) = status {
                    for line in String::from_utf8_lossy(&o.stdout).lines() {
                        let f = line.get(3..).unwrap_or(line).trim();
                        if !f.is_empty() {
                            collected_files.push(f.to_string());
                        }
                    }
                }
                let diff = StdCommand::new("git")
                    .args(["diff", "--no-color"])
                    .current_dir(rp)
                    .output();
                if let Ok(o) = diff {
                    patch.push_str(&String::from_utf8_lossy(&o.stdout));
                }
                let result_md = rp.join("handoff_result.md");
                if result_md.exists() {
                    if let Ok(body) = fs::read_to_string(&result_md) {
                        if !patch.is_empty() {
                            patch.push('\n');
                        }
                        patch.push_str(&format!("+++ b/handoff_result.md\n{body}"));
                        if !collected_files.contains(&"handoff_result.md".to_string()) {
                            collected_files.push("handoff_result.md".to_string());
                        }
                        if req.apply_locally {
                            let dest = Path::new(&rec.local_cwd).join("handoff_result.md");
                            let _ = fs::write(dest, body);
                        }
                    }
                }
            }
            (collected_files, patch)
        } else if let Some(p) = peer {
            let target = format!("{}@{}", p.user, p.host);
            let remote_cmd = format!(
                "cd {} && (git status --short; echo '---PATCH---'; git diff --no-color HEAD 2>/dev/null || true)",
                shell_quote(&rec.remote_path)
            );
            let out = StdCommand::new("ssh")
                .args([
                    "-o",
                    "BatchMode=yes",
                    "-o",
                    "ConnectTimeout=6",
                    "-p",
                    &p.port.to_string(),
                    &target,
                    &remote_cmd,
                ])
                .output()
                .map_err(|e| format!("failed to contact peer: {e}"))?;

            if !out.status.success() {
                return Err(format!(
                    "SSH pull failed: {}",
                    String::from_utf8_lossy(&out.stderr).trim()
                ));
            }
            let raw = String::from_utf8_lossy(&out.stdout);
            let (status_part, patch_part) = raw.split_once("---PATCH---").unwrap_or((&raw, ""));
            let files: Vec<String> = status_part
                .lines()
                .map(|l| l.get(3..).unwrap_or(l).trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();

            if req.apply_locally {
                let _ = StdCommand::new("rsync")
                    .args([
                        "-az",
                        "--exclude",
                        ".git",
                        "--exclude",
                        "target",
                        "--exclude",
                        "node_modules",
                        "-e",
                        &format!("ssh -p {}", p.port),
                        &format!("{}:{}/", target, rec.remote_path.trim_end_matches('/')),
                        &format!("{}/", rec.local_cwd.trim_end_matches('/')),
                    ])
                    .output();
            }
            (files, patch_part.trim().to_string())
        } else {
            return Err("Peer configuration no longer exists".to_string());
        };

        fs::write(&patch_path, &patch_text).map_err(|e| e.to_string())?;
        rec.sync_state = "received".to_string();
        rec.received_files = files;
        rec.received_patch_preview = patch_text.chars().take(4000).collect();
        rec.patch_file = Some(patch_path.display().to_string());
        rec.updated_at_ms = now_ms();

        let _guard = self.lock.lock().map_err(|e| e.to_string())?;
        let mut list = self.load_handoffs_locked();
        if let Some(existing) = list.iter_mut().find(|h| h.handoff_id == handoff_id) {
            *existing = rec.clone();
        }
        self.save_handoffs_locked(&list)?;
        Ok(rec)
    }
}

/// Parses Host definitions from ~/.ssh/config for 1-click discovery.
pub fn parse_ssh_config(raw: &str) -> Vec<DiscoveredPeerHint> {
    let mut hints = Vec::new();
    let def_user = std::env::var("USER").unwrap_or_else(|_| "user".into());
    let mut cur_alias: Option<String> = None;
    let mut cur_host: Option<String> = None;
    let mut cur_user: Option<String> = None;
    let mut cur_port: u16 = 22;

    let flush = |hints: &mut Vec<DiscoveredPeerHint>,
                 alias: Option<String>,
                 host: Option<String>,
                 user: Option<String>,
                 port: u16,
                 def_user: &str| {
        if let Some(a) = alias {
            if !a.contains('*') && !a.contains('?') {
                hints.push(DiscoveredPeerHint {
                    name: a.clone(),
                    transport: RemoteTransport::Ssh,
                    host: host.unwrap_or(a),
                    user: user.unwrap_or_else(|| def_user.to_string()),
                    port,
                    source: "ssh_config".to_string(),
                });
            }
        }
    };

    for line in raw.lines() {
        let trimmed = line.split('#').next().unwrap_or("").trim();
        if trimmed.is_empty() {
            continue;
        }
        let mut parts = trimmed.split_whitespace();
        let Some(key) = parts.next() else { continue };
        let val = parts.collect::<Vec<_>>().join(" ");
        match key.to_ascii_lowercase().as_str() {
            "host" => {
                flush(&mut hints, cur_alias.take(), cur_host.take(), cur_user.take(), cur_port, &def_user);
                cur_alias = val.split_whitespace().next().map(|s| s.to_string());
                cur_port = 22;
            }
            "hostname" => cur_host = Some(val),
            "user" => cur_user = Some(val),
            "port" => cur_port = val.parse().unwrap_or(22),
            _ => {}
        }
    }
    flush(&mut hints, cur_alias, cur_host, cur_user, cur_port, &def_user);
    hints
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn parses_ssh_config_and_quotes_safely() {
        let cfg = r#"
Host *
  ServerAliveInterval 60

Host friends-mac-mini
  HostName 100.88.12.4
  User alex
  Port 2222
"#;
        let parsed = parse_ssh_config(cfg);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].name, "friends-mac-mini");
        assert_eq!(parsed[0].host, "100.88.12.4");
        assert_eq!(parsed[0].user, "alex");
        assert_eq!(parsed[0].port, 2222);

        assert_eq!(shell_quote("it's clean"), "'it'\"'\"'s clean'");
    }

    #[test]
    fn loopback_handoff_dispatches_live_agent_and_receives_patch_back() {
        let tmp = tempfile::tempdir().unwrap();
        let workspace = tmp.path().join("local-repo");
        fs::create_dir_all(&workspace).unwrap();

        let agent_mgr = AgentManager::new(&workspace, None);
        let remote_mgr = RemoteHandoffManager::new(tmp.path().join("cfg"), &workspace);

        let rec = remote_mgr
            .dispatch_handoff(
                &agent_mgr,
                DispatchHandoffRequest {
                    peer_id: "peer-loopback-studio".to_string(),
                    engine: "claude-code".to_string(),
                    agent_name: Some("Remote-Worker-1".to_string()),
                    task_prompt: Some("Refactor glass shader on remote Mac Studio".to_string()),
                    local_cwd: None,
                    source_agent_id: None,
                    sync_workspace: Some(false),
                    view_mode: Some(AgentViewMode::Harness),
                },
            )
            .unwrap();

        let deadline = std::time::Instant::now() + Duration::from_secs(4);
        while std::time::Instant::now() < deadline {
            if std::path::Path::new(&rec.remote_path)
                .join("handoff_result.md")
                .exists()
            {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        let agent = agent_mgr.get(&rec.agent_id).unwrap();
        assert_eq!(
            agent.summary.remote_node.as_ref().map(|r| r.handoff_id.as_str()),
            Some(rec.handoff_id.as_str())
        );

        let pulled = remote_mgr
            .receive_handoff(&rec.handoff_id, ReceiveHandoffRequest { apply_locally: true })
            .unwrap();
        assert_eq!(pulled.sync_state, "received");
        assert!(pulled.received_files.contains(&"handoff_result.md".to_string()));
        assert!(workspace.join("handoff_result.md").exists());
        let _ = agent_mgr.stop(&rec.agent_id);
    }

    #[test]
    fn add_remove_peer_and_unknown_peer_errors() {
        let tmp = tempfile::tempdir().unwrap();
        let workspace = tmp.path().join("repo");
        fs::create_dir_all(&workspace).unwrap();

        let remote_mgr = RemoteHandoffManager::new(tmp.path().join("cfg"), &workspace);
        let added = remote_mgr
            .add_peer(AddPeerRequest {
                name: "Test GPU Rig".into(),
                transport: RemoteTransport::Ssh,
                host: "100.99.99.99".into(),
                user: Some("builder".into()),
                port: Some(2222),
                remote_dir: Some("~/code".into()),
                specs_label: Some("M3 Ultra".into()),
            })
            .unwrap();
        assert!(remote_mgr.state().peers.iter().any(|p| p.id == added.id));

        remote_mgr.remove_peer(&added.id).unwrap();
        assert!(!remote_mgr.state().peers.iter().any(|p| p.id == added.id));

        let agent_mgr = AgentManager::new(&workspace, None);
        let err = remote_mgr
            .dispatch_handoff(
                &agent_mgr,
                DispatchHandoffRequest {
                    peer_id: "nonexistent-peer".into(),
                    engine: "claude".into(),
                    agent_name: None,
                    task_prompt: None,
                    local_cwd: None,
                    source_agent_id: None,
                    sync_workspace: Some(false),
                    view_mode: None,
                },
            )
            .unwrap_err();
        assert!(!err.is_empty());
    }
}

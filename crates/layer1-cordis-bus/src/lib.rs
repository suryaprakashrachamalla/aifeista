use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant};

use layer0_contracts::{CordisRpcRequest, CordisRpcResponse, CordisState};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use thiserror::Error;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum CordisError {
    #[error("failed to spawn cordis sidecar '{0}': {1}")]
    SpawnFailed(String, String),
    #[error("cordis RPC '{0}' timed out after {1:?}")]
    Timeout(String, Duration),
    #[error("cordis sidecar I/O error: {0}")]
    Io(String),
    #[error("cordis RPC returned error ({0}): {1}")]
    Rpc(i32, String),
    #[error("cordis invalid JSON response: {0}")]
    InvalidJson(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CordisConfig {
    pub node_bin: String,
    pub script_path: PathBuf,
    pub repo_root: PathBuf,
    pub request_timeout: Duration,
    pub idle_timeout: Duration,
    pub max_retries: usize,
}

impl CordisConfig {
    pub fn default_for_repo(repo_root: &Path) -> Self {
        Self {
            node_bin: "node".to_string(),
            script_path: repo_root.join("sidecars/cordis-bridge/index.mjs"),
            repo_root: repo_root.to_path_buf(),
            request_timeout: Duration::from_secs(5),
            idle_timeout: Duration::from_secs(60),
            max_retries: 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CordisStatus {
    pub state: CordisState,
    pub pid: Option<u32>,
    pub request_count: u64,
    pub restart_count: u64,
    pub last_error: Option<String>,
}

struct RunningChild {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    pid: u32,
}

struct BusInner {
    state: CordisState,
    running: Option<RunningChild>,
    request_count: u64,
    restart_count: u64,
    last_active: Option<Instant>,
    last_error: Option<String>,
    reaper_started: bool,
}

/// Lazy Cordis Node.js Sidecar Supervisor:
/// - Starts in `CordisState::Stopped` (0 MB Node RAM at idle)
/// - Lazily spawns `sidecars/cordis-bridge/index.mjs` on first request
/// - Recovers automatically from sidecar crashes or timeouts
/// - Shuts down automatically after `idle_timeout`
#[derive(Clone)]
pub struct CordisBus {
    config: CordisConfig,
    inner: Arc<Mutex<BusInner>>,
}

impl CordisBus {
    pub fn new(config: CordisConfig) -> Self {
        Self {
            config,
            inner: Arc::new(Mutex::new(BusInner {
                state: CordisState::Stopped,
                running: None,
                request_count: 0,
                restart_count: 0,
                last_active: None,
                last_error: None,
                reaper_started: false,
            })),
        }
    }

    pub async fn status(&self) -> CordisStatus {
        let mut guard = self.inner.lock().await;
        Self::refresh_child_liveness(&mut guard);
        CordisStatus {
            state: guard.state,
            pid: guard.running.as_ref().map(|r| r.pid),
            request_count: guard.request_count,
            restart_count: guard.restart_count,
            last_error: guard.last_error.clone(),
        }
    }

    /// Sends a typed JSON-RPC request to the Cordis sidecar, lazily spawning
    /// the Node.js process on demand and retrying once on transient failure.
    pub async fn request(&self, method: &str, params: Value) -> Result<Value, CordisError> {
        self.ensure_reaper_started().await;

        let mut last_err = None;
        for attempt in 0..=self.config.max_retries {
            match self.request_once(method, params.clone()).await {
                Ok(val) => return Ok(val),
                Err(err) => {
                    tracing::warn!(attempt, method, error = %err, "cordis rpc attempt failed");
                    last_err = Some(err);
                    self.force_reset_degraded().await;
                }
            }
        }
        Err(last_err.unwrap_or_else(|| CordisError::Io("unknown cordis failure".into())))
    }

    /// Explicitly shuts down the sidecar process if running (returns to 0 MB idle RAM).
    pub async fn shutdown(&self) -> Result<(), CordisError> {
        let mut guard = self.inner.lock().await;
        Self::stop_child_locked(&mut guard, self.config.request_timeout).await;
        Ok(())
    }

    async fn ensure_reaper_started(&self) {
        let should_spawn = {
            let mut guard = self.inner.lock().await;
            if guard.reaper_started {
                false
            } else {
                guard.reaper_started = true;
                true
            }
        };
        if !should_spawn {
            return;
        }

        let bus = self.clone();
        let idle_timeout = self.config.idle_timeout;
        let check_interval = (idle_timeout / 4).max(Duration::from_millis(25));
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(check_interval).await;
                let should_reap = {
                    let mut guard = bus.inner.lock().await;
                    Self::refresh_child_liveness(&mut guard);
                    if guard.running.is_some() {
                        guard
                            .last_active
                            .map(|t| t.elapsed() >= idle_timeout)
                            .unwrap_or(false)
                    } else {
                        false
                    }
                };
                if should_reap {
                    let _ = bus.shutdown().await;
                }
            }
        });
    }

    async fn request_once(&self, method: &str, params: Value) -> Result<Value, CordisError> {
        let mut guard = self.inner.lock().await;
        Self::refresh_child_liveness(&mut guard);

        if guard.running.is_none() {
            Self::spawn_locked(&self.config, &mut guard).await?;
        }

        let req_id = Uuid::new_v4().to_string();
        let req = CordisRpcRequest::new(&req_id, method, params);
        let mut line = serde_json::to_string(&req)
            .map_err(|e| CordisError::InvalidJson(e.to_string()))?;
        line.push('\n');

        let timeout = self.config.request_timeout;
        let running = guard
            .running
            .as_mut()
            .ok_or_else(|| CordisError::Io("sidecar not running".into()))?;

        let io_fut = async {
            running
                .stdin
                .write_all(line.as_bytes())
                .await
                .map_err(|e| CordisError::Io(e.to_string()))?;
            running
                .stdin
                .flush()
                .await
                .map_err(|e| CordisError::Io(e.to_string()))?;

            let mut resp_line = String::new();
            let read = running
                .stdout
                .read_line(&mut resp_line)
                .await
                .map_err(|e| CordisError::Io(e.to_string()))?;
            if read == 0 {
                return Err(CordisError::Io("cordis sidecar closed stdout unexpectedly".into()));
            }
            let parsed: CordisRpcResponse = serde_json::from_str(resp_line.trim())
                .map_err(|e| CordisError::InvalidJson(format!("{e}: {resp_line}")))?;
            if let Some(err) = parsed.error {
                return Err(CordisError::Rpc(err.code, err.message));
            }
            Ok(parsed.result.unwrap_or(Value::Null))
        };

        let result = match tokio::time::timeout(timeout, io_fut).await {
            Ok(res) => res,
            Err(_) => Err(CordisError::Timeout(method.to_string(), timeout)),
        };

        match result {
            Ok(val) => {
                guard.request_count += 1;
                guard.last_active = Some(Instant::now());
                guard.state = CordisState::Running;
                guard.last_error = None;
                Ok(val)
            }
            Err(err) => {
                guard.last_error = Some(err.to_string());
                Err(err)
            }
        }
    }

    async fn spawn_locked(config: &CordisConfig, guard: &mut BusInner) -> Result<(), CordisError> {
        let was_degraded = guard.state == CordisState::Degraded || guard.request_count > 0;
        guard.state = CordisState::Starting;

        let resolved_node = resolve_node_bin(&config.node_bin);
        let mut path_parts = vec![
            "/opt/homebrew/bin".to_string(),
            "/usr/local/bin".to_string(),
        ];
        if let Ok(existing) = std::env::var("PATH") {
            if !existing.is_empty() {
                path_parts.push(existing);
            }
        } else {
            path_parts.push("/usr/bin:/bin:/usr/sbin:/sbin".to_string());
        }

        let mut cmd = Command::new(&resolved_node);
        cmd.arg(&config.script_path)
            .arg("--repo-root")
            .arg(&config.repo_root)
            .env("PATH", path_parts.join(":"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());

        let mut child = cmd
            .spawn()
            .map_err(|e| CordisError::SpawnFailed(resolved_node, e.to_string()))?;
        let pid = child.id().unwrap_or(0);
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| CordisError::Io("missing stdin pipe".into()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| CordisError::Io("missing stdout pipe".into()))?;

        if was_degraded {
            guard.restart_count += 1;
        }
        guard.running = Some(RunningChild {
            child,
            stdin,
            stdout: BufReader::new(stdout),
            pid,
        });
        guard.state = CordisState::Running;
        guard.last_active = Some(Instant::now());
        Ok(())
    }

    fn refresh_child_liveness(guard: &mut BusInner) {
        if let Some(running) = guard.running.as_mut() {
            match running.child.try_wait() {
                Ok(Some(status)) => {
                    guard.running = None;
                    guard.state = CordisState::Degraded;
                    guard.last_error = Some(format!("sidecar exited unexpectedly: {status}"));
                }
                Ok(None) => {}
                Err(e) => {
                    guard.running = None;
                    guard.state = CordisState::Degraded;
                    guard.last_error = Some(e.to_string());
                }
            }
        }
    }

    async fn force_reset_degraded(&self) {
        let mut guard = self.inner.lock().await;
        if let Some(mut running) = guard.running.take() {
            let _ = running.child.kill().await;
            let _ = running.child.wait().await;
        }
        guard.state = CordisState::Degraded;
    }

    async fn stop_child_locked(guard: &mut BusInner, timeout: Duration) {
        if let Some(mut running) = guard.running.take() {
            let req = CordisRpcRequest::new("shutdown", "shutdown", json!({}));
            if let Ok(mut line) = serde_json::to_string(&req) {
                line.push('\n');
                let _ = running.stdin.write_all(line.as_bytes()).await;
                let _ = running.stdin.flush().await;
            }
            let wait_res = tokio::time::timeout(
                timeout.min(Duration::from_millis(500)),
                running.child.wait(),
            )
            .await;
            if wait_res.is_err() {
                let _ = running.child.kill().await;
                let _ = running.child.wait().await;
            }
        }
        guard.state = CordisState::Stopped;
        guard.last_active = None;
    }
}

fn resolve_node_bin(configured: &str) -> String {
    if configured == "node" {
        for candidate in [
            "/opt/homebrew/bin/node",
            "/usr/local/bin/node",
            "/usr/bin/node",
        ] {
            if Path::new(candidate).exists() {
                return candidate.to_string();
            }
        }
    }
    configured.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap()
    }

    #[tokio::test]
    async fn cordis_lazy_spawn_discover_crash_recovery_and_idle_shutdown() {
        let repo_root = test_repo_root();
        let mut cfg = CordisConfig::default_for_repo(&repo_root);
        cfg.idle_timeout = Duration::from_millis(350);

        let bus = CordisBus::new(cfg);

        // 1. Starts Stopped (0 MB Node RAM)
        let st0 = bus.status().await;
        assert_eq!(st0.state, CordisState::Stopped);
        assert_eq!(st0.pid, None);

        // 2. First call lazily spawns Node and discovers all 3 harnesses
        let discovered = bus.request("discover_harnesses", json!({})).await.unwrap();
        let st1 = bus.status().await;
        assert_eq!(st1.state, CordisState::Running);
        let first_pid = st1.pid.expect("pid should be set");
        let harnesses = discovered["harnesses"].as_array().unwrap();
        assert_eq!(harnesses.len(), 3);
        assert!(harnesses.iter().all(|h| h["present"] == true));

        // 3. Simulate crash by killing the Node PID externally, verify automatic recovery
        let _ = Command::new("kill")
            .arg("-9")
            .arg(first_pid.to_string())
            .output()
            .await;
        tokio::time::sleep(Duration::from_millis(80)).await;

        let ping = bus.request("ping", json!({})).await.unwrap();
        assert_eq!(ping["ok"], true);
        let st2 = bus.status().await;
        assert_eq!(st2.state, CordisState::Running);
        assert_ne!(st2.pid, Some(first_pid));
        assert!(st2.restart_count >= 1);

        // 4. Wait past idle_timeout and confirm sidecar shuts down back to Stopped
        tokio::time::sleep(Duration::from_millis(550)).await;
        let st3 = bus.status().await;
        assert_eq!(st3.state, CordisState::Stopped);
        assert_eq!(st3.pid, None);
    }
}

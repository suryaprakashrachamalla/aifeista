use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::{Arc, Mutex};

use layer0_contracts::{now_ms, CapabilityEnvelope, CuaActionRequest};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SandboxError {
    #[error("native OS sandbox unavailable (fail-closed): {0}")]
    Unavailable(String),
    #[error("capability denied: {0}")]
    Denied(String),
    #[error("path canonicalization failed for '{0}': {1}")]
    InvalidPath(String, String),
    #[error("process execution failed: {0}")]
    Exec(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrivacyScope {
    VisionScreenCapture,
    AudioMicrophone,
}

#[derive(Debug, Clone)]
struct GrantRecord {
    scope: PrivacyScope,
    expires_at_ms: i64,
}

/// Enforces zero background surveillance: Vision and Audio capture fail-closed
/// unless an explicit, short-lived, user-triggered grant token is active.
#[derive(Clone, Default)]
pub struct PrivacyGuard {
    grants: Arc<Mutex<HashMap<String, GrantRecord>>>,
}

impl PrivacyGuard {
    pub fn new() -> Self {
        Self::default()
    }

    /// Issues an explicit on-demand capture token valid for `ttl_ms`.
    pub fn issue_explicit_grant(&self, scope: PrivacyScope, ttl_ms: i64) -> String {
        let token = format!(
            "grant-{}-{}",
            match scope {
                PrivacyScope::VisionScreenCapture => "vision",
                PrivacyScope::AudioMicrophone => "audio",
            },
            now_ms()
        );
        if let Ok(mut map) = self.grants.lock() {
            map.insert(
                token.clone(),
                GrantRecord {
                    scope,
                    expires_at_ms: now_ms() + ttl_ms.max(1),
                },
            );
        }
        token
    }

    pub fn revoke(&self, token: &str) {
        if let Ok(mut map) = self.grants.lock() {
            map.remove(token);
        }
    }

    pub fn verify(&self, token: Option<&str>, required: PrivacyScope) -> Result<(), SandboxError> {
        let token = token.ok_or_else(|| {
            SandboxError::Denied(format!("missing explicit on-demand token for {required:?}"))
        })?;
        let mut map = self
            .grants
            .lock()
            .map_err(|_| SandboxError::Denied("privacy guard lock poisoned".into()))?;
        let now = now_ms();
        map.retain(|_, rec| rec.expires_at_ms >= now);
        match map.get(token) {
            Some(rec) if rec.scope == required && rec.expires_at_ms >= now => Ok(()),
            _ => Err(SandboxError::Denied(format!(
                "invalid or expired on-demand token for {required:?}"
            ))),
        }
    }
}

/// Validates CUA actions against a CapabilityEnvelope before allowing OS input.
pub fn validate_cua_action(
    envelope: &CapabilityEnvelope,
    req: &CuaActionRequest,
) -> Result<(), SandboxError> {
    if envelope.cua_apps.is_empty() {
        return Err(SandboxError::Denied(format!(
            "subject '{}' has no CUA app permissions",
            envelope.subject
        )));
    }
    let allowed = envelope
        .cua_apps
        .iter()
        .any(|app| app == "*" || app.eq_ignore_ascii_case(&req.target_app));
    if !allowed {
        return Err(SandboxError::Denied(format!(
            "target app '{}' is outside CapabilityEnvelope {:?}",
            req.target_app, envelope.cua_apps
        )));
    }
    Ok(())
}

/// Canonicalizes a path (or its nearest existing parent) so macOS `/var` -> `/private/var`
/// and `/tmp` -> `/private/tmp` symlinks resolve accurately for Seatbelt rules.
pub fn canonicalize_for_sandbox(path: &Path) -> Result<PathBuf, SandboxError> {
    if path.exists() {
        return std::fs::canonicalize(path)
            .map_err(|e| SandboxError::InvalidPath(path.display().to_string(), e.to_string()));
    }
    if let (Some(parent), Some(name)) = (path.parent(), path.file_name()) {
        if parent.exists() {
            let canon_parent = std::fs::canonicalize(parent).map_err(|e| {
                SandboxError::InvalidPath(parent.display().to_string(), e.to_string())
            })?;
            return Ok(canon_parent.join(name));
        }
    }
    Err(SandboxError::InvalidPath(
        path.display().to_string(),
        "path and parent directory do not exist".into(),
    ))
}

#[cfg(target_os = "macos")]
pub fn build_seatbelt_profile(envelope: &CapabilityEnvelope) -> Result<String, SandboxError> {
    let mut profile = String::from(
        "(version 1)\n\
         (deny default)\n\
         (allow process-exec)\n\
         (allow process-fork)\n\
         (allow sysctl-read)\n\
         (allow mach-lookup)\n\
         (allow file-read*\n\
           (subpath \"/usr\")\n\
           (subpath \"/bin\")\n\
           (subpath \"/sbin\")\n\
           (subpath \"/System\")\n\
           (subpath \"/Library\")\n\
           (subpath \"/dev\")\n\
           (subpath \"/private/var/db\")\n\
           (subpath \"/private/etc\")\n\
           (literal \"/\")\n\
           (literal \"/private\")\n\
           (literal \"/var\")\n\
           (literal \"/tmp\"))\n",
    );

    if !envelope.fs_read.is_empty() {
        profile.push_str("(allow file-read*\n");
        for p in &envelope.fs_read {
            let canon = canonicalize_for_sandbox(Path::new(p))?;
            let escaped = escape_sbpl(&canon.to_string_lossy());
            profile.push_str(&format!("  (subpath \"{escaped}\")\n"));
        }
        profile.push_str(")\n");
    }

    if !envelope.fs_write.is_empty() {
        profile.push_str("(allow file-read* file-write*\n");
        for p in &envelope.fs_write {
            let canon = canonicalize_for_sandbox(Path::new(p))?;
            let escaped = escape_sbpl(&canon.to_string_lossy());
            profile.push_str(&format!("  (subpath \"{escaped}\")\n"));
        }
        profile.push_str(")\n");
    }

    if envelope.network_allow {
        profile.push_str("(allow network*)\n");
    }

    Ok(profile)
}

#[cfg(target_os = "macos")]
fn escape_sbpl(raw: &str) -> String {
    raw.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Fail-closed command execution under macOS Seatbelt (`/usr/bin/sandbox-exec`).
#[cfg(target_os = "macos")]
pub fn run_sandboxed_command(
    envelope: &CapabilityEnvelope,
    program: &str,
    args: &[&str],
    cwd: Option<&Path>,
) -> Result<Output, SandboxError> {
    let seatbelt_bin = Path::new("/usr/bin/sandbox-exec");
    if !seatbelt_bin.exists() {
        return Err(SandboxError::Unavailable(
            "/usr/bin/sandbox-exec not found".into(),
        ));
    }

    let profile = build_seatbelt_profile(envelope)?;
    let mut cmd = Command::new(seatbelt_bin);
    cmd.arg("-p").arg(profile).arg(program).args(args);
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }

    cmd.output()
        .map_err(|e| SandboxError::Exec(e.to_string()))
}

/// Cross-platform stub for Windows AppContainer/ACL & Linux Landlock parity.
#[cfg(not(target_os = "macos"))]
pub fn run_sandboxed_command(
    _envelope: &CapabilityEnvelope,
    _program: &str,
    _args: &[&str],
    _cwd: Option<&Path>,
) -> Result<Output, SandboxError> {
    Err(SandboxError::Unavailable(
        "native OS sandbox backend not yet enabled for this target OS (fail-closed)".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use layer0_contracts::CuaActionKind;
    use serde_json::json;
    use tempfile::TempDir;

    #[test]
    fn privacy_guard_blocks_background_capture_without_explicit_grant() {
        let guard = PrivacyGuard::new();
        assert!(guard
            .verify(None, PrivacyScope::VisionScreenCapture)
            .is_err());

        let token = guard.issue_explicit_grant(PrivacyScope::VisionScreenCapture, 5_000);
        assert!(guard
            .verify(Some(&token), PrivacyScope::VisionScreenCapture)
            .is_ok());
        assert!(guard
            .verify(Some(&token), PrivacyScope::AudioMicrophone)
            .is_err());

        guard.revoke(&token);
        assert!(guard
            .verify(Some(&token), PrivacyScope::VisionScreenCapture)
            .is_err());
    }

    #[test]
    fn cua_validation_enforces_per_app_envelope() {
        let env = CapabilityEnvelope {
            subject: "companion".into(),
            fs_read: vec![],
            fs_write: vec![],
            network_allow: false,
            cua_apps: vec!["Xcode".into()],
            allow_vision: true,
            allow_audio: false,
        };
        let allowed_req = CuaActionRequest {
            action_id: "1".into(),
            target_app: "Xcode".into(),
            action: CuaActionKind::Click,
            params: json!({"x": 10, "y": 20}),
            background: false,
        };
        let blocked_req = CuaActionRequest {
            action_id: "2".into(),
            target_app: "Terminal".into(),
            action: CuaActionKind::Click,
            params: json!({"x": 10, "y": 20}),
            background: false,
        };
        assert!(validate_cua_action(&env, &allowed_req).is_ok());
        assert!(validate_cua_action(&env, &blocked_req).is_err());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_seatbelt_allows_inside_workspace_and_denies_outside_write() {
        let allowed_dir = TempDir::new().unwrap();
        let forbidden_dir = TempDir::new().unwrap();
        let allowed_file = allowed_dir.path().join("inside.txt");
        let forbidden_file = forbidden_dir.path().join("outside.txt");

        let env = CapabilityEnvelope {
            subject: "test-seatbelt".into(),
            fs_read: vec![allowed_dir.path().display().to_string()],
            fs_write: vec![allowed_dir.path().display().to_string()],
            network_allow: false,
            cua_apps: vec![],
            allow_vision: false,
            allow_audio: false,
        };

        let ok_out = run_sandboxed_command(
            &env,
            "/usr/bin/touch",
            &[allowed_file.to_str().unwrap()],
            Some(allowed_dir.path()),
        )
        .unwrap();
        assert!(
            ok_out.status.success(),
            "expected touch inside allowed_dir to succeed: stderr={}",
            String::from_utf8_lossy(&ok_out.stderr)
        );
        assert!(allowed_file.exists());

        let denied_out = run_sandboxed_command(
            &env,
            "/usr/bin/touch",
            &[forbidden_file.to_str().unwrap()],
            Some(allowed_dir.path()),
        )
        .unwrap();
        assert!(
            !denied_out.status.success(),
            "expected Seatbelt to deny write outside allowed_dir"
        );
        assert!(!forbidden_file.exists());
    }
}

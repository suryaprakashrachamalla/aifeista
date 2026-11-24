use std::path::{Component, Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CoreGuardError {
    #[error("forbidden write to immutable Layer-0 target: {0}")]
    ImmutableTarget(String),
}

/// Enforces the Layer-0 vs Layer-1 boundary: AI agents, plugins, and self-rebuilding
/// workflows may never overwrite immutable Layer-0 crates, contracts, or core binaries.
#[derive(Debug, Clone)]
pub struct ImmutableCoreGuard {
    protected_roots: Vec<PathBuf>,
}

impl ImmutableCoreGuard {
    pub fn new(protected_roots: Vec<PathBuf>) -> Self {
        Self { protected_roots }
    }

    pub fn for_workspace(unified_root: &Path) -> Self {
        let roots = vec![
            unified_root.join("Cargo.toml"),
            unified_root.join("Cargo.lock"),
            unified_root.join("crates/layer0-contracts"),
            unified_root.join("crates/layer0-core"),
            unified_root.join("crates/layer0-store"),
            unified_root.join("crates/layer0-watchdog"),
            unified_root.join("crates/layer0-sandbox"),
            unified_root.join("crates/carnival-daemon"),
        ];
        Self::new(roots)
    }

    /// Rejects any candidate write path targeting Layer-0 source or configuration.
    pub fn check_layer1_write(&self, candidate: &Path) -> Result<(), CoreGuardError> {
        let norm_candidate = normalize_lexical(candidate);

        // Quick substring guard for relative paths containing layer0-* crate segments
        for comp in norm_candidate.components() {
            if let Component::Normal(os_str) = comp {
                let s = os_str.to_string_lossy();
                if s.starts_with("layer0-") || s == "carnival-daemon" {
                    return Err(CoreGuardError::ImmutableTarget(
                        norm_candidate.display().to_string(),
                    ));
                }
            }
        }

        for protected in &self.protected_roots {
            let norm_protected = normalize_lexical(protected);
            if norm_candidate == norm_protected || norm_candidate.starts_with(&norm_protected) {
                return Err(CoreGuardError::ImmutableTarget(
                    norm_candidate.display().to_string(),
                ));
            }
        }

        Ok(())
    }
}

fn normalize_lexical(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_layer0_writes_and_allows_layer1() {
        let root = PathBuf::from("/tmp/carnival");
        let guard = ImmutableCoreGuard::for_workspace(&root);

        assert!(guard
            .check_layer1_write(&root.join("crates/layer0-watchdog/src/lib.rs"))
            .is_err());
        assert!(guard
            .check_layer1_write(&root.join("crates/layer1-router/../layer0-core/src/lib.rs"))
            .is_err());
        assert!(guard
            .check_layer1_write(&root.join("Cargo.toml"))
            .is_err());

        assert!(guard
            .check_layer1_write(&root.join("crates/layer1-router/src/custom_opt.rs"))
            .is_ok());
    }
}

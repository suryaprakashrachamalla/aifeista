# Carnival Architecture Blueprint

> **Target Platform:** macOS `arm64` (Apple Silicon) with Tauri v2 desktop shell, Rust runtime crates, Node.js workspace runtime, and native Swift voice/spatial companion.

---

## 1. Three-Layer System Design

```mermaid
flowchart TD
    subgraph L2["Layer 2: Desktop Experience (Tauri v2 · AI Carnival.app)"]
        L2S["Jet Black Smoked Glass Shell (:3080 Workspace + Preferences)"]
        L2M["Agent Manager (⌘⇧A) · 9 Interactive Studio Layouts + Dual UI/Terminal"]
        L2K["Spotlight FlashGrep (⌘K)"]
        L2O["Spatial Companion Overlay ([POINT:x,y:label])"]
    end

    subgraph L1["Layer 1: Mutable Orchestration & Service Plane"]
        L1B["HTTP/SSE Bridge (:8787 Companion / :32123 Control Bridge)"]
        L1A["AgentManager (Portable PTY + OSC 7501 Telemetry)"]
        L1H["RemoteHandoff (SSH & Tailscale Compute Routing)"]
        L1R["StudioRecipes (~/.config/ai-carnival/layouts + Shape UI + Rollback)"]
        L1T["SkillTree Router & Prefix-Cache Optimization"]
        L1C["CordisBus (Lazy Node Sidecar JSON-RPC -> sidecars/cordis-bridge)"]
    end

    subgraph L0["Layer 0: Immutable Rust Core & Watchdog"]
        CORE["layer0-core (SeamRegistry, EffectGuard, WorkspaceIndex, ImmutableCoreGuard)"]
        STORE["layer0-store (SQLite WAL Sessions, Fork/Replay, Snapshots)"]
        SB["layer0-sandbox (macOS Seatbelt sandbox-exec, CapabilityEnvelope, PrivacyGuard)"]
        WD["layer0-watchdog (6-Stage Hot-Swap + <1s Auto-Rollback)"]
    end

    L2S --> L1B
    L2M --> L1B
    L2K --> CORE
    L2O --> L1B
    L1B --> L1A
    L1B --> L1H
    L1B --> L1R
    L1B --> L1T
    L1T --> L1C
    L1B --> SB
    L1B --> STORE
    WD --> CORE
    WD --> STORE
```

---

## 2. Repository Modules

| Path | Responsibility |
|---|---|
| `crates/layer0-*` | Immutable core guards, resident `WorkspaceIndex` (`FlashGrep`), SQLite WAL session store, macOS Seatbelt sandbox, and `<1s` Watchdog rollback. |
| `crates/layer1-*` | Skill-tree & prefix-cache router, lazy Cordis sidecar bus, Axum HTTP/SSE bridge (`:8787` & `:32123`), PTY Agent Manager, Studio Recipes, and SSH/Tailscale Remote Handoff. |
| `crates/carnival-daemon/` | Headless daemon binary supporting `--self-check` and background bridge serving. |
| `desktop/src-tauri/` & `desktop/web-shell/` | Native macOS Tauri v2 shell (`AI Carnival.app`), Smoked Glass UI, Spotlight `⌘K`, Spatial Overlay, and Agent Manager `⌘⇧A`. |
| `runtime/` | Multi-model web workspace server (`dsh web`), Cordis microkernel, PTC runtime, and Orchestrator preset. |
| `companion/` | Native macOS Swift screen-and-voice companion (`Ctrl + Option` push-to-talk, on-device Apple Speech + Siri neural TTS, ScreenCaptureKit). |
| `sidecars/cordis-bridge/` | Lazy on-demand Node.js JSON-RPC bridge between Rust services and `runtime/` + `companion/`. |
| `contracts/` | Cross-component JSON Schema and TypeScript contract definitions. |

---

## 3. Security & Lifecycle Guarantees

- **Immutable Core Protection**: `ImmutableCoreGuard` blocks Layer-1 writes targeting `crates/layer0-*`, `Cargo.toml`, `Cargo.lock`, or `crates/carnival-daemon`.
- **macOS Seatbelt (`sandbox-exec`) & PrivacyGuard**: `layer0-sandbox` enforces SBPL v1 write restrictions outside authorized directories, and `PrivacyGuard` blocks background screen/audio capture without an explicit active grant.
- **Process Hygiene**: `AI Carnival.app` enforces single-instance window focus, hides to the Dock on red-button close, tears down active PTY workers via `AgentManager::stop_all()`, and terminates its child process group cleanly on quit or `SIGTERM`.

---

## 4. Local Runbook

```bash
# Run full Rust workspace test suite
export PATH="$HOME/.cargo/bin:$PATH"
cargo check --workspace
cargo test --workspace

# Run daemon self-check
cargo run -p carnival-daemon -- --workspace . --self-check

# Build & launch macOS desktop app
cd desktop/src-tauri
cargo run
```

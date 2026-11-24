use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use layer0_contracts::CapabilityEnvelope;
use layer0_core::{default_workspace_roots, SeamRegistry, WorkspaceIndex};
use layer0_sandbox::PrivacyGuard;
use layer0_store::EventStore;
use layer1_adapters::{
    spawn_bridge_servers, AgentManager, AppState, OpenBitFunAdapter, OverlayState,
    RemoteHandoffManager, StudioRecipes,
};
use layer1_cordis_bus::{CordisBus, CordisConfig};
use layer1_router::OptimizationMiddleware;
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

async fn http_request(
    addr: SocketAddr,
    method: &str,
    path: &str,
    body: Option<&str>,
) -> (u16, String) {
    let mut stream = TcpStream::connect(addr).await.unwrap();
    let payload = body.unwrap_or("");
    let req = if body.is_some() {
        format!(
            "{method} {path} HTTP/1.1\r\nHost: {addr}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
            payload.len()
        )
    } else {
        format!("{method} {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n")
    };
    stream.write_all(req.as_bytes()).await.unwrap();
    let mut resp = String::new();
    stream.read_to_string(&mut resp).await.unwrap();
    let (head, resp_body) = resp.split_once("\r\n\r\n").unwrap_or((&resp, ""));
    let status = head
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(0);
    (status, resp_body.to_string())
}

#[tokio::test]
async fn clicky_8787_and_bridge_32123_end_to_end() {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();

    let registry = SeamRegistry::new(repo_root.display().to_string());
    let store = EventStore::open_in_memory().unwrap();
    let index = WorkspaceIndex::new(default_workspace_roots(&repo_root));
    let openbitfun = OpenBitFunAdapter::new(&repo_root, index);
    registry.mount(Arc::new(openbitfun.clone())).unwrap();

    let cordis = CordisBus::new(CordisConfig::default_for_repo(&repo_root));
    let optimizer = OptimizationMiddleware::default();
    let privacy_guard = PrivacyGuard::new();
    let cua_envelope = CapabilityEnvelope {
        subject: "clicky-cua".into(),
        fs_read: vec![repo_root.display().to_string()],
        fs_write: vec![],
        network_allow: false,
        cua_apps: vec!["Xcode".into(), "Figma".into()],
        allow_vision: true,
        allow_audio: true,
    };

    let cfg_tmp = tempfile::tempdir().unwrap();
    let state = AppState {
        registry,
        store: store.clone(),
        cordis: cordis.clone(),
        optimizer,
        openbitfun: openbitfun.clone(),
        privacy_guard,
        cua_envelope,
        overlay_state: Arc::new(RwLock::new(OverlayState::default())),
        window_tx: None,
        agent_manager: AgentManager::new(&repo_root, Some(openbitfun)),
        studio_recipes: StudioRecipes::new(cfg_tmp.path()),
        remote_handoff: RemoteHandoffManager::new(cfg_tmp.path(), &repo_root),
    };

    let bound = spawn_bridge_servers(state.clone(), 0, 0).await.unwrap();

    // 1. Companion POST /chat returns Anthropic SSE + [POINT:...] and logs to SQLite WAL
    let chat_body = json!({
        "session_id": "companion-test",
        "messages": [{"role": "user", "content": "Where is the commit button?"}]
    })
    .to_string();
    let (chat_status, chat_resp) =
        http_request(bound.clicky_addr, "POST", "/chat", Some(&chat_body)).await;
    assert_eq!(chat_status, 200);
    assert!(chat_resp.contains("content_block_delta"));
    assert!(chat_resp.contains("[POINT:"));
    assert!(chat_resp.contains("message_stop"));

    let events = store.project_session_events("companion-test").unwrap();
    assert!(events.len() >= 2);

    // 2. Overlay state reflects the spatial command from /chat and supports manual Draw + Glass sync
    let glass_req = json!({ "opacity": 0.16, "tint": "warm-smoke" }).to_string();
    let (glass_status, glass_body) =
        http_request(bound.bridge_addr, "POST", "/ui/glass", Some(&glass_req)).await;
    assert_eq!(glass_status, 200);
    assert!(glass_body.contains("warm-smoke"));

    let (ov_status, ov_body) =
        http_request(bound.bridge_addr, "GET", "/overlay/state", None).await;
    assert_eq!(ov_status, 200);
    let ov_json: Value = serde_json::from_str(&ov_body).unwrap();
    assert!(!ov_json["commands"].as_array().unwrap().is_empty());
    assert_eq!(ov_json["glass_tint"], "warm-smoke");
    assert!((ov_json["glass_opacity"].as_f64().unwrap() - 0.16).abs() < 0.01);

    let draw_cmd = json!({
        "command": { "type": "draw", "shape": "rect", "x": 50, "y": 60, "width": 120, "height": 80, "caption": "Inspect" }
    })
    .to_string();
    let (ov_post_status, _) =
        http_request(bound.bridge_addr, "POST", "/overlay/command", Some(&draw_cmd)).await;
    assert_eq!(ov_post_status, 200);

    // 3. CUA action is 403 Forbidden outside CapabilityEnvelope, 200 OK inside
    let denied_cua = json!({
        "action_id": "c1",
        "target_app": "Terminal",
        "action": "click",
        "params": {"x": 10, "y": 10},
        "background": false
    })
    .to_string();
    let (s_denied, _) =
        http_request(bound.bridge_addr, "POST", "/cua/action", Some(&denied_cua)).await;
    assert_eq!(s_denied, 403);

    let ok_cua = json!({
        "action_id": "c2",
        "target_app": "Xcode",
        "action": "click",
        "params": {"x": 20, "y": 30},
        "background": true
    })
    .to_string();
    let (s_ok, _) = http_request(bound.bridge_addr, "POST", "/cua/action", Some(&ok_cua)).await;
    assert_eq!(s_ok, 200);

    // 4. Sessions list + fork + memory + watchdog rollback drill
    let (sess_status, sess_body) =
        http_request(bound.bridge_addr, "GET", "/sessions", None).await;
    assert_eq!(sess_status, 200);
    assert!(sess_body.contains("companion-test"));

    let fork_body = json!({ "up_to_seq": events[0].seq, "new_title": "Forked UI Test" }).to_string();
    let (fork_status, fork_resp) = http_request(
        bound.bridge_addr,
        "POST",
        "/sessions/companion-test/fork",
        Some(&fork_body),
    )
    .await;
    assert_eq!(fork_status, 200);
    assert!(fork_resp.contains("Forked UI Test"));

    let mem_body = json!({ "key": "user.os", "value": "macOS arm64", "pinned": true }).to_string();
    let (mem_post, _) =
        http_request(bound.bridge_addr, "POST", "/memory", Some(&mem_body)).await;
    assert_eq!(mem_post, 200);
    let (mem_get, mem_list) = http_request(bound.bridge_addr, "GET", "/memory", None).await;
    assert_eq!(mem_get, 200);
    assert!(mem_list.contains("user.os"));

    let (rb_status, rb_body) = http_request(
        bound.bridge_addr,
        "POST",
        "/watchdog/test-rollback",
        Some("{}"),
    )
    .await;
    assert_eq!(rb_status, 200);
    let rb_json: Value = serde_json::from_str(&rb_body).unwrap();
    assert!(rb_json["rollback_ms"].as_u64().unwrap() < 1000);
    assert_eq!(rb_json["restored_version"], "1.0.0-stable");

    // 5. Telemetry reflects live Cordis status and optimization stats
    let (tel_status, tel_body) = http_request(bound.bridge_addr, "GET", "/telemetry", None).await;
    assert_eq!(tel_status, 200);
    let tel: Value = serde_json::from_str(&tel_body).unwrap();
    assert_eq!(tel["cordis_state"], "running");
    assert!(tel["total_events_logged"].as_u64().unwrap() >= 3);

    // 6. Resident WorkspaceIndex search locates CompanionManager.swift
    let (search_status, search_body) = http_request(
        bound.bridge_addr,
        "GET",
        "/search?q=CompanionManager&max=5",
        None,
    )
    .await;
    assert_eq!(search_status, 200);
    let search_json: Value = serde_json::from_str(&search_body).unwrap();
    assert!(!search_json["matches"].as_array().unwrap().is_empty());

    // 7. Cordis shutdown endpoint drops state to stopped
    let (sd_status, sd_body) =
        http_request(bound.bridge_addr, "POST", "/cordis/shutdown", Some("{}")).await;
    assert_eq!(sd_status, 200);
    let sd_json: Value = serde_json::from_str(&sd_body).unwrap();
    assert_eq!(sd_json["cordis"]["state"], "stopped");

    // 8. Agent Manager UI + Demo Crew + Studio Shape/Rollback + Remote Handoff endpoints
    let (ui_status, ui_html) = http_request(bound.bridge_addr, "GET", "/agent-manager", None).await;
    assert_eq!(ui_status, 200);
    assert!(ui_html.contains("Agent Manager"));

    let (crew_status, crew_body) = http_request(bound.bridge_addr, "POST", "/agents/demo-crew", Some("{}")).await;
    assert_eq!(crew_status, 200);
    assert!(crew_body.contains("Ringmaster-Orchestrator"));

    let (shape_status, shape_body) = http_request(
        bound.bridge_addr,
        "POST",
        "/studio/shape",
        Some(r#"{"prompt":"Use pixel hq layout showing only blocked agents"}"#),
    )
    .await;
    assert_eq!(shape_status, 200);
    assert!(shape_body.contains("pixel-hq"));

    let (rb_ui_status, rb_ui_body) = http_request(bound.bridge_addr, "POST", "/studio/rollback", Some("{}")).await;
    assert_eq!(rb_ui_status, 200);
    assert!(rb_ui_body.contains("\"active_layout\":\"kanso\""));

    let (hof_status, hof_body) = http_request(
        bound.bridge_addr,
        "POST",
        "/remote/handoff",
        Some(r#"{"peer_id":"peer-loopback-studio","engine":"claude-code","task_prompt":"Test remote handoff via bridge"}"#),
    )
    .await;
    assert_eq!(hof_status, 200);
    let hof_json: Value = serde_json::from_str(&hof_body).unwrap();
    let hof_id = hof_json["handoff"]["handoff_id"].as_str().unwrap();

    tokio::time::sleep(std::time::Duration::from_millis(650)).await;
    let (recv_status, recv_body) = http_request(
        bound.bridge_addr,
        "POST",
        &format!("/remote/handoff/{hof_id}/receive"),
        Some(r#"{"apply_locally":false}"#),
    )
    .await;
    assert_eq!(recv_status, 200);
    assert!(recv_body.contains("handoff_result.md"));

    // 9. Favicon 204, /health runtime telemetry & /ui/glass clamping/normalization
    let (fav_status, _) = http_request(bound.bridge_addr, "GET", "/favicon.ico", None).await;
    assert_eq!(fav_status, 204);

    let (health_status, health_body) = http_request(bound.bridge_addr, "GET", "/health", None).await;
    assert_eq!(health_status, 200);
    let h_json: Value = serde_json::from_str(&health_body).unwrap();
    assert!(h_json["active_agents"].as_u64().unwrap_or(0) >= 3);

    let (g1_status, g1_body) = http_request(
        bound.bridge_addr,
        "POST",
        "/ui/glass",
        Some(r#"{"opacity":1.9,"tint":"invalid-neon"}"#),
    )
    .await;
    assert_eq!(g1_status, 200);
    let g1: Value = serde_json::from_str(&g1_body).unwrap();
    assert!((g1["glass_opacity"].as_f64().unwrap() - 0.96).abs() < 0.01);
    assert_eq!(g1["glass_tint"], "obsidian");

    // Reset glass cleanly back to default 0.80 Obsidian
    let _ = http_request(
        bound.bridge_addr,
        "POST",
        "/ui/glass",
        Some(r#"{"opacity":0.80,"tint":"obsidian"}"#),
    )
    .await;

    let _ = state.agent_manager.stop_all();
    bound.clicky_handle.abort();
    bound.bridge_handle.abort();
}

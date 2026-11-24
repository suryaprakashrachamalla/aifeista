#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";
import readline from "node:readline";

const startedAt = Date.now();
const args = process.argv.slice(2);
const repoRootFlagIdx = args.indexOf("--repo-root");
const repoRoot =
  repoRootFlagIdx >= 0 && args[repoRootFlagIdx + 1]
    ? path.resolve(args[repoRootFlagIdx + 1])
    : path.resolve(process.cwd());

function discoverHarnesses(root) {
  const coreDir = path.join(root, "crates", "layer0-core");
  const runtimeDir = path.join(root, "runtime");
  const companionDir = path.join(root, "companion");

  const readJsonNameVersion = (filePath) => {
    try {
      const parsed = JSON.parse(fs.readFileSync(filePath, "utf8"));
      return { name: parsed.name ?? null, version: parsed.version ?? null };
    } catch {
      return { name: null, version: null };
    }
  };

  const runtimeMeta = readJsonNameVersion(path.join(runtimeDir, "package.json"));

  return {
    repo_root: root,
    harnesses: [
      {
        id: "openbitfun",
        present: fs.existsSync(path.join(coreDir, "Cargo.toml")),
        path: coreDir,
        name: "carnival-core",
        version: "0.1.0",
        capabilities: ["tauri_desktop", "rust_runtime", "workspace_index", "sandbox", "watchdog"],
      },
      {
        id: "deepseek_harness",
        present: fs.existsSync(path.join(runtimeDir, "package.json")),
        path: runtimeDir,
        name: runtimeMeta.name,
        version: runtimeMeta.version,
        capabilities: ["cordis_microkernel", "ptc_runtime", "sandbox_policy", "sdk_jsonrpc"],
      },
      {
        id: "clicky",
        present: fs.existsSync(path.join(companionDir, "leanring-buddy")),
        path: companionDir,
        name: "carnival-companion",
        version: "macos-native-speech",
        capabilities: ["apple_speech_tts", "flying_pointer", "screen_capture_kit", "chat_sse_8787"],
      },
    ],
  };
}

function handleRouteTurn(params = {}) {
  const prompt = String(params.prompt ?? "").trim();
  const lane = String(params.lane ?? "builder_workspace");
  const lower = prompt.toLowerCase();
  const screenCtx = params.screen_context ?? null;

  let spatialCommand = null;
  let text;

  if (
    lane === "clicky_ambient" ||
    lower.includes("where") ||
    lower.includes("point") ||
    lower.includes("button") ||
    lower.includes("click")
  ) {
    const x = typeof screenCtx?.cursor_x === "number" ? screenCtx.cursor_x + 120 : 420;
    const y = typeof screenCtx?.cursor_y === "number" ? screenCtx.cursor_y + 40 : 180;
    const label = lower.includes("commit")
      ? "commit button"
      : lower.includes("export")
        ? "export setting"
        : "target control";
    spatialCommand = { type: "point", x, y, label, screen: 1 };
    text = `Carnival routed your request: look near (${x}, ${y}) for the ${label}. [POINT:${x},${y}:${label}]`;
  } else {
    text = `Carnival routed (${lane}): processed "${prompt}" via skill tree and core runtime.`;
  }

  return {
    handled_by: "cordis_lazy_sidecar",
    text,
    spatial_command: spatialCommand,
    uptime_ms: Date.now() - startedAt,
    rss_bytes: process.memoryUsage().rss,
  };
}

const rl = readline.createInterface({
  input: process.stdin,
  crlfDelay: Infinity,
});

function send(obj) {
  process.stdout.write(`${JSON.stringify(obj)}\n`);
}

rl.on("line", (line) => {
  const trimmed = line.trim();
  if (!trimmed) return;

  let req;
  try {
    req = JSON.parse(trimmed);
  } catch (err) {
    send({
      jsonrpc: "2.0",
      id: "parse-error",
      error: { code: -32700, message: `Invalid JSON: ${String(err)}` },
    });
    return;
  }

  const id = String(req.id ?? "");
  const method = req.method;
  const params = req.params ?? {};

  try {
    if (method === "ping") {
      send({
        jsonrpc: "2.0",
        id,
        result: {
          ok: true,
          pid: process.pid,
          rss_bytes: process.memoryUsage().rss,
          uptime_ms: Date.now() - startedAt,
        },
      });
    } else if (method === "discover_harnesses") {
      const targetRoot = params.repo_root ? path.resolve(String(params.repo_root)) : repoRoot;
      send({
        jsonrpc: "2.0",
        id,
        result: discoverHarnesses(targetRoot),
      });
    } else if (method === "route_turn") {
      send({
        jsonrpc: "2.0",
        id,
        result: handleRouteTurn(params),
      });
    } else if (method === "shutdown") {
      send({
        jsonrpc: "2.0",
        id,
        result: { shutting_down: true, pid: process.pid },
      });
      rl.close();
      process.exit(0);
    } else {
      send({
        jsonrpc: "2.0",
        id,
        error: { code: -32601, message: `Method not found: ${String(method)}` },
      });
    }
  } catch (err) {
    send({
      jsonrpc: "2.0",
      id,
      error: { code: -32000, message: String(err) },
    });
  }
});

rl.on("close", () => {
  process.exit(0);
});

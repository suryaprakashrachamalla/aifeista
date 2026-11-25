#!/usr/bin/env bash
set -euo pipefail

VERSION="${VERSION:-0.1.0}"
NODE_VERSION="26.13.1"
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIST_DIR="$ROOT_DIR/dist"
APP_DIR="$DIST_DIR/Carnival.app"
RESOURCES_DIR="$APP_DIR/Contents/Resources"
NODE_CACHE_DIR="/tmp/carnival-node-cache"
STAGE_DMG="/tmp/carnival-dmg-stage"
STAGE_PKG_ROOT="/tmp/carnival-pkg-root"
TMP_COMPONENT_PKG="/tmp/carnival-component.pkg"
SMOKE_DSH_HOME="/tmp/carnival-pkg-smoke"

export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"

echo "==> [1/8] Fetching official standalone Node v${NODE_VERSION} (darwin-arm64)"
mkdir -p "$NODE_CACHE_DIR"
NODE_TARBALL="node-v${NODE_VERSION}-darwin-arm64.tar.gz"
NODE_ARCHIVE="$NODE_CACHE_DIR/$NODE_TARBALL"
if [ ! -f "$NODE_ARCHIVE" ]; then
  curl -fsSL "https://nodejs.org/dist/v${NODE_VERSION}/${NODE_TARBALL}" -o "$NODE_ARCHIVE"
fi

echo "==> [2/8] Building desktop Tauri release binary"
cargo build --release --manifest-path "$ROOT_DIR/desktop/src-tauri/Cargo.toml"

DESKTOP_BIN=""
for candidate in \
  "$ROOT_DIR/desktop/src-tauri/target/release/ai-carnival-desktop" \
  "$ROOT_DIR/target/release/ai-carnival-desktop"; do
  if [ -x "$candidate" ]; then
    DESKTOP_BIN="$candidate"
    break
  fi
done
if [ -z "$DESKTOP_BIN" ]; then
  echo "ERROR: Could not locate compiled ai-carnival-desktop release binary" >&2
  exit 1
fi
echo "    Using binary: $DESKTOP_BIN"

echo "==> [3/8] Assembling $APP_DIR"
rm -rf "$APP_DIR"
mkdir -p "$APP_DIR/Contents/MacOS" "$RESOURCES_DIR/bin" "$RESOURCES_DIR/sidecars/cordis-bridge"

cp "$DESKTOP_BIN" "$APP_DIR/Contents/MacOS/ai-carnival-desktop"
chmod +x "$APP_DIR/Contents/MacOS/ai-carnival-desktop"
cp "$ROOT_DIR/desktop/src-tauri/icons/icon.icns" "$RESOURCES_DIR/icon.icns"
cp "$ROOT_DIR/sidecars/cordis-bridge/index.mjs" "$RESOURCES_DIR/sidecars/cordis-bridge/index.mjs"

tar -xzf "$NODE_ARCHIVE" -C "$NODE_CACHE_DIR" "node-v${NODE_VERSION}-darwin-arm64/bin/node"
cp "$NODE_CACHE_DIR/node-v${NODE_VERSION}-darwin-arm64/bin/node" "$RESOURCES_DIR/bin/node"
chmod +x "$RESOURCES_DIR/bin/node"

OTOOL_OUT="$(otool -L "$RESOURCES_DIR/bin/node")"
if echo "$OTOOL_OUT" | grep -qE '/opt/homebrew|/usr/local'; then
  echo "ERROR: Bundled node binary links non-system libraries:" >&2
  echo "$OTOOL_OUT" >&2
  exit 1
fi
echo "    Verified bundled node has zero Homebrew/local dylib dependencies."

cat > "$APP_DIR/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleDevelopmentRegion</key>
	<string>English</string>
	<key>CFBundleDisplayName</key>
	<string>Carnival</string>
	<key>CFBundleExecutable</key>
	<string>ai-carnival-desktop</string>
	<key>CFBundleIdentifier</key>
	<string>com.carnival.desktop</string>
	<key>CFBundleInfoDictionaryVersion</key>
	<string>6.0</string>
	<key>CFBundleName</key>
	<string>Carnival</string>
	<key>CFBundlePackageType</key>
	<string>APPL</string>
	<key>CFBundleShortVersionString</key>
	<string>${VERSION}</string>
	<key>CFBundleVersion</key>
	<string>${VERSION}</string>
	<key>CFBundleIconFile</key>
	<string>icon.icns</string>
	<key>NSHighResolutionCapable</key>
	<true/>
	<key>LSMinimumSystemVersion</key>
	<string>13.0</string>
	<key>NSMicrophoneUsageDescription</key>
	<string>Carnival uses your microphone for hands-free voice companion queries.</string>
	<key>NSSpeechRecognitionUsageDescription</key>
	<string>Carnival uses speech recognition to transcribe voice companion prompts.</string>
</dict>
</plist>
EOF

echo "==> [4/8] Deploying standalone runtime into $RESOURCES_DIR/runtime"
(
  cd "$ROOT_DIR/runtime"
  pnpm --filter dsh-python-runtime-closure --prod deploy --legacy "$RESOURCES_DIR/runtime"
)

echo "==> [5/8] Hoisting runtime links, writing entrypoint, pruning dev bloat, checking symlinks"
node - "$RESOURCES_DIR/runtime" <<'NODE_SCRIPT'
const fs = require("node:fs");
const path = require("node:path");

const target = path.resolve(process.argv[2]);
const pnpmDir = path.join(target, "node_modules", ".pnpm");
const hoistRoot = path.join(pnpmDir, "node_modules");
const topRoot = path.join(target, "node_modules");
fs.mkdirSync(hoistRoot, { recursive: true });

// Materialize any symlinks pointing outside the runtime bundle (e.g. workspace `link:` overrides)
let materialized = 0;
function derefExternalSymlinks(dir) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isSymbolicLink()) {
      const linkTarget = fs.readlinkSync(full);
      const resolved = path.resolve(path.dirname(full), linkTarget);
      if (resolved !== target && !resolved.startsWith(target + path.sep)) {
        fs.rmSync(full, { recursive: true, force: true });
        if (fs.existsSync(resolved)) {
          fs.cpSync(resolved, full, {
            recursive: true,
            dereference: true,
            filter: (src) => path.basename(src) !== "node_modules" && path.basename(src) !== ".git",
          });
          materialized++;
        }
      }
    } else if (entry.isDirectory()) {
      derefExternalSymlinks(full);
    }
  }
}
derefExternalSymlinks(target);

const seen = new Map();
for (const entry of fs.readdirSync(pnpmDir)) {
  if (entry === "node_modules") continue;
  const innerNm = path.join(pnpmDir, entry, "node_modules");
  if (!fs.existsSync(innerNm)) continue;
  for (const item of fs.readdirSync(innerNm)) {
    const full = path.join(innerNm, item);
    if (item.startsWith("@")) {
      for (const sub of fs.readdirSync(full)) {
        const pkgName = `${item}/${sub}`;
        const pkgDir = path.join(full, sub);
        if (!seen.has(pkgName) && fs.existsSync(path.join(pkgDir, "package.json"))) {
          seen.set(pkgName, pkgDir);
        }
      }
    } else if (!seen.has(item) && fs.existsSync(path.join(full, "package.json"))) {
      seen.set(item, full);
    }
  }
}

for (const [pkgName, realDir] of seen.entries()) {
  for (const base of [hoistRoot, topRoot]) {
    const dest = path.join(base, pkgName);
    if (fs.existsSync(dest)) continue;
    fs.mkdirSync(path.dirname(dest), { recursive: true });
    fs.symlinkSync(path.relative(path.dirname(dest), realDir), dest, "dir");
  }
}

fs.mkdirSync(path.join(target, "lib"), { recursive: true });
fs.writeFileSync(
  path.join(target, "lib", "bin.js"),
  'import { runCli } from "../node_modules/@carnival/dsh/lib/bin.js";\nawait runCli();\n',
  "utf8"
);

// Prune unneeded type declarations, source maps, build info, and test dirs
function prune(dir) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isSymbolicLink()) continue;
    if (entry.isDirectory()) {
      if (entry.name === "tests" || entry.name === "__tests__") {
        fs.rmSync(full, { recursive: true, force: true });
      } else {
        prune(full);
      }
    } else if (
      entry.name.endsWith(".map") ||
      entry.name.endsWith(".d.ts") ||
      entry.name.endsWith(".d.mts") ||
      entry.name.endsWith(".tsbuildinfo")
    ) {
      fs.rmSync(full, { force: true });
    }
  }
}
prune(target);

// Verify zero symlinks escape outside target
let escaped = [];
function verifySymlinks(dir) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isSymbolicLink()) {
      const linkTarget = fs.readlinkSync(full);
      const resolved = path.resolve(path.dirname(full), linkTarget);
      if (resolved !== target && !resolved.startsWith(target + path.sep)) {
        escaped.push({ link: full, target: linkTarget, resolved });
      }
    } else if (entry.isDirectory()) {
      verifySymlinks(full);
    }
  }
}
verifySymlinks(target);
if (escaped.length > 0) {
  console.error("ERROR: Found symlinks escaping runtime bundle:", escaped.slice(0, 10));
  process.exit(1);
}
console.log("    Hoisted packages:", seen.size, "| Escaped symlinks: 0");
NODE_SCRIPT

echo "==> [6/8] Smoke-testing bundled node + runtime"
rm -rf "$SMOKE_DSH_HOME" /tmp/carnival-pkg-smoke.log /tmp/carnival-pkg-cookies /tmp/carnival-pkg-index.html
DSH_HOME="$SMOKE_DSH_HOME" "$RESOURCES_DIR/bin/node" "$RESOURCES_DIR/runtime/lib/bin.js" web --no-open --port 3092 > /tmp/carnival-pkg-smoke.log 2>&1 &
SMOKE_PID=$!
cleanup_smoke() {
  kill "$SMOKE_PID" 2>/dev/null || true
  wait "$SMOKE_PID" 2>/dev/null || true
  rm -rf "$SMOKE_DSH_HOME" /tmp/carnival-pkg-smoke.log /tmp/carnival-pkg-cookies /tmp/carnival-pkg-index.html
}
trap cleanup_smoke EXIT

BOOT_URL=""
for _ in $(seq 1 40); do
  if grep -qE 'http://127\.0\.0\.1:3092' /tmp/carnival-pkg-smoke.log 2>/dev/null; then
    BOOT_URL="$(grep -oE 'http://127\.0\.0\.1:3092[^[:space:]]*' /tmp/carnival-pkg-smoke.log | head -n1)"
    break
  fi
  sleep 0.5
done

if [ -z "$BOOT_URL" ]; then
  echo "ERROR: Bundled runtime failed to emit URL on port 3092:" >&2
  cat /tmp/carnival-pkg-smoke.log >&2 || true
  exit 1
fi

HTTP_CODE="$(curl -sSL -c /tmp/carnival-pkg-cookies -b /tmp/carnival-pkg-cookies -o /tmp/carnival-pkg-index.html -w "%{http_code}" "$BOOT_URL")"
if [ "$HTTP_CODE" != "200" ]; then
  echo "ERROR: Bundled runtime web check returned HTTP $HTTP_CODE (expected 200)" >&2
  head -c 500 /tmp/carnival-pkg-index.html >&2 || true
  exit 1
fi
echo "    Bundled runtime web boot OK (HTTP 200 at $BOOT_URL)"
cleanup_smoke
trap - EXIT

echo "==> [7/8] Signing Carnival.app (ad-hoc)"
codesign --force --deep --sign - "$APP_DIR"
codesign --verify --deep --strict "$APP_DIR"

echo "==> [8/8] Building .dmg and .pkg installers"
DMG_PATH="$DIST_DIR/Carnival-${VERSION}-macOS-arm64.dmg"
PKG_PATH="$DIST_DIR/Carnival-${VERSION}-macOS-arm64.pkg"

rm -rf "$STAGE_DMG" "$DMG_PATH"
mkdir -p "$STAGE_DMG"
cp -R "$APP_DIR" "$STAGE_DMG/Carnival.app"
ln -s /Applications "$STAGE_DMG/Applications"
hdiutil create -volname "Carnival" -srcfolder "$STAGE_DMG" -ov -format UDZO "$DMG_PATH"
rm -rf "$STAGE_DMG"

rm -rf "$STAGE_PKG_ROOT" "$TMP_COMPONENT_PKG" "$PKG_PATH"
mkdir -p "$STAGE_PKG_ROOT/Applications"
cp -R "$APP_DIR" "$STAGE_PKG_ROOT/Applications/Carnival.app"
pkgbuild \
  --root "$STAGE_PKG_ROOT" \
  --identifier com.carnival.desktop \
  --version "$VERSION" \
  --install-location / \
  "$TMP_COMPONENT_PKG"
productbuild \
  --package "$TMP_COMPONENT_PKG" \
  "$PKG_PATH"
rm -rf "$STAGE_PKG_ROOT" "$TMP_COMPONENT_PKG"

echo "==> Done! Packaged artifacts:"
ls -lh "$DIST_DIR"

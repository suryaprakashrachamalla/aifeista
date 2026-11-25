(function () {
  if (
    window.location.protocol !== "http:" ||
    window.location.hostname !== "127.0.0.1" ||
    window.location.port === "32123" ||
    window.location.port === "8787"
  ) {
    return;
  }

  const BRIDGE = "http://127.0.0.1:32123";
  const TINTS = {
    obsidian: { label: "Obsidian", rgb: "6, 6, 8", sidebar: "9, 9, 11", layer: "16, 16, 19" },
    jet: { label: "Jet Black", rgb: "2, 2, 4", sidebar: "4, 4, 6", layer: "12, 12, 15" },
    "warm-smoke": { label: "Warm Smoke", rgb: "14, 12, 11", sidebar: "18, 16, 14", layer: "24, 21, 19" },
  };

  function normalizeTint(key) {
    if (TINTS[key]) return key;
    return "obsidian";
  }

  function clampOpacity(val) {
    if (!Number.isFinite(val)) return 0.80;
    return Math.min(0.96, Math.max(0.08, val));
  }

  let currentOpacity = clampOpacity(parseFloat(localStorage.getItem("carnival.glassOpacity") || "0.80"));
  let currentTint = normalizeTint(localStorage.getItem("carnival.glassTint") || "obsidian");

  function applyGlassVars(opacity, tintKey) {
    currentOpacity = clampOpacity(opacity);
    currentTint = normalizeTint(tintKey);
    const t = TINTS[currentTint];
    const op = currentOpacity;
    const sidebarOp = Math.min(0.97, op + 0.06);
    const layer1 = Math.min(0.96, op + 0.09);
    const layer2 = Math.min(0.98, op + 0.14);
    const layer3 = Math.min(0.99, op + 0.18);
    const menuOp = Math.min(0.97, Math.max(0.74, op + 0.14));

    const root = document.documentElement;
    root.style.colorScheme = "dark";
    root.style.setProperty("--carnival-glass-opacity", op.toFixed(2));
    root.style.setProperty("--carnival-glass-base-rgb", t.rgb);
    if (document.body) {
      document.body.setAttribute("data-ds-dark-theme", "true");
    }

    let varsStyle = document.getElementById("carnival-glass-vars");
    if (!varsStyle) {
      varsStyle = document.createElement("style");
      varsStyle.id = "carnival-glass-vars";
      (document.head || root).appendChild(varsStyle);
    }
    varsStyle.textContent = `
      html, body, body[data-ds-dark-theme], #root {
        --carnival-glass-opacity: ${op.toFixed(2)};
        --carnival-glass-base-rgb: ${t.rgb};
        --dsw-alias-bg-base: rgba(${t.rgb}, ${op.toFixed(3)}) !important;
        --dsw-specific-sidebar-fill: rgba(${t.sidebar}, ${sidebarOp.toFixed(3)}) !important;
        --dsw-alias-bg-layer-1: rgba(${t.layer}, ${layer1.toFixed(3)}) !important;
        --dsw-alias-bg-layer-2: rgba(${t.layer}, ${layer2.toFixed(3)}) !important;
        --dsw-alias-bg-layer-3: rgba(${t.layer}, ${layer3.toFixed(3)}) !important;
        --dsw-specific-input-major: rgba(${t.layer}, ${layer1.toFixed(3)}) !important;
        --dsw-alias-card-fill: rgba(${t.layer}, ${layer1.toFixed(3)}) !important;
        --dsw-specific-menu: rgba(${t.sidebar}, ${menuOp.toFixed(3)}) !important;
        --dsw-alias-label-primary: #f5f2eb !important;
        --dsw-alias-label-secondary: rgba(245, 242, 235, 0.70) !important;
        --dsw-alias-label-tertiary: rgba(245, 242, 235, 0.44) !important;
        --dsw-alias-label-caption: rgba(245, 242, 235, 0.38) !important;
        --dsw-alias-interactive-bg-hover: rgba(255, 255, 255, 0.08) !important;
        --dsw-alias-bg-mask-1: rgba(4, 4, 6, 0.56) !important;
        --dsw-alias-border-l1: rgba(255, 255, 255, 0.09) !important;
        --dsw-alias-border-l2: rgba(255, 255, 255, 0.14) !important;
        --dsw-alias-border-l3: rgba(255, 255, 255, 0.07) !important;
        --dsw-alias-brand-primary: #f5f2eb !important;
        --dsw-specific-primary-fill: rgba(237, 230, 214, 0.16) !important;
      }
    `;

    localStorage.setItem("carnival.glassOpacity", op.toFixed(2));
    localStorage.setItem("carnival.glassTint", currentTint);

    const amFrame = document.getElementById("carnival-agent-manager-frame");
    if (amFrame && amFrame.contentWindow) {
      amFrame.contentWindow.postMessage(
        { type: "carnival:glass", opacity: op, tint: currentTint },
        "*"
      );
    }
  }

  let glassSyncTimer = null;
  function pushGlassState(immediate) {
    const send = () => {
      fetch(BRIDGE + "/ui/glass", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ opacity: currentOpacity, tint: currentTint }),
      }).catch(() => {});
    };
    if (glassSyncTimer) clearTimeout(glassSyncTimer);
    if (immediate) send();
    else glassSyncTimer = setTimeout(send, 80);
  }

  window.addEventListener("carnival:set-glass", function (e) {
    const detail = (e && e.detail) || {};
    const op = typeof detail.opacity === "number" ? detail.opacity : currentOpacity;
    const tint = typeof detail.tint === "string" ? detail.tint : currentTint;
    applyGlassVars(op, tint);
    pushGlassState(true);
  });

  function toggleSpotlight() {
    fetch(BRIDGE + "/window/spotlight", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ action: "toggle" }),
    }).catch(() => {});
  }

  let agentManagerOpen = false;
  function toggleAgentManager(forceState) {
    const next = typeof forceState === "boolean" ? forceState : !agentManagerOpen;
    agentManagerOpen = next;
    let stage = document.getElementById("carnival-agent-manager-stage");
    if (!stage && next) {
      stage = document.createElement("div");
      stage.id = "carnival-agent-manager-stage";
      stage.innerHTML = `<iframe id="carnival-agent-manager-frame" src="${BRIDGE}/agent-manager" allow="clipboard-write"></iframe>`;
      document.body.appendChild(stage);
      const frame = document.getElementById("carnival-agent-manager-frame");
      frame.addEventListener("load", () => {
        if (frame.contentWindow) {
          frame.contentWindow.postMessage(
            { type: "carnival:glass", opacity: currentOpacity, tint: currentTint },
            "*"
          );
        }
      });
    }
    if (stage) {
      stage.style.display = next ? "block" : "none";
    }
    window.dispatchEvent(
      new CustomEvent("carnival:agent-manager-state", { detail: { open: next } })
    );
  }

  window.addEventListener("carnival:toggle-agent-manager", function () {
    toggleAgentManager();
  });

  window.addEventListener("message", function (ev) {
    const msg = ev && ev.data;
    if (msg && msg.type === "carnival:toggle-agent-manager") {
      toggleAgentManager();
    }
  });

  window.addEventListener("keydown", function (e) {
    if ((e.metaKey || e.ctrlKey) && !e.shiftKey && e.key.toLowerCase() === "k") {
      e.preventDefault();
      toggleSpotlight();
    } else if ((e.metaKey || e.ctrlKey) && e.shiftKey && e.key.toLowerCase() === "a") {
      e.preventDefault();
      toggleAgentManager();
    }
  });

  function scrubChromeText(root) {
    if (!root) return;
    if (document.title !== "Carnival" && document.title !== "Connect to this Host") {
      document.title = "Carnival";
    }
    const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT, null);
    let node;
    while ((node = walker.nextNode())) {
      const parent = node.parentElement;
      if (!parent) continue;
      if (parent.closest("input, textarea, code, pre, [contenteditable]")) continue;
      const v = node.nodeValue;
      if (!v) continue;
      if (v.includes("Carnival") === false && (v.includes("DSH Local Build") || v.includes("DSH"))) {
        node.nodeValue = v
          .replace(/DSH Local Build/g, "Carnival");
      } else if (v.includes("AI Carnival")) {
        node.nodeValue = v.replace(/AI Carnival/g, "Carnival");
      }
    }
  }

  function mountShell() {
    if (document.title === "Connect to this Host") return;
    document.title = "Carnival";
    scrubChromeText(document.body);
    applyGlassVars(currentOpacity, currentTint);

    if (!document.getElementById("carnival-glass-style")) {
      const style = document.createElement("style");
      style.id = "carnival-glass-style";
      style.textContent = `
        html, body, #root {
          height: 100%;
          margin: 0;
          padding: 0;
          background: transparent !important;
          background-color: transparent !important;
        }
        /* Avoid flashing device pairing gate during instant token redirect */
        dsh-device-pairing {
          opacity: 0 !important;
          pointer-events: none !important;
        }
        /* Seamless smoked glass card on main composer */
        [data-composer-card] {
          position: relative;
          background: rgba(var(--carnival-glass-base-rgb, 6, 6, 8), calc(var(--carnival-glass-opacity, 0.80) + 0.08)) !important;
          backdrop-filter: blur(28px) saturate(160%) !important;
          -webkit-backdrop-filter: blur(28px) saturate(160%) !important;
          border: 1px solid rgba(255, 255, 255, 0.11) !important;
          box-shadow: 0 18px 46px rgba(0, 0, 0, 0.45), inset 0 1px 0 rgba(255, 255, 255, 0.08) !important;
          transition: border-color 200ms ease, box-shadow 200ms ease !important;
        }
        [data-composer-card]:focus-within {
          border-color: rgba(237, 230, 214, 0.34) !important;
          box-shadow: 0 20px 52px rgba(0, 0, 0, 0.52), 0 0 0 1px rgba(237, 230, 214, 0.14), inset 0 1px 0 rgba(255, 255, 255, 0.12) !important;
        }
        /* Monochrome send button & razor-thin smoked scrollbars */
        [data-composer-card] button[type="submit"],
        [data-composer-card] button[aria-label*="Send" i],
        [data-composer-card] button[disabled] {
          background-color: rgba(237, 230, 214, 0.14) !important;
          color: #f5f2eb !important;
          border: 1px solid rgba(255, 255, 255, 0.12) !important;
        }
        [data-composer-card] button[type="submit"]:not(:disabled),
        [data-composer-card] button[aria-label*="Send" i]:not(:disabled) {
          background-color: #f5f2eb !important;
          color: #09090b !important;
        }
        ::-webkit-scrollbar {
          width: 6px;
          height: 6px;
        }
        ::-webkit-scrollbar-track {
          background: transparent;
        }
        ::-webkit-scrollbar-thumb {
          background: rgba(237, 230, 214, 0.16);
          border-radius: 999px;
        }
        ::-webkit-scrollbar-thumb:hover {
          background: rgba(237, 230, 214, 0.30);
        }
        [role="menu"],
        [role="listbox"],
        [role="dialog"] {
          backdrop-filter: blur(28px) saturate(160%) !important;
          -webkit-backdrop-filter: blur(28px) saturate(160%) !important;
          border: 1px solid rgba(255, 255, 255, 0.12) !important;
          border-radius: 14px !important;
          box-shadow: 0 24px 60px rgba(0, 0, 0, 0.62), inset 0 1px 0 rgba(255, 255, 255, 0.10) !important;
          color: #f5f2eb !important;
        }
        #carnival-titlebar-drag {
          position: fixed;
          top: 0;
          left: 0;
          right: 0;
          height: 38px;
          z-index: 2147483640;
          -webkit-app-region: drag;
          pointer-events: auto;
        }
        /* Ultra-minimal quiet Search trigger in top-right (all glass/engine settings live inside Settings -> General) */
        #carnival-switch-dock {
          position: fixed;
          top: 8px;
          right: 12px;
          z-index: 2147483647;
          display: flex;
          align-items: center;
          -webkit-app-region: no-drag;
        }
        #carnival-switch-dock .cs-search-pill {
          display: inline-flex;
          align-items: center;
          gap: 6px;
          height: 26px;
          padding: 0 10px;
          border-radius: 8px;
          border: 1px solid rgba(255, 255, 255, 0.10);
          background: rgba(14, 14, 17, 0.62);
          backdrop-filter: blur(20px);
          -webkit-backdrop-filter: blur(20px);
          color: rgba(245, 242, 235, 0.78);
          font-family: -apple-system, BlinkMacSystemFont, "SF Pro Text", sans-serif;
          font-size: 11.5px;
          font-weight: 500;
          cursor: pointer;
          transition: background 140ms ease, color 140ms ease, border-color 140ms ease;
        }
        #carnival-switch-dock .cs-search-pill:hover {
          background: rgba(255, 255, 255, 0.10);
          border-color: rgba(255, 255, 255, 0.20);
          color: #f5f2eb;
        }
        #carnival-switch-dock .cs-kbd {
          color: rgba(245, 242, 235, 0.44);
          font-size: 10.5px;
        }
        /* Flush borderless Agent Manager stage above the 50px bottom dock */
        #carnival-agent-manager-stage {
          position: fixed;
          top: 38px;
          left: 0;
          right: 0;
          bottom: 50px;
          z-index: 2147483635;
          display: none;
          padding: 0;
          box-sizing: border-box;
          background: rgba(5, 5, 7, 0.78);
          backdrop-filter: blur(26px);
          -webkit-backdrop-filter: blur(26px);
        }
        #carnival-agent-manager-frame {
          width: 100%;
          height: 100%;
          border: none;
          border-radius: 0;
          background: transparent;
          color-scheme: dark;
        }
      `;
      document.head.appendChild(style);
    }

    if (!document.getElementById("carnival-titlebar-drag")) {
      const dragBar = document.createElement("div");
      dragBar.id = "carnival-titlebar-drag";
      dragBar.setAttribute("data-tauri-drag-region", "");
      document.body.appendChild(dragBar);
    }

    if (document.getElementById("carnival-switch-dock")) return;

    const dock = document.createElement("div");
    dock.id = "carnival-switch-dock";
    dock.innerHTML = `
      <button type="button" class="cs-search-pill" id="cs-open-spotlight" title="Search Carnival (⌘K)">
        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><circle cx="11" cy="11" r="7"/><line x1="20" y1="20" x2="16" y2="16"/></svg>
        <span>Search</span>
        <span class="cs-kbd">⌘K</span>
      </button>
    `;
    document.body.appendChild(dock);

    document.getElementById("cs-open-spotlight").addEventListener("click", toggleSpotlight);

    function syncFromBridge() {
      fetch(BRIDGE + "/overlay/state")
        .then((r) => r.json())
        .then((ov) => {
          if (ov && typeof ov.glass_opacity === "number") {
            const nextTint = normalizeTint(ov.glass_tint || currentTint);
            if (Math.abs(ov.glass_opacity - currentOpacity) >= 0.015 || nextTint !== currentTint) {
              applyGlassVars(ov.glass_opacity, nextTint);
            }
          }
        })
        .catch(() => {});
    }

    syncFromBridge();
    setInterval(syncFromBridge, 2500);
  }

  function startObserver() {
    mountShell();
    if (document.body) {
      const obs = new MutationObserver(function () {
        scrubChromeText(document.body);
      });
      obs.observe(document.body, { childList: true, subtree: true, characterData: true });
    }
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", startObserver);
  } else {
    startObserver();
  }
  setInterval(function () {
    if (!document.getElementById("carnival-switch-dock") && document.body) {
      mountShell();
    }
  }, 1500);
})();

/** The package names the plugin manager protects, hides, orders, or marks experimental. */

/**
 * Plugin packages the plugin manager refuses to disable or remove, because management itself needs them; a bundle that
 * contains one is read-only as well. Removing a name drops only this `management-required` refusal; the manager's own
 * entry, installation-carried bundles, and unaddressable rows stay protected by their own rules.
 */
export const PROTECTED_PLUGINS: ReadonlySet<string> = new Set([
  '@carnival/dsh-plugin-manager', '@carnival/cordis-plugin-loader',
  '@carnival/cordis-plugin-include', '@carnival/dsh-api-gateway',
  '@carnival/dsh-host-webserver', '@carnival/dsh-client-modules',
  '@carnival/dsh-client-ui-plugin-manager',
  '@carnival/dsh-host-plugin-inventory', '@carnival/dsh-typert-registry',
  '@carnival/dsh-api-remotes', '@carnival/dsh-api-remote-access',
  '@carnival/cordis-plugin-timer', '@carnival/dsh-client-connection',
  '@carnival/dsh-host-frontend-static', '@carnival/dsh-tools',
  '@carnival/dsh-hmr',
])

/**
 * Built-in profile bundles, the standalone worktree tool, and the Computer Use MCP profile
 * bundle, which have no plugin manager page.
 * Their Host availability and activation stay intact.
 */
export const HIDDEN_BUNDLES: ReadonlySet<string> = new Set([
  '@carnival/dsh-base',
  '@carnival/dsh-web-app',
  '@carnival/dsh-headless',
  '@carnival/dsh-sdk-app',
  '@carnival/dsh-acp-app',
  '@carnival/dsh-sdk-minimal',
  '@carnival/dsh-experimental-tool-worktree',
  '@carnival/dsh-cua-mcp',
])

/**
 * The optional bundles the plugin manager's main and More lists rank first, in this order. {@link MORE_BUNDLES}, not
 * this list, decides whether a bundle appears on the main list.
 */
export const EXTENSION_BUNDLES: readonly string[] = [
  '@carnival/dsh-experimental-voice-input-bundle',
  '@carnival/dsh-experimental-plugin-market-bundle',
  '@carnival/dsh-experimental-auto-review',
]

/**
 * The optional bundles the plugin manager's main list leaves out until the profile enables them; its More list,
 * which includes every eligible bundle, ranks them last in this order.
 */
export const MORE_BUNDLES: readonly string[] = [
  '@carnival/dsh-experimental-inspector-profile',
  '@carnival/dsh-cua-bundle',
  '@carnival/dsh-timeline',
  '@carnival/dsh-experimental-terminal-bundle',
  '@carnival/dsh-experimental-cot-translation-bundle',
  '@carnival/dsh-experimental-session-search',
  '@carnival/dsh-experimental-session-titles-bundle',
  '@carnival/dsh-experimental-badge-skill-bundle',
  '@carnival/dsh-experimental-ralph-bundle',
  '@carnival/dsh-deepseek-chat-import',
  '@carnival/dsh-subagent-codex',
  '@carnival/dsh-subagent-claude-code',
]

/** Official bundles the plugin manager marks with the Experimental badge. */
export const EXPERIMENTAL_BUNDLES: ReadonlySet<string> = new Set([
  '@carnival/dsh-cua-bundle',
  '@carnival/dsh-cua-mcp',
  '@carnival/dsh-timeline',
  '@carnival/dsh-experimental-terminal-bundle',
  '@carnival/dsh-experimental-voice-input-bundle',
  '@carnival/dsh-experimental-auto-review',
  '@carnival/dsh-experimental-inspector-profile',
  '@carnival/dsh-experimental-forge-bundle',
  '@carnival/dsh-experimental-tool-worktree',
  '@carnival/dsh-experimental-cot-translation-bundle',
  '@carnival/dsh-experimental-session-search',
  '@carnival/dsh-experimental-session-titles-bundle',
  '@carnival/dsh-experimental-badge-skill-bundle',
  '@carnival/dsh-experimental-ralph-bundle',
  '@carnival/dsh-experimental-plugin-market-bundle',
  '@carnival/dsh-experimental-liveshare',
  '@carnival/dsh-deepseek-chat-import',
])

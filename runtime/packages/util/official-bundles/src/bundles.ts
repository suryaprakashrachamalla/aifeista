/** The bundle package names the dsh installation selects, offers, installs on demand, or retires. */

/** Installation-owned defaults used when a shipped profile is first opened. */
export interface ProfileTemplate {
  /** Ordered bundle layer list. */
  bundles: readonly string[]
}

/** The shipped profile templates auto-initialized on first use, by name. */
export const PROFILE_TEMPLATES: Record<string, ProfileTemplate> = {
  acp: {
    bundles: ['@carnival/dsh-base', '@carnival/dsh-acp-app'],
  },
  web: {
    bundles: ['@carnival/dsh-base', '@carnival/dsh-web-app'],
  },
  headless: {
    bundles: ['@carnival/dsh-base', '@carnival/dsh-headless'],
  },
  sdk: {
    bundles: ['@carnival/dsh-base', '@carnival/dsh-sdk-app'],
  },
  'sdk-minimal': {
    bundles: ['@carnival/dsh-sdk-minimal'],
  },
}

/**
 * Installation-owned bundle tuples that an earlier release wrote into a shipped profile; app-boot replaces an exact
 * match with the {@link PROFILE_TEMPLATES} entry of the same name and leaves every other list untouched.
 */
export const INSTALLATION_OWNED_PROFILE_TUPLES: Readonly<Record<string, readonly string[]>> = {
  headless: ['@carnival/dsh-base', '@carnival/dsh-web-app', '@carnival/dsh-headless'],
}

/** Bundles an earlier release shipped and the installation no longer carries; app-boot removes them from profiles. */
export const RETIRED_BUNDLES: ReadonlySet<string> = new Set([
  // The Web composition mounts Schedule itself
  // ([upgrade guide](../../../../docs/upgrade-guide/v0.2.0-rc.2/schedule-bundle-retired/guide.md)).
  '@carnival/dsh-experimental-schedule-bundle',
  '@carnival/dsh-experimental-agent-team-profile',
  // The Web composition mounts personalized recommendations itself
  // ([upgrade guide](../../../../docs/upgrade-guide/v0.2.2-rc.5/personalized-recommendations-bundle-retired/guide.md)).
  '@carnival/dsh-personalized-recommendations',
])

/** The bundle list a `dsh plugin` init uses for a name with no shipped template. */
export const DEFAULT_PROFILE_BUNDLES: readonly string[] = ['@carnival/dsh-base']

/**
 * The bundles the dsh installation ships for a person to switch on: each a
 * runtime dependency of the installation that declares `dsh.bundle.patch`,
 * an `icon`, and `./locale/*.json` display metadata, selected by no shipped
 * template, and offered switched off by the plugin manager when listed
 * ([rationale](../../../../.agents/notes/implemented/process/2026-09-15-shipped-optional-bundles.md),
 * [admission](../../../../.agents/notes/implemented/architecture/2026-09-21-experimental-capabilities-as-optional-bundles.md)).
 */
export const OPTIONAL_BUNDLES: readonly string[] = [
  '@carnival/dsh-cua-bundle',
  '@carnival/dsh-cua-mcp',
  '@carnival/dsh-timeline',
  '@carnival/dsh-experimental-session-search',
  '@carnival/dsh-experimental-ralph-bundle',
  '@carnival/dsh-experimental-terminal-bundle',
  '@carnival/dsh-experimental-badge-skill-bundle',
  '@carnival/dsh-experimental-session-titles-bundle',
  '@carnival/dsh-experimental-voice-input-bundle',
  '@carnival/dsh-experimental-cot-translation-bundle',
  '@carnival/dsh-experimental-auto-review',
  '@carnival/dsh-experimental-inspector-profile',
  '@carnival/dsh-experimental-liveshare',
  '@carnival/dsh-experimental-plugin-market-bundle',
  '@carnival/dsh-experimental-tool-worktree',
  '@carnival/dsh-experimental-forge-bundle',
  '@carnival/dsh-deepseek-chat-import',
]

/** Public DSH release members available through the ordinary on-demand bundle installer. */
export const ON_DEMAND_BUNDLES: readonly string[] = [
  '@carnival/dsh-subagent-claude-code',
  '@carnival/dsh-subagent-codex',
]

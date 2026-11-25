# Loadable Harness plugin packages

This file is GENERATED from workspace manifests (`scripts/gen-plugin-packages.ts`) and verified fresh by `pnpm run verify-plugin-packages` (part of `doc-sync`); do not edit it by hand.

Every package below exports a Cordis plugin that a bundle patch can name in a Loader row. `Config` marks packages whose row accepts a `config` mapping; read the mounted schema from `Config.listConfigs` through `cordis_inspect` (filter by `name`, then query the `entry` id) when available, otherwise from the package README. Packages under `experimental` are pre-stable.

## account

| Package | Config | Description |
|---|---|---|
| `@carnival/dsh-deepseek-account` | yes | Sign in to DeepSeek and manage account credentials and notifications |
| `@carnival/dsh-deepseek-auth` | no | DeepSeek request authentication (ctx.deepseekAuth): choose between the API key and the account sign-in for one DeepSeek API request and release its headers |

## acp

| Package | Config | Description |
|---|---|---|
| `@carnival/dsh-acp` | yes | Automation-only Agent Client Protocol server for driving DeepSeek Harness agents over JSON-RPC stdio |

## api

| Package | Config | Description |
|---|---|---|
| `@carnival/dsh-api-gateway` | yes | Typert Remote Host dispatcher and Client API endpoint |
| `@carnival/dsh-api-job-controller` | yes | Job Remote observation stream and the reference-counted client job-output service |
| `@carnival/dsh-api-remote-access` | no | Method-level Remote access control for non-owner Peers and the per-Peer access snapshot |
| `@carnival/dsh-api-remotes` | no | Remote BFF assembly for application-selected Host capabilities |
| `@carnival/dsh-api-session-controller` | yes | Session Remote commands, cold reads, and live control transport |
| `@carnival/dsh-api-settings-controller` | yes | Remote owner for the configuration surfaces over the settings-domain seams |
| `@carnival/dsh-api-terminal-controller` | yes | Session-owned interactive terminals with shell discovery, screen recovery and typed Remote control |
| `@carnival/dsh-api-workspace-controller` | yes | Workspace Remote commands and reconnect-safe state transport |
| `@carnival/dsh-api-workspace-files` | yes | Workspace file service and Client resource provider: bounded reads, directory listing, and live metadata over the workspaceFiles Remote namespace |

## attachment

| Package | Config | Description |
|---|---|---|
| `@carnival/dsh-attachment-local` | yes | Private content-addressed DSH_HOME attachment storage |

## boot

| Package | Config | Description |
|---|---|---|
| `@carnival/dsh-config-editor` | no | Persist plugin configuration through profile patches and Loader reconciliation |
| `@carnival/dsh-hmr` | yes | Coordinated module and profile configuration hot reload |
| `@carnival/dsh-plugin-manager` | yes | Current-profile plugin and bundle management shared by dsh CLI, Web and agent tools |
| `@carnival/dsh-plugin-market` | yes | Configured plugin market indexes for the Web Discover page and the plugin_market agent tool |

## browser-use

| Package | Config | Description |
|---|---|---|
| `@carnival/dsh-browser-use` | no | Exclusive named browser-use provider registration |

## bundle

| Package | Config | Description |
|---|---|---|
| `@carnival/dsh-acp-app` | no | The dsh ACP profile bundle: automation-only JSON-RPC stdio and process lifecycle over dsh-base |
| `@carnival/dsh-headless` | yes | The dsh one-shot bundle: a direct core Agent/Session runner over dsh-base with no Host, HTTP, or browser layer |
| `@carnival/dsh-sdk-app` | yes | The dsh SDK profile bundle: stdio JSON-RPC serving and process lifecycle over dsh-base |
| `@carnival/dsh-web-app` | yes | The dsh browser-surface bundle: the web patch layer over dsh-base plus the runtime glue plugin (frontend dist serving, bash runtime variables, URL line) |

## client

| Package | Config | Description |
|---|---|---|
| `@carnival/dsh-client-config` | no | Types of browser-synced plugin configuration |
| `@carnival/dsh-client-connection` | yes | Authenticated RPC transport and generation lifecycle |
| `@carnival/dsh-client-file-upload` | no | Agent-scoped browser file upload, streaming intake, and staged receipt service |
| `@carnival/dsh-client-hmr` | yes | Web client graph and rebuilt-bundle delivery over an authenticated Remote stream |
| `@carnival/dsh-client-locale` | no | Locale plugin: Host-backed preference, extensible language catalog, browser fallback, and typed built-in dictionaries |
| `@carnival/dsh-client-modules` | no | Client module system, dual-face: node half composes the __DSH_BOOT__ entry graph (incremental dsh.client scan, bundle route, index tap, webPlugins service); browser half is the lazy-CJS module table the vendored cordis Loader consumes as its internal seam |
| `@carnival/dsh-client-product-analytics` | yes | Desktop product event collection and authenticated Host reporting |
| `@carnival/dsh-client-remote-connection` | yes | QR login, private login links, and owner-approved browser pairing |
| `@carnival/dsh-client-resources` | no | Unified client resource model: protocol-registered providers turn URL addresses into live values, consumed through the useResource global standard hook |
| `@carnival/dsh-client-script-realm` | no | Direct-reference script environments and Cordis-owned provider registration |
| `@carnival/dsh-client-script-realm-iframe` | no | Optional iframe ScriptRealm provider |
| `@carnival/dsh-client-shortcuts` | yes | Application keyboard command registry and physical-key routing |
| `@carnival/dsh-client-ui-account-notifications` | yes | Display account notifications and refresh localized content |
| `@carnival/dsh-client-ui-action-cards` | no | Action cards below completed answers: the shared card frame, kind bodies and the layer that hides the answer blocks offering them |
| `@carnival/dsh-client-ui-agent-preset` | no | Agent-preset surfaces: the default for later sessions, this session's seat, and the composition editor |
| `@carnival/dsh-client-ui-agent-team` | no | Web shared task board for a subagent tree |
| `@carnival/dsh-client-ui-approval` | no | Approval composer takeover over the scoped Remote Event waterfall |
| `@carnival/dsh-client-ui-attachment` | no | Dynamic attachment presentation plugin for conversation input, message-image, and trajectory image slots |
| `@carnival/dsh-client-ui-brand-official` | no | Official DeepSeek Harness brand artwork and click-triggered home background |
| `@carnival/dsh-client-ui-chat` | no | Chat Conversation target, node definitions, renderers, and details surface |
| `@carnival/dsh-client-ui-commands` | no | Client command surface: global directory cache, '/' source, three command UI kinds, popupSelect registry |
| `@carnival/dsh-client-ui-connector-account` | no | Connector account surfaces: the Host registry CLI app plugins and MCP connectors register their accounts with, and the Plugins page section, title tag, activation prompt, and notices it serves over one Remote |
| `@carnival/dsh-client-ui-context-menu` | yes | Typed, plugin-extensible context menus with captured targets and shared Web presentation |
| `@carnival/dsh-client-ui-conversation` | no | Target-neutral Conversation assembly, shell, composer, queue, and view navigation |
| `@carnival/dsh-client-ui-creation-recommendations` | no | Body of the personalized page card: location choice, creation progress, Undo and Restore |
| `@carnival/dsh-client-ui-deliverables` | no | Changed-files card with per-file comparison tabs, delivery cards, and clickable final-response file references for Web |
| `@carnival/dsh-client-ui-directory-picker-browse` | no | In-app directory browsing surface: the workspace directory-flow owner rendering the host's listing and creation primitives |
| `@carnival/dsh-client-ui-directory-picker-native` | no | Native directory-picker surface: the renderless workspace directory-flow occupant driving the local Desktop or Host OS chooser |
| `@carnival/dsh-client-ui-entry-customization` | no | Customize plugin entries in the left sidebar, conversation views, and right sidebar, and quickly create new plugins. |
| `@carnival/dsh-client-ui-file-actions` | no | Original-file saving and image clipboard actions for Web and Desktop |
| `@carnival/dsh-client-ui-file-drag` | no | Application-owned file drag transfers and localized failure feedback |
| `@carnival/dsh-client-ui-goal` | no | Session goal surface: GoalBar docked above the composer, read from the goal session projection |
| `@carnival/dsh-client-ui-input-trigger` | no | Input trigger pipeline: '/' and '@' detection, candidate menu, pick routing to registered sources |
| `@carnival/dsh-client-ui-jobs` | no | Session panel background-job list with on-demand streaming record panels |
| `@carnival/dsh-client-ui-layout` | no | Shell plugin: three-column AppFrame with drag handles, ctx.layout viewing-state service (navigation + panels) |
| `@carnival/dsh-client-ui-message-feedback` | no | The Web feedback surface: per-message Like/Dislike in the assistant-message action strip and the feedback dialog behind both ratings and /feedback, backed by the messageFeedback and sessionFeedback Host Remotes |
| `@carnival/dsh-client-ui-model-selection` | no | Model selection over the shared model catalog, Session projection, and session.selectModel |
| `@carnival/dsh-client-ui-open-in-app` | no | Session application submenus and native opening controls for directories and previewed files |
| `@carnival/dsh-client-ui-permission-presets` | no | Permission surfaces: a new-session default in General settings and a current-session /permission popup over the permissions projection |
| `@carnival/dsh-client-ui-plan` | no | Plan mode controls, persistent transcript plan cards, and sidebar Markdown previews |
| `@carnival/dsh-client-ui-plugin-manager` | yes | Plugin management for the dsh web client: the sidebar Plugins panel installs, enables, disables, retries, and composes installed plugin packages |
| `@carnival/dsh-client-ui-plugin-market` | no | Discover tab of the dsh web Plugins page: browse configured plugin market indexes and install their bundles |
| `@carnival/dsh-client-ui-plugin-recommendations` | no | Official plugin card body with catalog text and enabling |
| `@carnival/dsh-client-ui-reference` | no | Unified Web @file and @session reference source |
| `@carnival/dsh-client-ui-renderer` | no | Browser UI renderer: React slot bindings, ctx.uiRenderer, and the assembled application root |
| `@carnival/dsh-client-ui-schedule` | no | Host task management page and Session reminder catalog |
| `@carnival/dsh-client-ui-secret-mask` | no | Web display masking for recognized credentials in sent user text |
| `@carnival/dsh-client-ui-session` | no | Session Controller adapter for React and session-scoped slots |
| `@carnival/dsh-client-ui-session-panel` | no | Floating Session panel opened from the Session header, composed of sections that plugins register |
| `@carnival/dsh-client-ui-session-status` | no | Client Session status table over Host reading records: running state, the effective pending interaction, and read or unread reminders shared by clients of one Host |
| `@carnival/dsh-client-ui-settings` | no | Settings domain base plugin: shared configuration forms and the canonical settings slot-type contract |
| `@carnival/dsh-client-ui-settings-account` | yes | Manage DeepSeek login and open Platform billing pages |
| `@carnival/dsh-client-ui-settings-agent-loop` | no | Settings page of the agent loop on the dsh web client's Plugins page: the parallel tool-call cap of the agent-loop namespace |
| `@carnival/dsh-client-ui-settings-general` | no | Settings ownerless-copy and product onboarding plugin: the General section, shell trigger/header chrome content, settings dictionaries, and the versioned welcome notice |
| `@carnival/dsh-client-ui-settings-models` | yes | Models settings and shared product-onboarding dialogs over existing settings and credential joins |
| `@carnival/dsh-client-ui-settings-session-log` | no | General settings control for Session-log upload with DeepSeek API requests |
| `@carnival/dsh-client-ui-settings-shell` | no | Settings page of the shell executor on the dsh web client's Plugins page: the command timeout and the per-stream output cap of the shell namespace |
| `@carnival/dsh-client-ui-settings-subagent` | no | Settings page of Subagent delegation on the dsh web client's Plugins page: recursion depth, parallel capacity, and the models agents may choose for subagents |
| `@carnival/dsh-client-ui-settings-web-search` | no | Settings page for web search and fetch on the dsh web client's Plugins page: the Search and Fetch provider menus, the DeepSeek Search API result count, DeepSeek Search and Fetch API billing, and legacy Messages search settings |
| `@carnival/dsh-client-ui-shortcuts` | no | Keyboard shortcut reference, recording, and local preference editing |
| `@carnival/dsh-client-ui-sidebar` | no | Sidebar plugin: session multi-level tree, search, grouping, state dots |
| `@carnival/dsh-client-ui-sidebar-browser` | no | Sandboxed Web browser tabs for the right Sidebar |
| `@carnival/dsh-client-ui-sidebar-documentpreview` | yes | Extensible Sidebar previews for Office documents, spreadsheets, Markdown, code, images, PDF, HTML, and plain text |
| `@carnival/dsh-client-ui-sidebar-files` | no | The workspace files page and one workspace directory tree per pane beside docked and floating file previews, listed lazily over the workspaceFiles Remote namespace |
| `@carnival/dsh-client-ui-sidebar-right` | no | Right Sidebar: the docking surface's session-bound state, its panel and header expand control, and the navigation service over it |
| `@carnival/dsh-client-ui-sidebar-terminal` | no | Interactive shell tabs for the right Sidebar |
| `@carnival/dsh-client-ui-skill` | no | Web skill references and the dedicated skill tool row |
| `@carnival/dsh-client-ui-subagent` | no | Subagent conversation catalog, continuation routing UI, and '@' reference source |
| `@carnival/dsh-client-ui-theme` | yes | Theme plugin: Host bootstrap for the pre-plugin palette; ThemeRuntime for light/dark/system state and icon replacements; --dsw-* token styles and Appearance settings row |
| `@carnival/dsh-client-ui-tool` | no | Client Tool call-tree renderer and keyed per-tool presentation slot |
| `@carnival/dsh-client-ui-trajectory` | yes | Trajectory event ledger and timing overview with live tool-result inspection |
| `@carnival/dsh-client-ui-user-questions` | no | Web ask_user_question composer takeover and plan-review presentation UI |
| `@carnival/dsh-client-ui-workflow-run` | no | Durable workflow-run Conversation Node and nested member disclosure for dsh web |
| `@carnival/dsh-client-ui-workspace` | no | Workspace picker plugin: one WorkspacePicker registered into the sidebar and empty-state workspace slots |

## compaction

| Package | Config | Description |
|---|---|---|
| `@carnival/dsh-command-compact` | no | Human-facing slash command for explicit session compaction |
| `@carnival/dsh-compaction-basic` | yes | Token-meter-driven compaction policy and LLM summarization backend for the DeepSeek Harness |
| `@carnival/dsh-compaction-image-offload` | no | Durable image offload for image-capable routes: replace over-budget request images with placeholders and retry |
| `@carnival/dsh-compaction-tool-result-pruner` | yes | Replay-safe model-free head/middle/tail pruning for tool-result surface nodes |

## computer-use

| Package | Config | Description |
|---|---|---|
| `@carnival/dsh-computer-use` | no | Exclusive named computer-use provider registration |

## connector

| Package | Config | Description |
|---|---|---|
| `@carnival/dsh-cli-account` | yes | Host Service of CLI app plugins: each plugin registers its third-party CLI and account, and the Service gives the model the CLI's absolute path, runs one account operation at a time, and shows the account on the Plugins page |
| `@carnival/dsh-dingtalk` | yes | DingTalk plugin: the official dws CLI with its skills, and sign-in from the Plugins page |
| `@carnival/dsh-feishu` | yes | Feishu plugin: the official lark-cli with its embedded skills, and account connection from the Plugins page |
| `@carnival/dsh-linear` | yes | Linear plugin: Linear's hosted MCP server, authorized through the shared MCP manager |
| `@carnival/dsh-notion` | yes | Notion plugin: Notion's hosted MCP server, authorized through the shared MCP manager |
| `@carnival/dsh-tencent-docs` | yes | Tencent Docs plugin: Tencent Docs' hosted MCP server, authorized through the shared MCP manager |
| `@carnival/dsh-tencent-meeting` | yes | Tencent Meeting plugin: the official tmeet with its skill, and account connection from the Plugins page |
| `@carnival/dsh-upstream-skills` | yes | Host Service of CLI app plugins: each plugin registers its entry skill and a pinned directory of the vendor's GitHub repository, and the Service downloads that directory into the DSH cache and lists the entry skill over it |
| `@carnival/dsh-wecom` | yes | WeCom plugin: the official wecom-cli with its skills, and authorization from the Plugins page |

## context

| Package | Config | Description |
|---|---|---|
| `@carnival/dsh-agent-instructions` | yes | Workspace context loader for AGENTS.md/CLAUDE.md instruction files |
| `@carnival/dsh-file-reference-local` | yes | Local-filesystem ctx.fileReferences provider with bounded fuzzy indexes |
| `@carnival/dsh-harness-dev-context` | no | Model context naming the DeepSeek Harness checkout and Web GUI URL without letting either change the system prompt |
| `@carnival/dsh-session-reference` | yes | Cross-session snapshot references and durable untrusted model context (ctx.sessionReferenceResolver) |
| `@carnival/dsh-time-context` | yes | Durable per-step context with the current time and elapsed time |
| `@carnival/dsh-tmux-context` | yes | Opt-in durable per-step context with this agent's tmux pane and window location |

## core

| Package | Config | Description |
|---|---|---|
| `@carnival/dsh-agent` | no | Agent interface, registry, initiator scope, and event vocabulary for the DeepSeek Harness |
| `@carnival/dsh-agent-default-model` | yes | Default model selection shared by Agent entry points |
| `@carnival/dsh-agent-loop` | yes | The concrete agent loop plugin for the DeepSeek Harness |
| `@carnival/dsh-agent-tool-presentation` | yes | Deprecated tool presentation selector; use dsh-tool-executor-native or dsh-tool-executor-ptc directly |
| `@carnival/dsh-session` | no | Event-sourced session store for the DeepSeek Harness |
| `@carnival/dsh-system-prompt` | yes | System prompt assembly registry for the DeepSeek Harness |
| `@carnival/dsh-tool-executor-native` | no | Native tool execution plugin |
| `@carnival/dsh-tool-executor-ptc` | yes | PTC program execution, tool bindings, and generated SDK prompts |
| `@carnival/dsh-tools` | no | Tool registry and execution pipeline for the DeepSeek Harness |

## credentials

| Package | Config | Description |
|---|---|---|
| `@carnival/dsh-authorization` | no | Authorization seam (ctx.authorization): plugin-owned flows that obtain a credential through a conversation with the human |
| `@carnival/dsh-credentials-local` | yes | File-backed credentials provider ($DSH_HOME/.env under the live process environment) for the DeepSeek Harness |

## deliverables

| Package | Config | Description |
|---|---|---|
| `@carnival/dsh-deliverable-file-references` | no | Shared model guidance for file links and deliverables across DeepSeek Harness hosts |
| `@carnival/dsh-tool-present` | yes | Explicit workspace file delivery declarations for the DeepSeek Harness |
| `@carnival/dsh-workspace-changes` | yes | Per-turn workspace file changes recorded from git working-tree snapshots and whole-file captures, with per-file comparisons, for the DeepSeek Harness |

## document

| Package | Config | Description |
|---|---|---|
| `@carnival/dsh-office-kit` | yes | Shared Office rendering, conversion, and recalculation with bounded queues and caching |
| `@carnival/dsh-tool-office` | yes | Model-facing Office tools for rendering, converting, and recalculating documents |

## experimental

| Package | Config | Description |
|---|---|---|
| `@carnival/dsh-cua` | no | Shared Computer Use application objects, authorization, and logged tool execution |
| `@carnival/dsh-cua-macos` | yes | macOS native provider for shared Computer Use application and window objects |
| `@carnival/dsh-cua-mcp` | yes | Computer Use as an MCP stdio server: a dsh profile bundle that exposes cua_run_code to external agent harnesses |
| `@carnival/dsh-cua-windows` | yes | Windows C# computer use through shared application and window objects |
| `@carnival/dsh-deepseek-chat-import` | yes | Import DeepSeek chat exports as stored Sessions in a chosen Workspace |
| `@carnival/dsh-experimental-api-speech-to-text` | yes | Authenticated experimental speech transcription for browser clients |
| `@carnival/dsh-experimental-auto-review` | no | Per-tool LLM authorization review for the DeepSeek Harness Auto permission preset |
| `@carnival/dsh-experimental-browser-use-chrome-devtools-mcp` | yes | Experimental per-Session Chromium browser tools through chrome-devtools-mcp |
| `@carnival/dsh-experimental-browser-use-playwright-mcp` | yes | Experimental per-Session Chromium browser tools through @playwright/mcp |
| `@carnival/dsh-experimental-browser-use-stagehand-native` | yes | Experimental Stagehand browser tools with separately configured native models |
| `@carnival/dsh-experimental-claude-code-mods` | yes | Experimental bridge: load Claude Code mods (hooks modules) and run their hook chains on DeepSeek Harness extension points |
| `@carnival/dsh-experimental-client-ui-claude-code-mods` | no | Web band above the prompt for Claude Code mods: draws each session's mod tree and sends button presses back to the bridge |
| `@carnival/dsh-experimental-client-ui-cot-translation` | yes | Optional machine translation of expanded reasoning, preserving original Session text |
| `@carnival/dsh-experimental-client-ui-forge` | yes | Session panel entries for a Session's change requests, with a worktree option for new Sessions and its Host Remote |
| `@carnival/dsh-experimental-client-ui-stats` | no | Per-turn and per-step context, timing, cache, and tool-cost table contributed as an experimental conversation View |
| `@carnival/dsh-experimental-client-ui-voice-input` | no | Record speech and insert editable text into the conversation draft |
| `@carnival/dsh-experimental-computer-use-cua-driver-mcp` | yes | Experimental computer use through an installed Cua Driver MCP executable |
| `@carnival/dsh-experimental-computer-use-cua-driver-native` | no | Experimental computer-use provider embedding the Cua Driver native npm SDK |
| `@carnival/dsh-experimental-forge` | yes | Provider-neutral change request registry with shared status polling |
| `@carnival/dsh-experimental-forge-github` | yes | GitHub provider for the experimental forge registry, read through the GitHub CLI on the host |
| `@carnival/dsh-experimental-forge-session` | yes | Binds change requests to the Session that created, pushed, or tracked them |
| `@carnival/dsh-experimental-inspector` | yes | Experimental cross-realm CDP hub for Host debugging and Client Runtime inspection |
| `@carnival/dsh-experimental-session-inspector` | no | Experimental virtualized Session log and live Chat group/node inspectors |
| `@carnival/dsh-experimental-speech-to-text` | yes | Experimental speech recognition with independently selectable providers |
| `@carnival/dsh-experimental-speech-to-text-sensevoice` | yes | Local SenseVoice ONNX transcription with a managed sherpa-onnx process |
| `@carnival/dsh-experimental-subagent-oar` | yes | Experimental one-shot subagents through OAR runtimes |
| `@carnival/dsh-experimental-tool-subagent-oar` | yes | Delegate standalone tasks to configured external subagent providers |
| `@carnival/dsh-experimental-tool-worktree` | no | Model tool that creates and enters a new Git worktree |
| `@carnival/dsh-experimental-translator` | yes | Machine translation with reusable Session results |
| `@carnival/dsh-experimental-worktree` | yes | Creates a named Git worktree and changes the calling Session working directory |
| `@carnival/dsh-forge-gitlab` | yes | GitLab provider for the experimental forge registry, read and written through the GitLab CLI on the host |
| `@carnival/dsh-hooks-claude-code` | yes | Bridge plugin: run a Claude Code hooks.json / settings hook config on the DeepSeek Harness interception seams |
| `@carnival/dsh-hooks-codex` | yes | Bridge plugin: run a Codex hooks.json hook config on the DeepSeek Harness interception seams |
| `@carnival/dsh-session-title-all-prompts-llm` | yes | All-user-messages LLM provider plugin for DeepSeek Harness session titles |
| `@carnival/dsh-skill-badge` | no | Bundled dsh badge skill provider for DeepSeek Harness |
| `@carnival/dsh-terminal-panel` | yes | Persistent Agent terminals shown in the Web sidebar, plus read-only access to the user's terminals |
| `@carnival/dsh-timeline` | yes | Replace Todo in long-running sessions with task trees, a live progress bar, and timed reports |
| `@carnival/dsh-tool-ralph` | yes | Model-facing fresh-agent Ralph loop over the workflow and subagent seams |
| `@carnival/dsh-tool-session-query` | yes | Workspace-authorized model-facing session history search, trace, and event read tools |
| `@carnival/dsh-tool-terminal` | yes | Six model-facing persistent PTY tools with owner isolation and generic background-job integration |
| `@carnival/dsh-webhook` | no | Fire-and-forget webhook rule runtime that creates Workspace-backed DeepSeek Harness Sessions |
| `@carnival/dsh-webhook-github` | yes | Signed GitHub HTTP webhook adapter for the DeepSeek Harness webhook runtime |

## extensions

| Package | Config | Description |
|---|---|---|
| `@carnival/dsh-client-ui-cordis` | no | Cordis dynamic-plugin definition card: the keyed cordis_define tool row with its run/stop switch |
| `@carnival/dsh-cordis-client-runner` | no | Browser half of dynamic dual-half plugin packages: event subscription, closure evaluation, guard facade, and loader entries |
| `@carnival/dsh-cordis-host-runner` | yes | Dynamic package definition registry, host-half sandbox lifecycle, and invoke handler table for model-mounted dual-half packages |
| `@carnival/dsh-creation-recommendations` | no | Create persistent removable interface plugins from explicit conversation needs |
| `@carnival/dsh-plugin-recommendations` | no | Official plugin suggestions with explicit user activation and persistent conversation decisions |
| `@carnival/dsh-tool-cordis` | no | Read-only runtime API inspection for Harness plugin development |

## feedback

| Package | Config | Description |
|---|---|---|
| `@carnival/dsh-command-feedback` | no | Log-only session feedback: the record event, the sessionFeedback Host Remote, and the human-facing slash command |
| `@carnival/dsh-message-feedback` | yes | Canonical Session-log ratings and notes for finalized assistant messages |

## fs

| Package | Config | Description |
|---|---|---|
| `@carnival/dsh-fs-local` | yes | Local-filesystem implementation of the DeepSeek Harness filesystem seam (ctx.fs) |
| `@carnival/dsh-fs-observation-policy` | no | File-context policy plugin for the DeepSeek Harness — observed-state, read-before-edit, and version-guarded write/edit added over the ctx.fs provider seam through the fs/* event gate (no service API) |
| `@carnival/dsh-fs-sandbox` | yes | Sandbox-enforcing implementation of the DeepSeek Harness filesystem seam: fences write/edit by the per-call sandbox mode (read-only denies mutation, workspace-write contains it to the workspace + temp roots) while reads pass through |
| `@carnival/dsh-tool-fs` | yes | Model-facing filesystem tools (read, write, edit) over the DeepSeek Harness filesystem seam (ctx.fs) |
| `@carnival/dsh-tool-fs-search` | yes | Model-facing filesystem discovery tools (glob, grep) backed by the packaged ripgrep binary (@vscode/ripgrep) |
| `@carnival/dsh-tool-str-replace-editor` | yes | Model-facing view, create, literal replace, and line insert tool over the Harness filesystem service |

## goal

| Package | Config | Description |
|---|---|---|
| `@carnival/dsh-command-goal` | no | Human-facing slash command for persisted same-session goals |
| `@carnival/dsh-goal` | yes | Event-sourced same-session goal state and lifecycle service for the DeepSeek Harness |
| `@carnival/dsh-goal-round-driver` | no | Race-fenced same-session goal-round driver |
| `@carnival/dsh-tool-goal` | yes | Model-facing same-session goal tools with execution-time authority checks |

## guard

| Package | Config | Description |
|---|---|---|
| `@carnival/dsh-repeat-tool-reminder` | yes | Repeat-tool-call guard plugin: advisory reminders when an agent loops on identical tool calls |
| `@carnival/dsh-tool-call-timeout-policy` | no | Tool-call timeout policy: a tools/execute wrapper that arms a per-tool deadline on exec.signal and returns TOOL_TIMEOUT when it wins |

## host

| Package | Config | Description |
|---|---|---|
| `@carnival/dsh-host-authentication-desktop` | yes | Desktop private-channel authentication provider |
| `@carnival/dsh-host-authentication-token` | yes | Default token authentication provider and HTTP cookie exchange |
| `@carnival/dsh-host-directory-picker-auto` | no | Adaptive chooser of the directory-picker seam: resolves the host situation at boot and mounts the native or browse backend for the DeepSeek Harness web GUI host |
| `@carnival/dsh-host-directory-picker-browse` | yes | In-app browsing backend of the directory-picker seam (listing/creation primitives over the host filesystem) |
| `@carnival/dsh-host-directory-picker-native` | no | Native-OS-chooser backend of the directory-picker seam for the DeepSeek Harness web GUI host |
| `@carnival/dsh-host-frontend-static` | yes | SPA dist server for the Web shell: owns the webserver fallback seat, serving explicit index entries and static assets with traversal rejection and 404 misses |
| `@carnival/dsh-host-open-in-app` | yes | Host half of open-in-app: resolved application catalog, icons, and the launch endpoint as three webServer routes |
| `@carnival/dsh-host-plugin-inventory` | no | Read-only Remote projection of current Cordis Loader plugin state |
| `@carnival/dsh-host-product-telemetry-otel` | yes | Explicit product usage events exported through OpenTelemetry HTTP logs |
| `@carnival/dsh-host-webserver` | yes | Web route-registration plugin: HTTP and upgrade routes, per-Peer index transform taps, and static dist fallback |

## interaction

| Package | Config | Description |
|---|---|---|
| `@carnival/dsh-action-cards` | yes | Optional action cards below answers: card state, user actions and status notices |
| `@carnival/dsh-commands` | no | Plugin-owned human command registry for DeepSeek Harness UIs |
| `@carnival/dsh-permission-presets` | yes | User-facing permission presets (ctx.permissionPresets) for the DeepSeek Harness: one product-level Permissions select bundling the sandbox-mode and approval-policy knobs, written through to their own session events |
| `@carnival/dsh-tool-ask-user` | yes | Model-facing ask_user_question tool over the ctx.userQuestions seam |
| `@carnival/dsh-user-approval` | yes | User-approval seam (ctx.approval) for the DeepSeek Harness: one-shot permission decisions dispatched to composed answerers over the approval/request waterfall, fail-closed by default |
| `@carnival/dsh-user-questions` | no | Abstract user-questions seam (ctx.userQuestions) for asking the human during agent runs |

## jobs

| Package | Config | Description |
|---|---|---|
| `@carnival/dsh-jobs-local` | yes | Process-local implementation of the DeepSeek Harness background job registry seam |
| `@carnival/dsh-tool-jobs` | yes | Model-facing background job control tools (job_output, job_list, job_kill) over the ctx.jobs registry |

## llm

| Package | Config | Description |
|---|---|---|
| `@carnival/dsh-deepseek-llm-api-extensions` | no | Additive request-field registry for the official DeepSeek LLM API adapter |
| `@carnival/dsh-llm` | no | Provider-neutral LLM service interface for the DeepSeek Harness |
| `@carnival/dsh-llm-deepseek-account` | yes | DeepSeek account provider authentication and discovery |
| `@carnival/dsh-llm-deepseek-api-key` | yes | DeepSeek api-key provider authentication and discovery |
| `@carnival/dsh-llm-pi-ai` | yes | pi-ai-backed DeepSeek adapter for the DeepSeek Harness LLM seam (design-verification twin of dsh-llm-deepseek) |
| `@carnival/dsh-llm-retry` | yes | Provider-routed LLM request retry policy for the DeepSeek Harness |
| `@carnival/dsh-plugin-package-inventory-deepseek` | yes | Active Loader-backed plugin package inventory for official DeepSeek LLM API requests |
| `@carnival/dsh-token-meter` | yes | Replay-aware token measurement service (ctx.tokenMeter) for the DeepSeek Harness |

## lsp

| Package | Config | Description |
|---|---|---|
| `@carnival/dsh-lsp` | no | Abstract LSP capability seam (ctx.lsp) for the DeepSeek Harness — language-server provider registry keyed by branded id and extension mapping, order-independent per-query selection, normalized definition/references/implementation/hover requests and results, and the LspError taxonomy |
| `@carnival/dsh-lsp-stdio` | yes | Generic stdio language-server provider for the DeepSeek Harness LSP capability seam (ctx.lsp) — spawns configured servers, translates JSON-RPC, and serves transient-open goToDefinition/findReferences/goToImplementation/hover queries in the host filesystem namespace |
| `@carnival/dsh-tool-lsp` | yes | Model-facing lsp tool over the DeepSeek Harness LSP capability seam (ctx.lsp) — one read-only tool with goToDefinition/findReferences/goToImplementation/hover operations, one-based UTF-16 cursor coordinates, bounded location rendering, and hover normalization |

## mcp

| Package | Config | Description |
|---|---|---|
| `@carnival/dsh-mcp-account` | no | MCP connector accounts on the Plugins page: each connector's OAuth server in the connector account registry, authorized through the MCP manager |
| `@carnival/dsh-mcp-client` | yes | MCP client bridge: connects to MCP servers and registers their tools on ctx.tools |
| `@carnival/dsh-mcp-manager` | yes | MCP server manager: a settings-driven fleet of mcp-client instances with runtime status, restart, and probe |
| `@carnival/dsh-mcp-resources` | no | Scoped MCP resource discovery and reading through shared model tools |

## plan

| Package | Config | Description |
|---|---|---|
| `@carnival/dsh-plan-mode` | yes | Logged per-agent plan mode with deployment guidance, a direct slash command, and a user-reviewed exit |

## preset

| Package | Config | Description |
|---|---|---|
| `@carnival/dsh-agent-preset` | yes | Declare an Agent capability composition in Cordis YAML |
| `@carnival/dsh-agent-preset-registry` | yes | Declarative Agent preset registry and profile-backed editing |
| `@carnival/dsh-persona` | yes | Composition-authored deployment persona section for the DeepSeek Harness |

## ptc-runtime

| Package | Config | Description |
|---|---|---|
| `@carnival/dsh-ptc-runtime-node` | yes | Node PTC execution with async function, REPL, scoped REPL and VM Script evaluators and optional retained state |

## sandbox

| Package | Config | Description |
|---|---|---|
| `@carnival/dsh-sandbox-local` | yes | Local process-sandbox backends for the DeepSeek Harness sandbox seam: bwrap, the npm-distributed landlock-run launcher, macOS Seatbelt, or the Windows ACL restricted-token runner — functionally probed, fail-closed |
| `@carnival/dsh-sandbox-policy` | yes | Per-call sandbox policy resolver and current model context: deployment fallbacks plus each session's mode and workspace root, shared by every enforcing capability family |

## schedule

| Package | Config | Description |
|---|---|---|
| `@carnival/dsh-schedule` | yes | Host-wide durable reminders with shared management and delivery to existing or new Sessions |
| `@carnival/dsh-tool-schedule` | no | Model-facing scheduled task tools (schedule_create, schedule_list, schedule_view, schedule_edit) over the Host ctx.schedule service |

## sdk

| Package | Config | Description |
|---|---|---|
| `@carnival/dsh-sdk-jsonrpc-server` | yes | Stdio JSON-RPC server plugin for out-of-process DeepSeek Harness SDK clients |

## session

| Package | Config | Description |
|---|---|---|
| `@carnival/dsh-message-quotes` | no | Selected-text quotation and Markdown blockquote presentation |
| `@carnival/dsh-session-checkpoint-policy` | no | Semantic session durability checkpoints before model requests and tool side effects |
| `@carnival/dsh-session-log-deepseek` | yes | Incremental lossless session-log request extension for the official DeepSeek LLM API |
| `@carnival/dsh-session-persistence-jsonl` | yes | JSONL durable session persistence backend for the DeepSeek Harness |
| `@carnival/dsh-session-persistence-sqlite` | yes | SQLite Session event storage backend for the DeepSeek Harness |
| `@carnival/dsh-session-projection` | no | Session-projection seam: the merge-extensible projection type table, the provider contract, and the ctx.sessionProjections registry serving whole current values of log-derived per-session state |
| `@carnival/dsh-session-projection-cache` | yes | Persisted projection cache (ctx.sessionProjectionCache): durable per-session checkpoint records on the session_projcache storage domain (per-record layout), throttled write-behind, and the cached listing read |
| `@carnival/dsh-session-stats` | no | Whole-log conversation counts and wall times projection (sessionStats) for the DeepSeek Harness |
| `@carnival/dsh-session-telemetry-otel` | yes | Feedback-authorized Session logs over byte-bounded OpenTelemetry HTTP requests |
| `@carnival/dsh-session-title` | yes | Log-backed session title service and provider registry for the DeepSeek Harness |
| `@carnival/dsh-session-title-first-prompt-llm` | yes | First-message LLM provider plugin for DeepSeek Harness session titles |
| `@carnival/dsh-session-turn-outline` | no | Whole-log turn outline projection (turnOutline) for the DeepSeek Harness |
| `@carnival/dsh-tool-session-control` | no | Send model-authored messages to ordinary Web Sessions with acceptance receipts |
| `@carnival/dsh-working-directory` | yes | Session working directories with durable changes and model context |

## session-query

| Package | Config | Description |
|---|---|---|
| `@carnival/dsh-session-log-export` | yes | Web Session-log ZIP export and Session Markdown copy |
| `@carnival/dsh-session-query-sqlite` | yes | Concrete ctx.sessionQuery backend with SQLite FTS5 search |
| `@carnival/dsh-tool-session-list` | no | Read-only paginated Workspace and Session metadata tools |

## settings

| Package | Config | Description |
|---|---|---|
| `@carnival/dsh-settings` | no | Abstract user-settings seam (ctx.settings) for the DeepSeek Harness |

## shell

| Package | Config | Description |
|---|---|---|
| `@carnival/dsh-bash-local` | yes | Local-subprocess implementation of the DeepSeek Harness bash executor seam |
| `@carnival/dsh-bash-sandbox` | yes | Sandbox-consuming implementation of the DeepSeek Harness bash executor seam (confines every command via ctx.sandbox, reports denial/enforcement result facts) |
| `@carnival/dsh-pwsh-local` | yes | Local PowerShell implementation of the DeepSeek Harness bash executor seam |
| `@carnival/dsh-pwsh-sandbox` | yes | Sandbox-consuming implementation of the DeepSeek Harness PowerShell executor seam (confines every command via ctx.sandbox, reports denial/enforcement result facts) |
| `@carnival/dsh-shell-env` | yes | Tool-independent managed DSH_* shell environment registry |
| `@carnival/dsh-tool-bash` | yes | Model-facing bash tool with optional generic background-job and sandbox-escalation support |
| `@carnival/dsh-tool-bash-persistent` | yes | Model-facing owner-scoped persistent Bash tool backed by the Harness PTY service |
| `@carnival/dsh-tool-pwsh` | yes | Model-facing pwsh tool over the bash executor seam |
| `@carnival/dsh-tool-pwsh-persistent` | yes | Model-facing owner-scoped persistent PowerShell tool backed by the Harness PTY service |

## skill

| Package | Config | Description |
|---|---|---|
| `@carnival/dsh-skill` | yes | Agent skill provider registry for the DeepSeek Harness |
| `@carnival/dsh-skill-filesystem` | yes | Local filesystem skill provider for the DeepSeek Harness |
| `@carnival/dsh-skill-office` | yes | Bundled Word, PowerPoint, and Excel workflows and Office operation guidance |
| `@carnival/dsh-tool-skill` | yes | Model-facing skill loading tool for the DeepSeek Harness |
| `@carnival/dsh-tool-workspace-dependencies` | yes | The load_workspace_dependencies tool: absolute paths into a bundled Python, Node.js, and pnpm payload |

## spill

| Package | Config | Description |
|---|---|---|
| `@carnival/dsh-spill-local` | yes | Local-filesystem implementation of the DeepSeek Harness spill storage seam (private session-scoped files) |
| `@carnival/dsh-spill-policy` | yes | Token-budgeted tool-result retention with recoverable text and image paths |

## ssh

| Package | Config | Description |
|---|---|---|
| `@carnival/dsh-fs-ssh` | no | Filesystem provider over the shared POSIX SSH helper |
| `@carnival/dsh-sandbox-ssh` | no | Remote POSIX sandbox argv provider over the shared SSH helper |
| `@carnival/dsh-ssh` | yes | Shared OpenSSH connection and versioned POSIX remote helper |
| `@carnival/dsh-subprocess-ssh` | no | Subprocess and terminal provider over the shared POSIX SSH helper |

## storage

| Package | Config | Description |
|---|---|---|
| `@carnival/dsh-storage` | no | Storage hub (ctx.storage): named backend registry plus mounted data-form facilities for the DeepSeek Harness |
| `@carnival/dsh-storage-domain` | yes | Domain data form (ctx.storage.domain): schema-validated, event-emitting KV domains over storage backends for the DeepSeek Harness |
| `@carnival/dsh-storage-json` | yes | JSON file KV storage backend for the DeepSeek Harness storage hub |
| `@carnival/dsh-storage-sqlite` | yes | SQLite storage backend (kv facet) for the DeepSeek Harness storage hub |

## subagent

| Package | Config | Description |
|---|---|---|
| `@carnival/dsh-subagent` | yes | Abstract subagent seam (ctx.subagents): named-provider registry for delegating to child agents |
| `@carnival/dsh-subagent-acp` | yes | Out-of-process ACP subagent backend: drives a child agent in a spawned subprocess over the Agent Client Protocol |
| `@carnival/dsh-subagent-claude-code` | yes | One-shot Claude Code subagent provider over the official Agent SDK |
| `@carnival/dsh-subagent-codex` | yes | One-shot Codex subagent provider over the official app-server protocol |
| `@carnival/dsh-subagent-dsh-sdk` | yes | Out-of-process SDK subagent backend: drives a child DeepSeek Harness runtime subprocess over stdio JSON-RPC through the TypeScript SDK client |
| `@carnival/dsh-subagent-fork-in-process` | yes | In-process fork subagent backend: runs a child agent seeded with a prefix of the parent's log |
| `@carnival/dsh-subagent-spawn-in-process` | yes | In-process spawn subagent backend: runs a fresh child agent on ctx.agents |
| `@carnival/dsh-tool-subagent` | yes | Model-facing subagent delegation tool over the ctx.subagents seam |
| `@carnival/dsh-tool-subagent-control` | yes | Shared messaging, interruption, and child discovery tools for subagents |

## subprocess

| Package | Config | Description |
|---|---|---|
| `@carnival/dsh-subprocess-local` | no | Local-subprocess implementation of the DeepSeek Harness subprocess seam |

## team

| Package | Config | Description |
|---|---|---|
| `@carnival/dsh-agent-team` | yes | Shared task DAG and change waits for subagent trees |
| `@carnival/dsh-tool-agent-team` | yes | Scoped model-facing Agent Teams tools over ctx.agentTeams |

## telemetry

| Package | Config | Description |
|---|---|---|
| `@carnival/dsh-otel` | no | Cordis service for independent ordinary-event and byte-bounded Session-log OTLP channels |

## terminal

| Package | Config | Description |
|---|---|---|
| `@carnival/dsh-terminal` | no | Persistent PTY session seam for the DeepSeek Harness — owner-scoped ids, backend registry, interactive sends, reads, signals, and awaited cleanup |
| `@carnival/dsh-terminal-bash` | yes | Persistent shell PTY backend over the DeepSeek Harness subprocess terminal primitive |

## test-support

| Package | Config | Description |
|---|---|---|
| `@carnival/dsh-llm-replay` | yes | Replay LLM plugin: short-circuits llm/stream with model chunks reconstructed from a recorded session JSONL (keyless snapshot tests) |

## todo

| Package | Config | Description |
|---|---|---|
| `@carnival/dsh-tool-todo` | yes | Model-facing todo_write tool over the DeepSeek Harness event-sourced session log |

## typert

| Package | Config | Description |
|---|---|---|
| `@carnival/dsh-typert-loader` | yes | Loader integration for generated Typert package contributions |

## web

| Package | Config | Description |
|---|---|---|
| `@carnival/dsh-tool-web` | yes | Model-facing web tools (web_search, web_fetch) over the DeepSeek Harness web capability seam (ctx.web) |
| `@carnival/dsh-web` | yes | Abstract web access capability seam (ctx.web) for the DeepSeek Harness — search/fetch provider registry, registration-order-independent selection, request/result vocabulary, and the WebError taxonomy |
| `@carnival/dsh-web-deepseek-api` | yes | DeepSeek Search and Fetch API providers for the DeepSeek Harness web capability seam (ctx.web) |
| `@carnival/dsh-web-fetch-http` | yes | Anonymous HTTP(S) fetch provider for the DeepSeek Harness web capability seam (ctx.web) |
| `@carnival/dsh-web-search-deepseek` | yes | DeepSeek-backed search provider (native web_search via the Anthropic-compatible API) for the DeepSeek Harness web capability seam (ctx.web) |

## workflow

| Package | Config | Description |
|---|---|---|
| `@carnival/dsh-tool-workflow` | yes | Model-facing workflow tool: run a JavaScript orchestration script over ctx.workflowEngine |
| `@carnival/dsh-workflow-ptc` | yes | Workflow orchestration in the shared sandboxed Node PTC runtime |

## workspace

| Package | Config | Description |
|---|---|---|
| `@carnival/dsh-workspace` | no | Workspace entity registry (ctx.workspaceRegistry): durable workspace records with validated session attachment over the domain data form for the DeepSeek Harness |

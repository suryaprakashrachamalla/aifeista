/**
 * Test-only client roster of the assembled viewer page.
 *
 * A served viewer page is the owner's own Web shell with the owner's client plugins; this roster is the
 * subset the viewer page specs need: the wire plugin, the Gateway, remote access, the Session/Workspace controllers,
 * Session status, the menu service, presentation plugins, and the Liveshare `./client` face, which mounts the viewer
 * surfaces when the carrier names a share. Every row is the package's own `/client` face registered as a module-table
 * factory. It must be imported only after {@link prepareViewerRemoteFaces} answered the generated
 * `/remote` faces these plugins import.
 *
 * @module @carnival/dsh-experimental-liveshare/tests/viewer-page-roster
 */

import * as apiGateway from '@carnival/dsh-api-gateway/client'
import * as apiRemoteAccess from '@carnival/dsh-api-remote-access/client'
import * as apiRemotes from '@carnival/dsh-api-remotes/client'
import * as apiSessionController from '@carnival/dsh-api-session-controller/client'
import * as apiWorkspaceController from '@carnival/dsh-api-workspace-controller/client'
import * as clientConnection from '@carnival/dsh-client-connection/client'
import * as clientFileUpload from '@carnival/dsh-client-file-upload/client'
import * as clientLocale from '@carnival/dsh-client-locale/client'
import * as clientModules from '@carnival/dsh-client-modules/client'
import * as clientResources from '@carnival/dsh-client-resources/client'
import * as clientShortcuts from '@carnival/dsh-client-shortcuts/client'
import * as uiAttachment from '@carnival/dsh-client-ui-attachment/client'
import * as uiChat from '@carnival/dsh-client-ui-chat/client'
import * as uiContextMenu from '@carnival/dsh-client-ui-context-menu/client'
import * as uiConversation from '@carnival/dsh-client-ui-conversation/client'
import * as uiDeliverables from '@carnival/dsh-client-ui-deliverables/client'
import * as uiFileDrag from '@carnival/dsh-client-ui-file-drag/client'
import * as uiLayout from '@carnival/dsh-client-ui-layout/client'
import * as uiRenderer from '@carnival/dsh-client-ui-renderer/client'
import * as uiSession from '@carnival/dsh-client-ui-session/client'
import * as uiSessionStatus from '@carnival/dsh-client-ui-session-status/client'
import * as uiSettings from '@carnival/dsh-client-ui-settings/client'
import * as uiSidebar from '@carnival/dsh-client-ui-sidebar/client'
import * as uiSidebarRight from '@carnival/dsh-client-ui-sidebar-right/client'
import * as uiTheme from '@carnival/dsh-client-ui-theme/client'
import * as uiTool from '@carnival/dsh-client-ui-tool/client'
import * as uiTrajectory from '@carnival/dsh-client-ui-trajectory/client'
import * as uiWorkspace from '@carnival/dsh-client-ui-workspace/client'
import * as typertRegistry from '@carnival/dsh-typert-registry/client'
import * as liveshare from '../src/client/index.ts'

/** One module-table row: the entry id the Loader creates, and that plugin's client exports. */
export interface ViewerPageRosterRow {
  readonly id: string
  readonly exports: Readonly<Record<string, unknown>>
}

/** Entry id of the module-system bootstrap whose exports the facade hands to the module system. */
export const VIEWER_PAGE_BOOTSTRAP_ID = '@carnival/dsh-client-modules'

/** The viewer page roster; Cordis composes these entries by service injection, so order is irrelevant. */
export const VIEWER_PAGE_ROSTER: readonly ViewerPageRosterRow[] = [
  { id: '@carnival/dsh-typert-registry', exports: typertRegistry },
  { id: '@carnival/dsh-experimental-liveshare', exports: liveshare },
  { id: VIEWER_PAGE_BOOTSTRAP_ID, exports: clientModules },
  { id: '@carnival/dsh-client-connection', exports: clientConnection },
  { id: '@carnival/dsh-api-gateway', exports: apiGateway },
  { id: '@carnival/dsh-api-remote-access', exports: apiRemoteAccess },
  { id: '@carnival/dsh-api-remotes', exports: apiRemotes },
  { id: '@carnival/dsh-api-session-controller', exports: apiSessionController },
  { id: '@carnival/dsh-api-workspace-controller', exports: apiWorkspaceController },
  { id: '@carnival/dsh-client-file-upload', exports: clientFileUpload },
  { id: '@carnival/dsh-client-locale', exports: clientLocale },
  { id: '@carnival/dsh-client-resources', exports: clientResources },
  { id: '@carnival/dsh-client-shortcuts', exports: clientShortcuts },
  { id: '@carnival/dsh-client-ui-renderer', exports: uiRenderer },
  { id: '@carnival/dsh-client-ui-theme', exports: uiTheme },
  { id: '@carnival/dsh-client-ui-settings', exports: uiSettings },
  { id: '@carnival/dsh-client-ui-layout', exports: uiLayout },
  { id: '@carnival/dsh-client-ui-session', exports: uiSession },
  { id: '@carnival/dsh-client-ui-session-status', exports: uiSessionStatus },
  { id: '@carnival/dsh-client-ui-sidebar', exports: uiSidebar },
  { id: '@carnival/dsh-client-ui-conversation', exports: uiConversation },
  { id: '@carnival/dsh-client-ui-chat', exports: uiChat },
  { id: '@carnival/dsh-client-ui-context-menu', exports: uiContextMenu },
  { id: '@carnival/dsh-client-ui-tool', exports: uiTool },
  { id: '@carnival/dsh-client-ui-attachment', exports: uiAttachment },
  { id: '@carnival/dsh-client-ui-file-drag', exports: uiFileDrag },
  { id: '@carnival/dsh-client-ui-sidebar-right', exports: uiSidebarRight },
  { id: '@carnival/dsh-client-ui-trajectory', exports: uiTrajectory },
  { id: '@carnival/dsh-client-ui-deliverables', exports: uiDeliverables },
  { id: '@carnival/dsh-client-ui-workspace', exports: uiWorkspace },
]

/**
 * AI Carnival Horizontal Bottom Glass Dock:
 * Replaces the traditional left sidebar with a 2-tier up-and-down dock anchored
 * totally at the bottom of the window:
 *   - Expandable Studios & Acts shelf (shown above the bar when toggled open)
 *   - Always-accessible bottom Liquid Glass command dock (Circus Wheel identity,
 *     New Act pill, Studios drawer toggle, horizontal navigation pills, footer
 *     actions, and Preferences).
 */
import { useEffect, useId, useMemo, useRef, useState } from 'react'
import type { PointerEvent as ReactPointerEvent } from 'react'
import clsx from 'clsx'
import {
  FishLogo, IconAppsAddOutlineRegular, IconNewChatOutlineMedium,
  IconPanelLeftOutlineRegular, IconPinFillRegular, focusWithoutRing, Menu, ShortcutKeys, Toast,
  Tooltip, writeClipboard,
} from '@carnival/dsh-client-ui-primitives'
import type { InjectFace, PropsRenderFactories, PropsRenderSlots, PropsRuntime } from '@carnival/dsh-client-ui-slots'
import type {
  SidebarPanelActionsOwnerProps, SidebarPanelMetadata, SidebarRootComponentProps, SidebarRootInjected, SidebarSectionHookContext,
  SidebarSectionOwnerProps,
} from './contract/slots.ts'
import { FIXED_PANEL_ID } from './panel-pins.ts'
import { createScrollHandoff } from './scroll-handoff.ts'
import css from './SidebarRoot.module.css'

/** Format complete-build metadata for the local brand badge. */
function localBuildVersion(): string | undefined {
  const version = process.env.DSH_CLIENT_VERSION
  if (version === undefined) return undefined
  const commit = process.env.DSH_CLIENT_COMMIT_HASH
  return version
    + (commit === undefined ? '' : `-${commit}`)
    + (process.env.DSH_CLIENT_GIT_DIRTY === 'true' ? '-dirty' : '')
}

type PanelRowProps =
  Pick<SidebarPanelMetadata, 'id' | 'label'>
  & Pick<SidebarSectionOwnerProps, 'wide'>
  & Pick<PropsRuntime<'sidebar'>, 'usePanelInfo'>
  & Pick<InjectFace<SidebarRootInjected>, 'selectPanel' | 'trackEntryClick' | 'setPinned'>
  & { readonly unpinLabel: string; readonly unpinTooltip: string; readonly canUnpin: boolean }
  & PropsRenderFactories

/** Analytics category of an official panel. */
function officialEntry(id: string): 'plugin' | 'cron' | undefined {
  if (id === 'plugins') return 'plugin'
  return id === 'schedules' ? 'cron' : undefined
}

/** Each panel row subscribes only to its own selection state. */
function PanelRow({
  id, label, wide, unpinLabel, unpinTooltip, canUnpin, usePanelInfo, selectPanel, setPinned, trackEntryClick, renderFactorySlot,
}: PanelRowProps) {
  const active = usePanelInfo(info => info.activePanelId === id)
  const trailing = useId()
  return (
    <div className={clsx(css.panelEntry, wide && css.panelEntryWide, active && css.panelEntryActive)}>
      <button
        type="button"
        className={css.panelRow}
        aria-label={label}
        aria-describedby={trailing}
        aria-current={active ? 'page' : undefined}
        onClick={() => {
          const entry = officialEntry(id)
          if (entry !== undefined) trackEntryClick({ menu_name: entry, position: 'sidebar' })
          selectPanel(id)
        }}
      >
        <span className={css.panelGlyph} aria-hidden="true">
          {renderFactorySlot('sidebar.panel.icon', { id, size: 15, active })}
        </span>
        <span className={css.panelTitle}>
          {label}
        </span>
        <span id={trailing} className={css.panelTrailing}>{renderFactorySlot('sidebar.panel.trailing', { id, wide: true, active })}</span>
      </button>
      {canUnpin && <Tooltip label={unpinTooltip} side="top" align="end" delayMs={500} portal>
        <button type="button" className={css.panelUnpin} aria-label={unpinLabel} onClick={(event) => {
          const navigation = event.currentTarget.closest('nav') as HTMLElement
          const more = navigation.querySelector('button[aria-haspopup="menu"]') as HTMLButtonElement
          if (more !== null) focusWithoutRing(more)
          setPinned(id, false)
        }}>
          <IconPinFillRegular size={12} />
        </button>
      </Tooltip>}
    </div>
  )
}

type PanelMoreMenuProps =
  Pick<SidebarSectionOwnerProps, 'wide'>
  & Omit<SidebarPanelActionsOwnerProps, 'onDismiss'>
  & PropsRenderSlots<'sidebar.panellist.actions'>
  & Pick<InjectFace<SidebarRootInjected>, 'trackEntryClick'>
  & { readonly label: string }

/** The panel list's More launcher. */
function PanelMoreMenu({ label, entries, onOpen, onPinnedChange, trackEntryClick, renderSlot }: PanelMoreMenuProps) {
  const [open, setOpen] = useState(false)
  const onDismiss = (): void => { setOpen(false) }
  return (
    <Menu open={open} onClose={onDismiss} portal autoFocus className={css.moreAnchor} listClassName={css.moreMenu}
      anchor={(
        <button type="button" className={css.panelRow} aria-label={label} aria-haspopup="menu" aria-expanded={open}
          onClick={() => {
            trackEntryClick({ menu_name: 'more', position: 'sidebar' })
            setOpen(value => !value)
          }}>
          <span className={css.panelGlyph} aria-hidden="true"><IconAppsAddOutlineRegular size={15} /></span>
          <span className={css.panelTitle}>{label}</span>
        </button>
      )}>
      {renderSlot('sidebar.panellist.actions', { entries, onOpen, onPinnedChange, onDismiss })}
    </Menu>
  )
}

/**
 * Render the bottom-docked AI Carnival navigation & Studios drawer.
 */
export function SidebarRoot({
  collapsed,
  startSession,
  toggleSidebar,
  selectPanel,
  setPinned,
  trackEntryClick,
  useRemoteAccess,
  usePanels,
  useEntryPins,
  useEntryActions,
  useShortcuts,
  usePanelInfo,
  t,
  renderSlot,
  renderFactorySlot,
}: SidebarRootComponentProps) {
  const panels = usePanels(snapshot => snapshot)
  const unpinned = useEntryPins(ids => ids)
  const activePanelId = usePanelInfo(info => info.activePanelId)
  const hasEntryActions = useEntryActions(registered => registered)
  const shortcut = useShortcuts(rows => rows.find(row => row.id === 'sidebar.left.toggle'))
  const newShortcut = useShortcuts(rows => rows.find(row => row.id === 'session.new'))
  const canStartSession = useRemoteAccess('session/create')
  const toggleLabel = collapsed ? t('toggle.open') : t('toggle.collapse')
  const [scrollHandoff] = useState(createScrollHandoff)
  const sectionHookContext = useMemo<SidebarSectionHookContext>(
    () => ({ useSectionScroller: scrollHandoff.useSectionScroller }), [scrollHandoff],
  )
  const openNewSession = (): void => {
    trackEntryClick({ menu_name: 'new_session', position: 'sidebar' })
    startSession()
  }

  const column = useRef<HTMLDivElement>(null)
  const [pointerInside, setPointerInside] = useState(false)
  const trackPointer = (event: ReactPointerEvent<HTMLDivElement>): void => {
    const rect = event.currentTarget.getBoundingClientRect()
    setPointerInside(event.clientX >= rect.left && event.clientX < rect.right
      && event.clientY >= rect.top && event.clientY < rect.bottom)
  }

  const buildVersion = localBuildVersion()
  const [copiedToast, setCopiedToast] = useState<number | null>(null)
  const [agentManagerOpen, setAgentManagerOpen] = useState(false)
  useEffect(() => {
    const onState = (ev: Event): void => {
      const detail = (ev as CustomEvent<{ open?: boolean }>).detail
      if (detail && typeof detail.open === 'boolean') setAgentManagerOpen(detail.open)
    }
    window.addEventListener('carnival:agent-manager-state', onState)
    return () => { window.removeEventListener('carnival:agent-manager-state', onState) }
  }, [])
  const copyVersion = (version: string) => (): void => {
    void writeClipboard(version).then((ok) => {
      if (ok) setCopiedToast(seq => (seq ?? 0) + 1)
    })
  }

  return (
    <div
      ref={column}
      data-bottom-sidebar="true"
      className={clsx(css.root, collapsed && css.collapsed, !pointerInside && css.quietBars)}
      onPointerEnter={trackPointer}
      onPointerMove={trackPointer}
      onPointerLeave={() => { setPointerInside(false) }}
    >
      {/* Expandable Studios & Acts shelf opening directly above the bottom dock bar */}
      {!collapsed && (
        <div className={css.studiosTray}>
          <div className={css.scrollRegion}>
            <div
              ref={scrollHandoff.bindViewportRef}
              className={css.scrollArea}
              data-sidebar-scrollport=""
            >
              <div className={css.regionArea}>
                {renderSlot('sidebar.workspaces', {
                  wide: true,
                  expandSidebar: () => { if (collapsed) toggleSidebar() },
                }, { hookContext: sectionHookContext })}
              </div>
            </div>
          </div>
        </div>
      )}

      {/* Primary horizontal Liquid Glass command bar docked totally at the bottom */}
      <div className={css.dockBar}>
        <div className={css.dockLeft}>
          <div className={css.logoRow}>
            {(() => {
              const identity = (
                <span className={css.brandIdentity} aria-hidden="true">
                  <span className={css.brandMark}>
                    {renderSlot('sidebar.brand.mark', { size: 22 }, {
                      fallback: buildVersion === undefined
                        ? <FishLogo size={22} />
                        : <span className={css.copyVersion} data-copy-version onClick={copyVersion(buildVersion)}><FishLogo size={22} /></span>,
                    })}
                  </span>
                  <span className={css.brandName}>
                    {renderSlot('sidebar.brand.name', {}, {
                      fallback: <span className={css.fallbackBrandName}>{t('brand.localBuild')}</span>,
                    })}
                  </span>
                </span>
              )
              return !canStartSession
                ? <span className={css.brand}>{identity}</span>
                : (
                  <button
                    type="button"
                    className={css.brand}
                    aria-label={t('session.new.label')}
                    aria-keyshortcuts={newShortcut?.aria}
                    onClick={(event) => {
                      if ((event.target as Element).closest('[data-copy-version]') !== null) return
                      openNewSession()
                    }}
                  >
                    {identity}
                  </button>
                )
            })()}
          </div>

          <span className={css.dockDivider} aria-hidden="true" />

          {canStartSession && (
            <div className={css.newSessionRow}>
              {renderSlot('sidebar.session.action', { wide: true }, {
                children: (
                  <Tooltip label={t('session.new.label')} shortcutKeys={newShortcut?.keys} delayMs={500} side="top">
                    <button
                      type="button"
                      className={css.newSession}
                      aria-label={t('session.new.label')}
                      aria-keyshortcuts={newShortcut?.aria}
                      onClick={openNewSession}
                    >
                      <span className={css.newSessionContent}>
                        <IconNewChatOutlineMedium size={14} />
                        <span className={css.newSessionLabel}>{t('session.new')}</span>
                      </span>
                      {newShortcut !== undefined && newShortcut.keys.length > 0 && (
                        <span className={css.newSessionShortcut} aria-hidden="true">
                          <ShortcutKeys keys={newShortcut.keys} />
                        </span>
                      )}
                    </button>
                  </Tooltip>
                ),
              })}
            </div>
          )}

          <Tooltip label={toggleLabel} shortcutKeys={shortcut?.keys} delayMs={500} side="top">
            <button
              data-sidebar-toggle
              type="button"
              className={clsx(css.studiosToggle, !collapsed && css.studiosToggleOpen)}
              aria-label={toggleLabel}
              aria-keyshortcuts={shortcut?.aria}
              onClick={() => { toggleSidebar() }}
            >
              <IconPanelLeftOutlineRegular size={14} />
              <span>{toggleLabel}</span>
            </button>
          </Tooltip>

          <button
            data-agent-manager-btn
            type="button"
            className={clsx(css.agentManagerBtn, agentManagerOpen && css.agentManagerBtnActive)}
            aria-label="Agent Manager"
            aria-pressed={agentManagerOpen}
            onClick={() => {
              window.dispatchEvent(new Event('carnival:toggle-agent-manager'))
            }}
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
              <rect x="3" y="3" width="7" height="7" rx="2" />
              <rect x="14" y="3" width="7" height="7" rx="2" />
              <rect x="3" y="14" width="7" height="7" rx="2" />
              <path d="M17.5 14v7M14 17.5h7" />
            </svg>
            <span>Agent Manager</span>
          </button>
        </div>

        {panels.length > 0 && (
          <nav className={css.panelList} aria-label={t('panels.label')}>
            {panels.filter(({ id }) => id === FIXED_PANEL_ID || !hasEntryActions || !unpinned.includes(id)).map(({ id, label }) => (
              <PanelRow
                key={id}
                id={id}
                label={label}
                wide
                unpinLabel={t('panels.unpin', { name: label })}
                unpinTooltip={t('panels.unpinTooltip')}
                canUnpin={hasEntryActions && id !== FIXED_PANEL_ID}
                setPinned={setPinned}
                usePanelInfo={usePanelInfo}
                selectPanel={selectPanel}
                trackEntryClick={trackEntryClick}
                renderFactorySlot={renderFactorySlot}
              />
            ))}
            {hasEntryActions && (
              <PanelMoreMenu
                wide
                label={t('panels.more')}
                entries={panels.filter(({ id }) => id !== FIXED_PANEL_ID)
                  .map(({ id, label }) => ({ id, label, pinned: !unpinned.includes(id), active: id === activePanelId }))}
                onOpen={(id) => {
                  const entry = officialEntry(id)
                  if (entry !== undefined) trackEntryClick({ menu_name: entry, position: 'sidebar_more' })
                  const panel = panels.find(candidate => candidate.id === id)
                  if (panel !== undefined) selectPanel(panel.id)
                }}
                onPinnedChange={setPinned}
                trackEntryClick={trackEntryClick}
                renderSlot={renderSlot}
              />
            )}
          </nav>
        )}

        <div className={css.footArea}>
          <div className={css.footerActions}>
            {renderSlot('sidebar.footer.action', { wide: true })}
          </div>
          <div className={css.settingsArea}>
            {renderSlot('sidebar.settings', { wide: true })}
          </div>
        </div>
      </div>

      {copiedToast !== null && (
        <Toast key={copiedToast} tone="success" text={t('version.copied')} onDone={() => { setCopiedToast(null) }} />
      )}
    </div>
  )
}

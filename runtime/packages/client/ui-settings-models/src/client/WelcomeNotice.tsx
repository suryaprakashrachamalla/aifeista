/** Product-wide, versioned preview notice. */

import { useCallback, useEffect, useRef } from 'react'
import type { ReactNode } from 'react'
import type { SnapshotStore } from '@deepseek-ai/dsh-client-store'
import type { InjectFace, PropsRuntime } from '@deepseek-ai/dsh-client-ui-slots'
import type { WelcomeNoticeState, WelcomeNoticeStore } from './welcome-store.ts'
import type { en } from './locales.ts'

/** Registration-side dependencies of {@link WelcomeNotice}. */
export interface WelcomeNoticeInjected {
  hooks: {
    /** Durable acknowledgement state. */
    welcome: SnapshotStore<WelcomeNoticeState>
  }
  /** Welcome acknowledgement controller. */
  controller: WelcomeNoticeStore
  /** Onboarding copy. */
  t: (key: keyof typeof en) => string
}

/** Coordinator owner props plus this step's injected face. */
export type WelcomeNoticeProps =
  PropsRuntime<'settings.onboarding'> & InjectFace<WelcomeNoticeInjected>

/**
 * Render the current notice until its exact copy version is acknowledged.
 * @param props - settings-shell owner state and welcome dependencies.
 * @returns the welcome modal or null while the step decides not to show.
 */
export function WelcomeNotice(props: WelcomeNoticeProps): ReactNode {
  const { complete } = props
  const finished = useRef(false)
  const finish = useCallback((): void => {
    if (finished.current) return
    finished.current = true
    complete()
  }, [complete])

  useEffect(() => {
    finish()
  }, [finish])

  return null
}

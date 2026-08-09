import { invoke, isTauri } from '@tauri-apps/api/core'
import type { UnlistenFn } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'

export type ThemePreference = 'system' | 'light' | 'dark'

const browserThemeKey = 'stellr.theme'

function validTheme(value: string | null): ThemePreference {
  return value === 'light' || value === 'dark' ? value : 'system'
}

export function hasNativeShell(): boolean {
  return isTauri()
}

export async function observeWindowSuspension(
  notify: (suspended: boolean) => void,
): Promise<UnlistenFn> {
  if (!isTauri()) {
    let lastState: boolean | undefined
    const publish = () => {
      if (lastState === document.hidden) return
      lastState = document.hidden
      notify(lastState)
    }
    document.addEventListener('visibilitychange', publish)
    publish()
    return () => document.removeEventListener('visibilitychange', publish)
  }

  let window: ReturnType<typeof getCurrentWindow>
  try {
    window = getCurrentWindow()
  } catch {
    notify(false)
    return () => undefined
  }
  const unlisteners: UnlistenFn[] = []
  let disposed = false
  let revision = 0
  let lastState: boolean | undefined
  const publish = (state: boolean) => {
    if (lastState === state) return
    lastState = state
    notify(state)
  }
  const refresh = async () => {
    const request = ++revision
    let minimized = false
    try {
      minimized = await window.isMinimized()
    } catch {
      minimized = false
    }
    if (!disposed && request === revision) publish(minimized)
  }

  try {
    unlisteners.push(await window.onResized(() => { void refresh() }))
    unlisteners.push(await window.onFocusChanged(() => { void refresh() }))
    await refresh()
  } catch {
    disposed = true
    revision++
    for (const unlisten of unlisteners) unlisten()
    publish(false)
    return () => undefined
  }

  return () => {
    disposed = true
    revision++
    for (const unlisten of unlisteners) unlisten()
  }
}

export function getThemePreference(): Promise<ThemePreference> {
  if (isTauri()) return invoke<ThemePreference>('get_theme_preference')
  return Promise.resolve(validTheme(window.localStorage.getItem(browserThemeKey)))
}

export function setThemePreference(preference: ThemePreference): Promise<void> {
  if (isTauri()) return invoke<void>('set_theme_preference', { preference })
  window.localStorage.setItem(browserThemeKey, preference)
  return Promise.resolve()
}

export function chooseRepositoryDirectory(): Promise<string | null> {
  return isTauri() ? invoke<string | null>('choose_repository_directory') : Promise.resolve(null)
}

export function openExternalUrl(url: string): Promise<void> {
  if (isTauri()) return invoke<void>('open_external_url', { url })
  const parsed = new URL(url)
  if (parsed.protocol !== 'https:') return Promise.reject(new Error('Only HTTPS links can be opened.'))
  window.open(parsed.href, '_blank', 'noopener,noreferrer')
  return Promise.resolve()
}

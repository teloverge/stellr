import { invoke, isTauri } from '@tauri-apps/api/core'

export type ThemePreference = 'system' | 'light' | 'dark'

const browserThemeKey = 'stellr.theme'

function validTheme(value: string | null): ThemePreference {
  return value === 'light' || value === 'dark' ? value : 'system'
}

export function hasNativeShell(): boolean {
  return isTauri()
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

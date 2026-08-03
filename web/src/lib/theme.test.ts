import { describe, expect, it, vi } from 'vitest'
import { ThemeController, type ThemePreference } from './theme.svelte'

class FakeMediaQueryList {
  matches: boolean
  readonly media = '(prefers-color-scheme: dark)'
  onchange = null
  readonly listeners = new Set<(event: MediaQueryListEvent) => void>()

  constructor(matches: boolean) {
    this.matches = matches
  }

  addEventListener(_type: 'change', listener: (event: MediaQueryListEvent) => void): void {
    this.listeners.add(listener)
  }

  removeEventListener(_type: 'change', listener: (event: MediaQueryListEvent) => void): void {
    this.listeners.delete(listener)
  }

  emit(matches: boolean): void {
    this.matches = matches
    for (const listener of this.listeners) listener({ matches } as MediaQueryListEvent)
  }
}

function controller(initial: ThemePreference, systemDark: boolean) {
  const getPreference = vi.fn(async () => initial)
  const setPreference = vi.fn(async () => undefined)
  const media = new FakeMediaQueryList(systemDark)
  const root = document.createElement('html')
  const theme = new ThemeController({
    root,
    media: media as unknown as MediaQueryList,
    getPreference,
    setPreference,
  })
  return { theme, root, media, getPreference, setPreference }
}

describe('ThemeController', () => {
  it('loads System and follows operating-system changes', async () => {
    const { theme, root, media } = controller('system', true)

    await theme.start()
    expect(theme.preference).toBe('system')
    expect(root.dataset.theme).toBe('dark')

    media.emit(false)
    expect(root.dataset.theme).toBe('light')
  })

  it('persists explicit appearance and ignores later system changes', async () => {
    const { theme, root, media, setPreference } = controller('system', true)
    await theme.start()

    await theme.setPreference('light')
    media.emit(true)

    expect(setPreference).toHaveBeenCalledWith('light')
    expect(theme.preference).toBe('light')
    expect(root.dataset.theme).toBe('light')
  })

  it('removes its operating-system listener when destroyed', async () => {
    const { theme, media } = controller('system', false)
    await theme.start()
    expect(media.listeners.size).toBe(1)

    theme.destroy()
    expect(media.listeners.size).toBe(0)
  })
})

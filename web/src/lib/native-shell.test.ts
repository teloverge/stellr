import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

const native = vi.hoisted(() => ({
  active: false,
  minimized: vi.fn<() => Promise<boolean>>(),
  onResized: vi.fn(),
  onFocusChanged: vi.fn(),
  resize: undefined as (() => void) | undefined,
  focus: undefined as (() => void) | undefined,
  offResize: vi.fn(),
  offFocus: vi.fn(),
}))

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
  isTauri: () => native.active,
}))

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({
    isMinimized: native.minimized,
    onResized: native.onResized,
    onFocusChanged: native.onFocusChanged,
  }),
}))

import { observeWindowSuspension } from './native-shell'

beforeEach(() => {
  native.active = false
  native.resize = undefined
  native.focus = undefined
  native.minimized.mockReset()
  native.onResized.mockReset().mockImplementation(async (handler: () => void) => {
    native.resize = handler
    return native.offResize
  })
  native.onFocusChanged.mockReset().mockImplementation(async (handler: () => void) => {
    native.focus = handler
    return native.offFocus
  })
  native.offResize.mockReset()
  native.offFocus.mockReset()
})

afterEach(() => {
  vi.restoreAllMocks()
})

describe('window rendering suspension', () => {
  it('observes browser document visibility and cleans up', async () => {
    let hidden = false
    vi.spyOn(document, 'hidden', 'get').mockImplementation(() => hidden)
    const states: boolean[] = []
    const stop = await observeWindowSuspension((state) => states.push(state))

    hidden = true
    document.dispatchEvent(new Event('visibilitychange'))
    document.dispatchEvent(new Event('visibilitychange'))
    expect(states).toEqual([false, true])

    stop()
    hidden = false
    document.dispatchEvent(new Event('visibilitychange'))
    expect(states).toEqual([false, true])
  })

  it('queries native minimized state on startup, resize, and focus changes', async () => {
    native.active = true
    native.minimized.mockResolvedValueOnce(true).mockResolvedValueOnce(false).mockResolvedValueOnce(true)
    const states: boolean[] = []
    const stop = await observeWindowSuspension((state) => states.push(state))
    expect(states).toEqual([true])

    native.resize?.()
    await vi.waitFor(() => expect(states).toEqual([true, false]))
    native.focus?.()
    await vi.waitFor(() => expect(states).toEqual([true, false, true]))

    stop()
    expect(native.offResize).toHaveBeenCalledOnce()
    expect(native.offFocus).toHaveBeenCalledOnce()
  })

  it('discards an older native query that resolves after a newer query', async () => {
    native.active = true
    native.minimized.mockResolvedValueOnce(false)
    const states: boolean[] = []
    const stop = await observeWindowSuspension((state) => states.push(state))

    let resolveOld!: (value: boolean) => void
    let resolveNew!: (value: boolean) => void
    native.minimized
      .mockReturnValueOnce(new Promise((resolve) => { resolveOld = resolve }))
      .mockReturnValueOnce(new Promise((resolve) => { resolveNew = resolve }))
    native.resize?.()
    native.focus?.()
    resolveNew(false)
    await Promise.resolve()
    resolveOld(true)
    await Promise.resolve()

    expect(states).toEqual([false])
    stop()
  })

  it('fails open when native minimized state cannot be queried', async () => {
    native.active = true
    native.minimized.mockRejectedValue(new Error('window state unavailable'))
    const states: boolean[] = []

    const stop = await observeWindowSuspension((state) => states.push(state))

    expect(states).toEqual([false])
    stop()
  })

  it('cleans up and fails open when native listener setup fails', async () => {
    native.active = true
    native.onFocusChanged.mockRejectedValueOnce(new Error('focus listener unavailable'))
    const states: boolean[] = []

    const stop = await observeWindowSuspension((state) => states.push(state))

    expect(states).toEqual([false])
    expect(native.offResize).toHaveBeenCalledOnce()
    expect(() => stop()).not.toThrow()
  })
})

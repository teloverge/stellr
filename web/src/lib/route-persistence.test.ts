import { beforeEach, describe, expect, it, vi } from 'vitest'

const invoke = vi.hoisted(() => vi.fn())
vi.mock('@tauri-apps/api/core', () => ({ invoke, isTauri: () => true }))

import { hasNativeRoutePersistence, persistNativeRoute } from './native-route'

describe('native route persistence', () => {
  beforeEach(() => invoke.mockReset())

  it('sends only the validated space and issue route to the native store', async () => {
    invoke.mockResolvedValue(undefined)

    expect(hasNativeRoutePersistence()).toBe(true)
    await persistNativeRoute('teloverge-stellr', 64)

    expect(invoke).toHaveBeenCalledWith('persist_route_state', {
      space: 'teloverge-stellr',
      issue: 64,
    })
  })
})

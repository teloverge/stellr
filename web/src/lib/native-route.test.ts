import { describe, expect, it, vi } from 'vitest'
import { applyNativeRouteEvent, type NativeRouteEvent } from './native-route'

describe('native route requests', () => {
  it('preserves the current route and reports invalid forwarded targets', async () => {
    const add = vi.fn()
    const event: NativeRouteEvent = { state: 'error', message: 'unsupported target' }

    expect(await applyNativeRouteEvent(event, ['current-space'], add)).toEqual({
      route: null,
      error: 'unsupported target',
    })
    expect(add).not.toHaveBeenCalled()
  })

  it('routes existing spaces without mutating them', async () => {
    const add = vi.fn()
    const event: NativeRouteEvent = {
      state: 'target',
      target: {
        space_id: 'teloverge-stellr',
        repo: 'teloverge/stellr',
        path: null,
        issue: 62,
      },
    }

    expect(await applyNativeRouteEvent(event, ['teloverge-stellr'], add)).toEqual({
      route: { space: 'teloverge-stellr', issue: 62 },
      error: null,
    })
    expect(add).not.toHaveBeenCalled()
  })

  it('adds a new forwarded path before routing it', async () => {
    const add = vi.fn().mockResolvedValue(new Response('', { status: 201 }))
    const event: NativeRouteEvent = {
      state: 'target',
      target: {
        space_id: 'teloverge-stellr',
        repo: 'teloverge/stellr',
        path: 'D:\\dev\\stellr',
        issue: null,
      },
    }

    expect(await applyNativeRouteEvent(event, [], add)).toEqual({
      route: { space: 'teloverge-stellr', issue: null },
      error: null,
    })
    expect(add).toHaveBeenCalledWith({ path: 'D:\\dev\\stellr' })
  })

  it('keeps the current route when adding the forwarded target fails', async () => {
    const add = vi.fn().mockResolvedValue(new Response('Repository not found', { status: 404 }))
    const event: NativeRouteEvent = {
      state: 'target',
      target: {
        space_id: 'missing-repo',
        repo: 'missing/repo',
        path: null,
        issue: null,
      },
    }

    expect(await applyNativeRouteEvent(event, [], add)).toEqual({
      route: null,
      error: 'Repository not found',
    })
  })
})

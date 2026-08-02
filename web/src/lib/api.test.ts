import { afterEach, describe, expect, it, vi } from 'vitest'
import { addSpace, refreshSpace, removeSpace } from './api'

afterEach(() => {
  vi.unstubAllGlobals()
})

describe('spaces API client', () => {
  it('posts exactly the supplied add-space field and returns the raw response', async () => {
    const response = new Response(JSON.stringify({ id: 'teloverge-stellr' }), { status: 201 })
    const fetch = vi.fn().mockResolvedValue(response)
    vi.stubGlobal('fetch', fetch)

    const received = await addSpace({ repo: 'teloverge/stellr' })

    expect(fetch).toHaveBeenCalledWith('/api/spaces', {
      method: 'POST',
      credentials: 'same-origin',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ repo: 'teloverge/stellr' }),
    })
    expect(received).toBe(response)
  })

  it('encodes the complete space ID when deleting', async () => {
    const fetch = vi.fn().mockResolvedValue(new Response(null, { status: 204 }))
    vi.stubGlobal('fetch', fetch)

    await removeSpace('owner/repo name')

    expect(fetch).toHaveBeenCalledWith('/api/spaces/owner%2Frepo%20name', {
      method: 'DELETE',
      credentials: 'same-origin',
    })
  })

  it('encodes the complete space ID before the refresh suffix', async () => {
    const fetch = vi.fn().mockResolvedValue(new Response(null, { status: 202 }))
    vi.stubGlobal('fetch', fetch)

    await refreshSpace('owner/repo name')

    expect(fetch).toHaveBeenCalledWith('/api/spaces/owner%2Frepo%20name/refresh', {
      method: 'POST',
      credentials: 'same-origin',
    })
  })
})

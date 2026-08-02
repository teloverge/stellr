import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { addSpace, refreshSpace, removeSpace } from './api'

describe('spaces API client', () => {
  const fetchStub = vi.fn<() => Promise<Response>>()

  beforeEach(() => {
    fetchStub.mockResolvedValue(new Response())
    vi.stubGlobal('fetch', fetchStub)
  })

  afterEach(() => {
    vi.unstubAllGlobals()
  })

  it('posts an add-space request with the supplied body', async () => {
    await addSpace({ repo: 'teloverge/stellr' })

    expect(fetchStub).toHaveBeenCalledWith('/api/spaces', {
      method: 'POST',
      credentials: 'same-origin',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ repo: 'teloverge/stellr' }),
    })
  })

  it('deletes an encoded space ID', async () => {
    await removeSpace('teloverge/stellr')

    expect(fetchStub).toHaveBeenCalledWith('/api/spaces/teloverge%2Fstellr', {
      method: 'DELETE',
      credentials: 'same-origin',
    })
  })

  it('posts a refresh request for an encoded space ID', async () => {
    await refreshSpace('teloverge/stellr')

    expect(fetchStub).toHaveBeenCalledWith('/api/spaces/teloverge%2Fstellr/refresh', {
      method: 'POST',
      credentials: 'same-origin',
    })
  })
})

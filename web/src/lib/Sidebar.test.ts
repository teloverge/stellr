import { afterEach, describe, expect, it, vi } from 'vitest'
import { flushSync, mount, unmount } from 'svelte'
import Sidebar from './Sidebar.svelte'
import type { SpaceModel } from './model'

const mounted: object[] = []

afterEach(async () => {
  for (const component of mounted.splice(0)) {
    await unmount(component)
  }
  document.body.innerHTML = ''
})

function space(overrides: Partial<SpaceModel> = {}): SpaceModel {
  return {
    id: 'teloverge-stellr',
    repo: 'teloverge/stellr',
    name: 'stellr',
    stars: [],
    synced_at: 1_754_006_400,
    stale: false,
    error: null,
    ...overrides,
  }
}

function render(
  spaces: SpaceModel[],
  overrides: Partial<{
    activeSpaceId: string | null
    connectionStatus: 'connecting' | 'open' | 'closed'
    select: (id: string) => void
    added: (id: string) => void
    removed: (id: string) => void
    addRequest: (body: { path?: string; repo?: string }) => Promise<Response>
    removeRequest: (id: string) => Promise<Response>
    refreshRequest: (id: string) => Promise<Response>
  }> = {},
): HTMLElement {
  const target = document.createElement('div')
  document.body.appendChild(target)
  mounted.push(
    mount(Sidebar, {
      target,
      props: {
        spaces,
        activeSpaceId: spaces[0]?.id ?? null,
        connectionStatus: 'open',
        select: () => undefined,
        added: () => undefined,
        removed: () => undefined,
        ...overrides,
      },
    }),
  )
  return target
}

function enter(input: HTMLInputElement, value: string): void {
  input.value = value
  input.dispatchEvent(new Event('input', { bubbles: true }))
  flushSync()
}

describe('Sidebar space list', () => {
  it('shows repository, explicit sync time, stale state, provider error, and active row', () => {
    const target = render([
      space(),
      space({
        id: 'cached-space',
        repo: 'teloverge/cached',
        name: 'cached',
        stale: true,
        error: 'GitHub rate limit exhausted',
      }),
    ])

    expect(target.textContent).toContain('stellr')
    expect(target.textContent).toContain('teloverge/stellr')
    expect(target.textContent).toContain('Synced 2025-08-01 00:00 UTC')
    expect(target.textContent).toContain('Stale')
    expect(target.textContent).toContain('GitHub rate limit exhausted')
    expect(
      target.querySelector<HTMLButtonElement>('button[data-space-id="teloverge-stellr"]')
        ?.getAttribute('aria-current'),
    ).toBe('true')
  })

  it('selects the exact space from its row button', () => {
    const selected: string[] = []
    const target = render([space(), space({ id: 'other', name: 'other' })], {
      select: (id) => selected.push(id),
    })

    target
      .querySelector<HTMLButtonElement>('button[data-space-id="other"]')!
      .dispatchEvent(new MouseEvent('click', { bubbles: true }))

    expect(selected).toEqual(['other'])
  })
})

describe('Sidebar add form', () => {
  it('enables Add only when exactly one trimmed source is present', () => {
    const target = render([])
    const path = target.querySelector<HTMLInputElement>('input[name="path"]')!
    const repo = target.querySelector<HTMLInputElement>('input[name="repo"]')!
    const add = target.querySelector<HTMLButtonElement>('button[type="submit"]')!

    expect(add.disabled).toBe(true)

    enter(path, '  D:\\dev\\stellr  ')
    expect(add.disabled).toBe(false)

    enter(repo, ' teloverge/stellr ')
    expect(add.disabled).toBe(true)

    enter(path, '  ')
    expect(add.disabled).toBe(false)

    enter(repo, ' ')
    expect(add.disabled).toBe(true)
  })

  it('adds the one trimmed source, clears both inputs, and selects the response ID', async () => {
    const addRequest = vi
      .fn()
      .mockResolvedValue(
        new Response(JSON.stringify({ id: 'teloverge-stellr' }), {
          status: 200,
          headers: { 'content-type': 'application/json' },
        }),
      )
    const added = vi.fn()
    const target = render([], { addRequest, added })
    const path = target.querySelector<HTMLInputElement>('input[name="path"]')!
    const repo = target.querySelector<HTMLInputElement>('input[name="repo"]')!

    enter(repo, '  teloverge/stellr  ')
    target.querySelector('form')!.dispatchEvent(new SubmitEvent('submit', { bubbles: true, cancelable: true }))

    await vi.waitFor(() => expect(added).toHaveBeenCalledWith('teloverge-stellr'))
    expect(addRequest).toHaveBeenCalledWith({ repo: 'teloverge/stellr' })
    expect(path.value).toBe('')
    expect(repo.value).toBe('')
  })

  it('keeps a non-success response beside the form without selecting a space', async () => {
    const added = vi.fn()
    const target = render([], {
      added,
      addRequest: vi
        .fn()
        .mockResolvedValue(new Response('repo must be in owner/name form', { status: 400 })),
    })

    enter(target.querySelector<HTMLInputElement>('input[name="repo"]')!, 'invalid')
    target.querySelector('form')!.dispatchEvent(new SubmitEvent('submit', { bubbles: true, cancelable: true }))

    await vi.waitFor(() =>
      expect(target.querySelector('[data-add-error]')?.textContent).toContain(
        'repo must be in owner/name form',
      ),
    )
    expect(added).not.toHaveBeenCalled()
  })

  it('keeps a thrown network error beside the form without selecting a space', async () => {
    const added = vi.fn()
    const target = render([], {
      added,
      addRequest: vi.fn().mockRejectedValue(new Error('connection reset')),
    })

    enter(target.querySelector<HTMLInputElement>('input[name="path"]')!, 'D:\\dev\\stellr')
    target.querySelector('form')!.dispatchEvent(new SubmitEvent('submit', { bubbles: true, cancelable: true }))

    await vi.waitFor(() =>
      expect(target.querySelector('[data-add-error]')?.textContent).toContain('connection reset'),
    )
    expect(added).not.toHaveBeenCalled()
  })
})

describe('Sidebar row actions', () => {
  it('targets one row and disables only that row while refresh is pending', async () => {
    let finish!: (response: Response) => void
    const refreshRequest = vi.fn(
      () =>
        new Promise<Response>((resolve) => {
          finish = resolve
        }),
    )
    const target = render([space(), space({ id: 'other', name: 'other' })], { refreshRequest })

    target
      .querySelector<HTMLButtonElement>('[data-space-row="teloverge-stellr"] [data-action="refresh"]')!
      .dispatchEvent(new MouseEvent('click', { bubbles: true }))
    flushSync()

    expect(refreshRequest).toHaveBeenCalledWith('teloverge-stellr')
    expect(
      target.querySelector<HTMLButtonElement>(
        '[data-space-row="teloverge-stellr"] [data-action="refresh"]',
      )?.disabled,
    ).toBe(true)
    expect(
      target.querySelector<HTMLButtonElement>(
        '[data-space-row="teloverge-stellr"] [data-action="remove"]',
      )?.disabled,
    ).toBe(false)
    expect(
      target.querySelector<HTMLButtonElement>('[data-space-row="other"] [data-action="refresh"]')
        ?.disabled,
    ).toBe(false)

    finish(new Response(null, { status: 200 }))
    await vi.waitFor(() =>
      expect(
        target.querySelector<HTMLButtonElement>(
          '[data-space-row="teloverge-stellr"] [data-action="refresh"]',
        )?.disabled,
      ).toBe(false),
    )
  })

  it('calls removed only after a successful deletion of the exact row', async () => {
    const removed = vi.fn()
    const removeRequest = vi.fn().mockResolvedValue(new Response(null, { status: 204 }))
    const target = render([space({ id: 'owner-repo' })], { removed, removeRequest })

    target
      .querySelector<HTMLButtonElement>('[data-space-row="owner-repo"] [data-action="remove"]')!
      .dispatchEvent(new MouseEvent('click', { bubbles: true }))

    await vi.waitFor(() => expect(removed).toHaveBeenCalledWith('owner-repo'))
    expect(removeRequest).toHaveBeenCalledWith('owner-repo')
  })

  it('keeps response and network failures in the affected rows', async () => {
    const removed = vi.fn()
    const target = render([space(), space({ id: 'other', name: 'other' })], {
      removed,
      refreshRequest: vi.fn().mockResolvedValue(new Response('refresh refused', { status: 503 })),
      removeRequest: vi.fn().mockRejectedValue(new Error('delete connection reset')),
    })

    target
      .querySelector<HTMLButtonElement>('[data-space-row="teloverge-stellr"] [data-action="refresh"]')!
      .dispatchEvent(new MouseEvent('click', { bubbles: true }))
    target
      .querySelector<HTMLButtonElement>('[data-space-row="other"] [data-action="remove"]')!
      .dispatchEvent(new MouseEvent('click', { bubbles: true }))

    await vi.waitFor(() => {
      expect(
        target.querySelector('[data-space-row="teloverge-stellr"] [data-row-error]')?.textContent,
      ).toContain('refresh refused')
      expect(target.querySelector('[data-space-row="other"] [data-row-error]')?.textContent).toContain(
        'delete connection reset',
      )
    })
    expect(removed).not.toHaveBeenCalled()
  })
})

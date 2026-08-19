import { afterEach, describe, expect, it, vi } from 'vitest'
import { flushSync, mount, unmount } from 'svelte'
import Sidebar from './Sidebar.svelte'
import type { addSpace, refreshSpace, removeSpace } from './api'
import type { chooseRepositoryDirectory } from './native-shell'
import type { ConnectionStatus } from './control.svelte'
import type { SpaceModel } from './model'

const mounted: object[] = []

afterEach(async () => {
  for (const component of mounted.splice(0)) {
    await unmount(component)
  }
  document.body.innerHTML = ''
  vi.restoreAllMocks()
})

function space(overrides: Partial<SpaceModel> = {}): SpaceModel {
  return {
    id: 'teloverge/stellr',
    repo: 'teloverge/stellr',
    name: 'Stellr',
    stars: [],
    synced_at: 1_754_000_000,
    stale: false,
    error: null,
    ...overrides,
  }
}

function render(
  spaces: SpaceModel[],
  overrides: {
    activeSpaceId?: string | null
    connectionStatus?: ConnectionStatus
    select?: (id: string) => void
    added?: (id: string) => void
    removed?: (id: string) => void
    addRequest?: typeof addSpace
    removeRequest?: typeof removeSpace
    refreshRequest?: typeof refreshSpace
    nativeShell?: boolean
    chooseDirectory?: typeof chooseRepositoryDirectory
  } = {},
): HTMLElement {
  const target = document.createElement('div')
  document.body.appendChild(target)
  mounted.push(
    mount(Sidebar, {
      target,
      props: {
        spaces,
        activeSpaceId: overrides.activeSpaceId ?? null,
        connectionStatus: overrides.connectionStatus ?? 'open',
        select: overrides.select ?? (() => undefined),
        added: overrides.added ?? (() => undefined),
        removed: overrides.removed ?? (() => undefined),
        addRequest: overrides.addRequest,
        removeRequest: overrides.removeRequest,
        refreshRequest: overrides.refreshRequest,
        nativeShell: overrides.nativeShell,
        chooseDirectory: overrides.chooseDirectory,
      },
    }),
  )
  flushSync()
  return target
}

function enter(input: HTMLInputElement, value: string): void {
  input.value = value
  input.dispatchEvent(new Event('input', { bubbles: true }))
  flushSync()
}

async function settle(): Promise<void> {
  await new Promise((resolve) => setTimeout(resolve, 0))
  flushSync()
}

describe('Sidebar display', () => {
  it('shows repository identity, sync age, connection state, and active-row semantics', () => {
    vi.spyOn(Date, 'now').mockReturnValue(1_754_000_120_000)
    const selected: string[] = []
    const target = render([space()], {
      activeSpaceId: 'teloverge/stellr',
      connectionStatus: 'connecting',
      select: (id) => selected.push(id),
    })

    expect(target.textContent).toContain('Stellr')
    expect(target.textContent).toContain('teloverge/stellr')
    expect(target.textContent).toContain('Synced 2 minutes ago')
    expect(target.textContent).toContain('Connecting')

    const row = target.querySelector<HTMLButtonElement>('button[aria-current="true"]')!
    row.dispatchEvent(new MouseEvent('click', { bubbles: true }))
    expect(selected).toEqual(['teloverge/stellr'])
  })

  it('keeps stale cached spaces selectable with textual stale and provider-error details', () => {
    const selected: string[] = []
    const target = render(
      [
        space({
          id: 'cached-space',
          repo: 'teloverge/cached',
          name: 'Cached space',
          stale: true,
          error: 'GitHub rate limit exceeded',
        }),
      ],
      { select: (id) => selected.push(id) },
    )

    expect(target.textContent).toContain('Stale')
    expect(target.textContent).toContain('GitHub rate limit exceeded')
    expect(target.querySelector('[role="status"]')?.textContent).toContain(
      'GitHub rate limit exceeded',
    )

    target
      .querySelector<HTMLButtonElement>('button[data-space-id="cached-space"]')!
      .dispatchEvent(new MouseEvent('click', { bubbles: true }))
    expect(selected).toEqual(['cached-space'])
  })

  it('labels an unauthorized connection as an expired session', () => {
    const target = render([], { connectionStatus: 'unauthorized' })

    expect(target.textContent).toContain('Session expired')
  })
})

describe('Sidebar add form', () => {
  it('uses the native chooser to populate one local repository path', async () => {
    const chooseDirectory = vi
      .fn<typeof chooseRepositoryDirectory>()
      .mockResolvedValue('D:\\dev\\stellr')
    const target = render([], { nativeShell: true, chooseDirectory })

    target
      .querySelector<HTMLButtonElement>('button[aria-label="Browse for local repository"]')!
      .click()
    await settle()

    expect(chooseDirectory).toHaveBeenCalledOnce()
    expect(target.querySelector<HTMLInputElement>('input[name="path"]')?.value).toBe(
      'D:\\dev\\stellr',
    )
  })

  it('enables Add only when exactly one trimmed input is nonblank', () => {
    const target = render([])
    const path = target.querySelector<HTMLInputElement>('input[name="path"]')!
    const repo = target.querySelector<HTMLInputElement>('input[name="repo"]')!
    const add = target.querySelector<HTMLButtonElement>('button[type="submit"]')!

    expect(add.disabled).toBe(true)

    enter(path, '  D:\\dev\\stellr  ')
    expect(add.disabled).toBe(false)

    enter(path, '   ')
    enter(repo, '  teloverge/stellr  ')
    expect(add.disabled).toBe(false)

    enter(path, ' D:\\dev\\stellr ')
    expect(add.disabled).toBe(true)
  })

  it('submits one trimmed field, calls added with the response ID, and clears both inputs', async () => {
    const addRequest = vi.fn<typeof addSpace>().mockResolvedValue(
      new Response(JSON.stringify({ id: 'local-stellr' }), {
        status: 201,
        headers: { 'content-type': 'application/json' },
      }),
    )
    const added: string[] = []
    const target = render([], { addRequest, added: (id) => added.push(id) })
    const path = target.querySelector<HTMLInputElement>('input[name="path"]')!
    const repo = target.querySelector<HTMLInputElement>('input[name="repo"]')!

    enter(path, '  D:\\dev\\stellr  ')
    target.querySelector<HTMLButtonElement>('button[type="submit"]')!.click()
    await settle()

    expect(addRequest).toHaveBeenCalledWith({ path: 'D:\\dev\\stellr' })
    expect(added).toEqual(['local-stellr'])
    expect(path.value).toBe('')
    expect(repo.value).toBe('')
  })

  it('keeps non-success response text beside the form without calling added', async () => {
    const addRequest = vi
      .fn<typeof addSpace>()
      .mockResolvedValue(new Response('Repository not found', { status: 404 }))
    const added: string[] = []
    const target = render([], { addRequest, added: (id) => added.push(id) })

    enter(target.querySelector<HTMLInputElement>('input[name="repo"]')!, 'missing/repo')
    target.querySelector<HTMLButtonElement>('button[type="submit"]')!.click()
    await settle()

    const form = target.querySelector('form')!
    expect(form.textContent).toContain('Repository not found')
    expect(added).toEqual([])
  })

  it('keeps thrown network errors beside the form without calling added', async () => {
    const addRequest = vi.fn<typeof addSpace>().mockRejectedValue(new Error('Network unavailable'))
    const added: string[] = []
    const target = render([], { addRequest, added: (id) => added.push(id) })

    enter(target.querySelector<HTMLInputElement>('input[name="repo"]')!, 'teloverge/stellr')
    target.querySelector<HTMLButtonElement>('button[type="submit"]')!.click()
    await settle()

    const form = target.querySelector('form')!
    expect(form.textContent).toContain('Network unavailable')
    expect(added).toEqual([])
  })
})

describe('Sidebar row actions', () => {
  it('tracks pending state per row and action while leaving other actions usable', async () => {
    let finishRefresh!: (response: Response) => void
    let finishRemove!: (response: Response) => void
    const refreshRequest = vi.fn<typeof refreshSpace>().mockReturnValue(
      new Promise((resolve) => {
        finishRefresh = resolve
      }),
    )
    const removeRequest = vi.fn<typeof removeSpace>().mockReturnValue(
      new Promise((resolve) => {
        finishRemove = resolve
      }),
    )
    const removed: string[] = []
    const target = render(
      [
        space(),
        space({ id: 'teloverge/other', repo: 'teloverge/other', name: 'Other' }),
      ],
      { refreshRequest, removeRequest, removed: (id) => removed.push(id) },
    )
    const firstRow = target.querySelector<HTMLElement>('[data-space-row="teloverge/stellr"]')!
    const secondRow = target.querySelector<HTMLElement>('[data-space-row="teloverge/other"]')!

    firstRow.querySelector<HTMLButtonElement>('button[aria-label="Refresh Stellr"]')!.click()
    flushSync()

    expect(refreshRequest).toHaveBeenCalledWith('teloverge/stellr')
    expect(firstRow.querySelector<HTMLButtonElement>('button[aria-label="Refresh Stellr"]')!.disabled).toBe(
      true,
    )
    expect(firstRow.querySelector<HTMLButtonElement>('button[aria-label="Remove Stellr"]')!.disabled).toBe(
      false,
    )
    expect(secondRow.querySelector<HTMLButtonElement>('button[aria-label="Refresh Other"]')!.disabled).toBe(
      false,
    )
    expect(secondRow.querySelector<HTMLButtonElement>('button[aria-label="Remove Other"]')!.disabled).toBe(
      false,
    )

    firstRow.querySelector<HTMLButtonElement>('button[aria-label="Remove Stellr"]')!.click()
    flushSync()
    expect(removeRequest).toHaveBeenCalledWith('teloverge/stellr')
    expect(firstRow.querySelector<HTMLButtonElement>('button[aria-label="Remove Stellr"]')!.disabled).toBe(
      true,
    )

    finishRefresh(new Response(null, { status: 204 }))
    await settle()
    expect(firstRow.querySelector<HTMLButtonElement>('button[aria-label="Refresh Stellr"]')!.disabled).toBe(
      false,
    )
    expect(firstRow.querySelector<HTMLButtonElement>('button[aria-label="Remove Stellr"]')!.disabled).toBe(
      true,
    )

    finishRemove(new Response(null, { status: 204 }))
    await settle()
    expect(removed).toEqual(['teloverge/stellr'])
    expect(firstRow.querySelector<HTMLButtonElement>('button[aria-label="Remove Stellr"]')!.disabled).toBe(
      false,
    )
  })

  it('renders response and network failures inside the affected rows', async () => {
    const refreshRequest = vi
      .fn<typeof refreshSpace>()
      .mockResolvedValue(new Response('Refresh rejected', { status: 503 }))
    const removeRequest = vi
      .fn<typeof removeSpace>()
      .mockRejectedValue(new Error('Delete network unavailable'))
    const target = render(
      [
        space(),
        space({ id: 'teloverge/other', repo: 'teloverge/other', name: 'Other' }),
      ],
      { refreshRequest, removeRequest },
    )
    const firstRow = target.querySelector<HTMLElement>('[data-space-row="teloverge/stellr"]')!
    const secondRow = target.querySelector<HTMLElement>('[data-space-row="teloverge/other"]')!

    firstRow.querySelector<HTMLButtonElement>('button[aria-label="Refresh Stellr"]')!.click()
    secondRow.querySelector<HTMLButtonElement>('button[aria-label="Remove Other"]')!.click()
    await settle()

    expect(firstRow.textContent).toContain('Refresh rejected')
    expect(firstRow.textContent).not.toContain('Delete network unavailable')
    expect(secondRow.textContent).toContain('Delete network unavailable')
    expect(secondRow.textContent).not.toContain('Refresh rejected')
  })

  it('calls removed with the target ID only after a successful deletion', async () => {
    let finishRemove!: (response: Response) => void
    const removeRequest = vi.fn<typeof removeSpace>().mockReturnValue(
      new Promise((resolve) => {
        finishRemove = resolve
      }),
    )
    const removed: string[] = []
    const target = render([space()], { removeRequest, removed: (id) => removed.push(id) })
    const row = target.querySelector<HTMLElement>('[data-space-row="teloverge/stellr"]')!

    row.querySelector<HTMLButtonElement>('button[aria-label="Remove Stellr"]')!.click()
    flushSync()

    expect(removeRequest).toHaveBeenCalledWith('teloverge/stellr')
    expect(removed).toEqual([])

    finishRemove(new Response(null, { status: 204 }))
    await settle()
    expect(removed).toEqual(['teloverge/stellr'])
  })

  it('does not call removed when deletion returns a non-success response', async () => {
    const removeRequest = vi
      .fn<typeof removeSpace>()
      .mockResolvedValue(new Response('Delete rejected', { status: 409 }))
    const removed: string[] = []
    const target = render([space()], { removeRequest, removed: (id) => removed.push(id) })
    const row = target.querySelector<HTMLElement>('[data-space-row="teloverge/stellr"]')!

    row.querySelector<HTMLButtonElement>('button[aria-label="Remove Stellr"]')!.click()
    await settle()

    expect(row.textContent).toContain('Delete rejected')
    expect(removed).toEqual([])
  })
})

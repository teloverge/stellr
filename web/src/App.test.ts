import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { flushSync, mount, unmount } from 'svelte'
import App from './App.svelte'
import type { Model, SpaceModel, Star } from './lib/model'
import { StarMap as Renderer } from './lib/starmap/starmap'

class FakeWebSocket {
  static instances: FakeWebSocket[] = []
  closed = false

  onclose: ((event: CloseEvent) => void) | null = null
  onmessage: ((event: MessageEvent) => void) | null = null
  onopen: ((event: Event) => void) | null = null

  constructor(readonly url: string) {
    FakeWebSocket.instances.push(this)
  }

  close(): void {
    this.closed = true
  }

  emitModel(model: Model): void {
    this.onmessage?.(new MessageEvent('message', { data: JSON.stringify(model) }))
  }
}

class FakeResizeObserver {
  static instances: FakeResizeObserver[] = []

  constructor(readonly callback: ResizeObserverCallback) {
    FakeResizeObserver.instances.push(this)
  }

  observe(): void {}
  disconnect(): void {}
  unobserve(): void {}

  emit(width: number, height: number): void {
    this.callback(
      [{ contentRect: { width, height } } as ResizeObserverEntry],
      this as unknown as ResizeObserver,
    )
  }
}

const mounted: object[] = []
let clientWidth: PropertyDescriptor | undefined
let clientHeight: PropertyDescriptor | undefined

beforeEach(() => {
  FakeWebSocket.instances = []
  FakeResizeObserver.instances = []
  vi.stubGlobal('WebSocket', FakeWebSocket as unknown as typeof WebSocket)
  vi.stubGlobal('ResizeObserver', FakeResizeObserver as unknown as typeof ResizeObserver)
  window.history.replaceState(null, '', '/')

  clientWidth = Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'clientWidth')
  clientHeight = Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'clientHeight')
  Object.defineProperty(HTMLElement.prototype, 'clientWidth', { configurable: true, get: () => 1000 })
  Object.defineProperty(HTMLElement.prototype, 'clientHeight', { configurable: true, get: () => 700 })
})

afterEach(async () => {
  for (const component of mounted.splice(0)) {
    await unmount(component)
  }
  document.body.innerHTML = ''
  window.history.replaceState(null, '', '/')
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
  if (clientWidth) Object.defineProperty(HTMLElement.prototype, 'clientWidth', clientWidth)
  if (clientHeight) Object.defineProperty(HTMLElement.prototype, 'clientHeight', clientHeight)
})

function issue(number: number): Star {
  return {
    number,
    parent_issue: null,
    title: `Issue ${number}`,
    status: 'frontier',
    blocked_by: [],
    milestone: 'M1',
    labels: ['ready-for-agent'],
    assignees: [],
    url: `https://github.com/teloverge/stellr/issues/${number}`,
    body: `Detail for **Issue ${number}**`,
  }
}

function space(id: string, issueNumber: number): SpaceModel {
  return {
    id,
    repo: `teloverge/${id}`,
    name: id,
    stars: [issue(issueNumber)],
    synced_at: 1_754_000_000,
    stale: false,
    error: null,
  }
}

const model: Model = {
  spaces: [space('first', 11), space('second', 22)],
}

const lifecycleModel: Model = {
  spaces: [space('first', 11), space('middle', 22), space('last', 33)],
}

function mountApp(): { target: HTMLElement; socket: FakeWebSocket; component: object } {
  const target = document.createElement('div')
  document.body.appendChild(target)
  const component = mount(App, { target })
  mounted.push(component)
  flushSync()
  return { target, socket: FakeWebSocket.instances[0], component }
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

describe('App issue routing', () => {
  it('closes its control socket when the App is unmounted', async () => {
    const { socket, component } = mountApp()

    await unmount(component)
    mounted.splice(mounted.indexOf(component), 1)

    expect(socket.closed).toBe(true)
  })

  it('routes an unaddressed snapshot to its first available space', () => {
    const { target, socket } = mountApp()

    socket.emitModel(model)
    flushSync()

    expect(window.location.hash).toBe('#s=first')
    expect(target.querySelector('[aria-label="Spaces"]')).not.toBeNull()
    expect(target.querySelector('[data-space-row="first"] [aria-current="true"]')).not.toBeNull()
    expect(target.querySelector('.star-map')).not.toBeNull()
  })

  it('routes a successfully added space without retaining an issue selection', async () => {
    const fetchRequest = vi.fn<typeof fetch>().mockResolvedValue(
      new Response(JSON.stringify({ id: 'new-space' }), {
        status: 201,
        headers: { 'content-type': 'application/json' },
      }),
    )
    vi.stubGlobal('fetch', fetchRequest)
    window.history.replaceState(null, '', '/#s=first&i=11')
    const { target, socket } = mountApp()
    socket.emitModel(model)
    flushSync()

    enter(target.querySelector<HTMLInputElement>('input[name="repo"]')!, 'teloverge/new-space')
    target.querySelector<HTMLButtonElement>('button[type="submit"]')!.click()
    await settle()

    expect(fetchRequest).toHaveBeenCalledWith(
      '/api/spaces',
      expect.objectContaining({ method: 'POST' }),
    )
    expect(window.location.hash).toBe('#s=new-space')
  })

  it('waits for the authoritative added-space snapshot before rendering its map', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn<typeof fetch>().mockResolvedValue(
        new Response(JSON.stringify({ id: 'new-space' }), {
          status: 201,
          headers: { 'content-type': 'application/json' },
        }),
      ),
    )
    window.history.replaceState(null, '', '/#s=first&i=11')
    const { target, socket } = mountApp()
    socket.emitModel(model)
    flushSync()

    enter(target.querySelector<HTMLInputElement>('input[name="repo"]')!, 'teloverge/new-space')
    target.querySelector<HTMLButtonElement>('button[type="submit"]')!.click()
    await settle()
    expect(window.location.hash).toBe('#s=new-space')
    expect(target.querySelector('.star-map')).toBeNull()

    socket.emitModel(model)
    flushSync()

    expect(window.location.hash).toBe('#s=new-space')
    expect(target.querySelector('.star-map')).toBeNull()

    socket.emitModel({ spaces: [...model.spaces, space('new-space', 44)] })
    flushSync()

    expect(window.location.hash).toBe('#s=new-space')
    expect(target.querySelector('[data-space-row="new-space"] [aria-current="true"]')).not.toBeNull()
    expect(target.querySelector('.star-map')).not.toBeNull()
  })

  it('routes to the following space after successfully removing the active middle space', async () => {
    const fetchRequest = vi.fn<typeof fetch>().mockResolvedValue(new Response(null, { status: 204 }))
    vi.stubGlobal('fetch', fetchRequest)
    window.history.replaceState(null, '', '/#s=middle&i=22')
    const { target, socket } = mountApp()
    socket.emitModel(lifecycleModel)
    flushSync()

    target.querySelector<HTMLButtonElement>('button[aria-label="Remove middle"]')!.click()
    await settle()

    expect(fetchRequest).toHaveBeenCalledWith(
      '/api/spaces/middle',
      expect.objectContaining({ method: 'DELETE' }),
    )
    expect(window.location.hash).toBe('#s=last')
  })

  it('uses pre-removal ordering when the post-removal snapshot arrives before success', async () => {
    let finishRemove!: (response: Response) => void
    vi.stubGlobal(
      'fetch',
      vi.fn<typeof fetch>().mockReturnValue(
        new Promise((resolve) => {
          finishRemove = resolve
        }),
      ),
    )
    window.history.replaceState(null, '', '/#s=middle&i=22')
    const { target, socket } = mountApp()
    socket.emitModel(lifecycleModel)
    flushSync()

    target.querySelector<HTMLButtonElement>('button[aria-label="Remove middle"]')!.click()
    flushSync()
    socket.emitModel({ spaces: [space('first', 11), space('last', 33)] })
    flushSync()

    finishRemove(new Response(null, { status: 204 }))
    await settle()

    expect(window.location.hash).toBe('#s=last')
  })

  it.each([
    { completionOrder: ['middle', 'last'], hasRemainingSpace: true, expected: '#s=first' },
    { completionOrder: ['last', 'middle'], hasRemainingSpace: true, expected: '#s=first' },
    { completionOrder: ['middle', 'last'], hasRemainingSpace: false, expected: '' },
    { completionOrder: ['last', 'middle'], hasRemainingSpace: false, expected: '' },
  ])(
    'routes to an available fallback when concurrent removals finish in order $completionOrder with remaining=$hasRemainingSpace',
    async ({ completionOrder, hasRemainingSpace, expected }) => {
      const finishRemove: Record<string, (response: Response) => void> = {}
      vi.stubGlobal(
        'fetch',
        vi.fn<typeof fetch>().mockImplementation(
          (input) =>
            new Promise((resolve) => {
              const id = String(input).split('/').at(-1)!
              finishRemove[id] = resolve
            }),
        ),
      )
      window.history.replaceState(null, '', '/#s=middle&i=22')
      const { target, socket } = mountApp()
      socket.emitModel({
        spaces: hasRemainingSpace
          ? lifecycleModel.spaces
          : [space('middle', 22), space('last', 33)],
      })
      flushSync()

      target.querySelector<HTMLButtonElement>('button[aria-label="Remove middle"]')!.click()
      target.querySelector<HTMLButtonElement>('button[aria-label="Remove last"]')!.click()
      flushSync()
      socket.emitModel({ spaces: hasRemainingSpace ? [space('first', 11)] : [] })
      flushSync()

      for (const id of completionOrder) {
        finishRemove[id]!(new Response(null, { status: 204 }))
        await settle()
      }

      expect(window.location.hash).toBe(expected)
    },
  )

  it.each([
    { active: 'first', issueNumber: 11, expected: '#s=middle' },
    { active: 'last', issueNumber: 33, expected: '#s=middle' },
  ])(
    'chooses the next available space after removing active $active',
    async ({ active, issueNumber, expected }) => {
      vi.stubGlobal(
        'fetch',
        vi.fn<typeof fetch>().mockResolvedValue(new Response(null, { status: 204 })),
      )
      window.history.replaceState(null, '', `/#s=${active}&i=${issueNumber}`)
      const { target, socket } = mountApp()
      socket.emitModel(lifecycleModel)
      flushSync()

      target.querySelector<HTMLButtonElement>(`button[aria-label="Remove ${active}"]`)!.click()
      await settle()

      expect(window.location.hash).toBe(expected)
    },
  )

  it('clears the route after successfully removing the only space', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn<typeof fetch>().mockResolvedValue(new Response(null, { status: 204 })),
    )
    window.history.replaceState(null, '', '/#s=only&i=44')
    const { target, socket } = mountApp()
    socket.emitModel({ spaces: [space('only', 44)] })
    flushSync()

    target.querySelector<HTMLButtonElement>('button[aria-label="Remove only"]')!.click()
    await settle()

    expect(window.location.hash).toBe('')
  })

  it('keeps the only-space removal route through repeated pre-removal snapshots', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn<typeof fetch>().mockResolvedValue(new Response(null, { status: 204 })),
    )
    window.history.replaceState(null, '', '/#s=only&i=44')
    const { target, socket } = mountApp()
    const onlySpaceModel = { spaces: [space('only', 44)] }
    socket.emitModel(onlySpaceModel)
    flushSync()

    target.querySelector<HTMLButtonElement>('button[aria-label="Remove only"]')!.click()
    await settle()
    expect(window.location.hash).toBe('')

    socket.emitModel(onlySpaceModel)
    flushSync()

    expect(window.location.hash).toBe('')
  })

  it('does not change the route when a mutation fails', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn<typeof fetch>().mockResolvedValue(new Response('Remove rejected', { status: 409 })),
    )
    window.history.replaceState(null, '', '/#s=middle&i=22')
    const { target, socket } = mountApp()
    socket.emitModel(lifecycleModel)
    flushSync()

    target.querySelector<HTMLButtonElement>('button[aria-label="Remove middle"]')!.click()
    await settle()

    expect(window.location.hash).toBe('#s=middle&i=22')
    expect(target.querySelector('[data-space-row="middle"]')?.textContent).toContain(
      'Remove rejected',
    )
  })

  it('reconciles a failed removal against the newer authoritative snapshot', async () => {
    let finishRemove!: (response: Response) => void
    vi.stubGlobal(
      'fetch',
      vi.fn<typeof fetch>().mockReturnValue(
        new Promise((resolve) => {
          finishRemove = resolve
        }),
      ),
    )
    window.history.replaceState(null, '', '/#s=middle&i=22')
    const { target, socket } = mountApp()
    socket.emitModel(lifecycleModel)
    flushSync()

    target.querySelector<HTMLButtonElement>('button[aria-label="Remove middle"]')!.click()
    flushSync()
    socket.emitModel({ spaces: [space('first', 11), space('last', 33)] })
    flushSync()
    expect(window.location.hash).toBe('#s=middle&i=22')

    finishRemove(new Response('Remove rejected', { status: 409 }))
    await settle()

    expect(window.location.hash).toBe('#s=first')
    expect(target.querySelector('[data-space-row="first"] [aria-current="true"]')).not.toBeNull()
    expect(target.querySelector('[aria-label="Issue details"]')).toBeNull()
  })

  it('keeps a stale cached space navigable with its map, detail, and provider error', () => {
    window.history.replaceState(null, '', '/#s=cached&i=55')
    const { target, socket } = mountApp()
    socket.emitModel({
      spaces: [
        {
          ...space('cached', 55),
          stale: true,
          error: 'GitHub rate limit exceeded',
        },
      ],
    })
    flushSync()

    expect(target.querySelector('[data-space-row="cached"]')?.textContent).toContain('Stale')
    expect(target.querySelector('[data-space-row="cached"]')?.textContent).toContain(
      'GitHub rate limit exceeded',
    )
    expect(target.querySelector('.star-map')).not.toBeNull()
    expect(target.querySelector('[aria-label="Issue details"]')?.textContent).toContain('#55')
  })

  it('keeps the add form available and shows a clear empty map when there are no spaces', () => {
    window.history.replaceState(null, '', '/#s=gone&i=99')
    const { target, socket } = mountApp()
    socket.emitModel({ spaces: [] })
    flushSync()

    expect(window.location.hash).toBe('')
    expect(target.querySelector('.star-map')).toBeNull()
    expect(target.querySelector('[aria-label="Issue map"]')?.textContent).toContain('No spaces yet')
    expect(target.querySelector('input[name="path"]')).not.toBeNull()
    expect(target.querySelector('input[name="repo"]')).not.toBeNull()
    expect(target.querySelector<HTMLButtonElement>('button[type="submit"]')?.disabled).toBe(true)
  })

  it('reconciles a now-missing routed space only when a new authoritative snapshot arrives', () => {
    window.history.replaceState(null, '', '/#s=second&i=22')
    const { target, socket } = mountApp()
    socket.emitModel(model)
    flushSync()

    expect(window.location.hash).toBe('#s=second&i=22')
    socket.emitModel({ spaces: [space('first', 11), space('last', 33)] })
    flushSync()

    expect(window.location.hash).toBe('#s=first')
    expect(target.querySelector('[data-space-row="first"] [aria-current="true"]')).not.toBeNull()
    expect(target.querySelector('[aria-label="Issue details"]')).toBeNull()
  })

  it('restores a valid deep link after the snapshot arrives', () => {
    const select = vi.spyOn(Renderer.prototype, 'select')
    window.history.replaceState(null, '', '/#s=second&i=22')
    const { target, socket } = mountApp()

    socket.emitModel(model)
    flushSync()

    expect(select).toHaveBeenLastCalledWith(22)
    expect(target.querySelector('[aria-label="Issue details"]')?.textContent).toContain('#22')
    expect(target.textContent).toContain('Detail for Issue 22')
  })

  it('clears an unknown issue while preserving the routed space', () => {
    window.history.replaceState(null, '', '/#s=second&i=999')
    const { target, socket } = mountApp()

    socket.emitModel(model)
    flushSync()

    expect(window.location.hash).toBe('#s=second')
    expect(target.querySelector('[aria-label="Issue details"]')).toBeNull()
  })

  it('opens and closes selected issue detail without remounting the map canvas', () => {
    const { target, socket } = mountApp()
    socket.emitModel(model)
    flushSync()
    window.dispatchEvent(new HashChangeEvent('hashchange'))
    flushSync()

    const canvas = target.querySelector<HTMLCanvasElement>('canvas')!
    canvas.dispatchEvent(new MouseEvent('mousedown', { clientX: 500, clientY: 350, bubbles: true }))
    canvas.dispatchEvent(new MouseEvent('mouseup', { clientX: 500, clientY: 350, bubbles: true }))
    window.dispatchEvent(new HashChangeEvent('hashchange'))
    flushSync()

    expect(window.location.hash).toBe('#s=first&i=11')
    expect(target.querySelector('[aria-label="Issue details"]')?.textContent).toContain('#11')
    expect(target.querySelector('canvas')).toBe(canvas)

    target
      .querySelector<HTMLButtonElement>('button[aria-label="Close issue details"]')!
      .dispatchEvent(new MouseEvent('click', { bubbles: true }))
    window.dispatchEvent(new HashChangeEvent('hashchange'))
    flushSync()

    expect(window.location.hash).toBe('#s=first')
    expect(target.querySelector('[aria-label="Issue details"]')).toBeNull()
    expect(target.querySelector('canvas')).toBe(canvas)
  })

  it('uses the hybrid policy to dock issue detail right or bottom', () => {
    window.history.replaceState(null, '', '/#s=second&i=22')
    const { target, socket } = mountApp()
    socket.emitModel(model)
    flushSync()

    const observer = FakeResizeObserver.instances[0]
    observer.emit(1000, 600)
    flushSync()
    expect(target.querySelector('.workspace')?.classList.contains('detail-right')).toBe(true)

    observer.emit(500, 800)
    flushSync()
    expect(target.querySelector('.workspace')?.classList.contains('detail-bottom')).toBe(true)
  })
})

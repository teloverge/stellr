import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { flushSync, mount, unmount } from 'svelte'
import App from './App.svelte'
import type { Model, SpaceModel, Star } from './lib/model'

class FakeWebSocket {
  static instances: FakeWebSocket[] = []

  onclose: ((event: CloseEvent) => void) | null = null
  onmessage: ((event: MessageEvent) => void) | null = null
  onopen: ((event: Event) => void) | null = null

  constructor(readonly url: string) {
    FakeWebSocket.instances.push(this)
  }

  close(): void {}

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
  if (clientWidth) Object.defineProperty(HTMLElement.prototype, 'clientWidth', clientWidth)
  if (clientHeight) Object.defineProperty(HTMLElement.prototype, 'clientHeight', clientHeight)
})

function issue(number: number): Star {
  return {
    number,
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

function mountApp(): { target: HTMLElement; socket: FakeWebSocket } {
  const target = document.createElement('div')
  document.body.appendChild(target)
  mounted.push(mount(App, { target }))
  flushSync()
  return { target, socket: FakeWebSocket.instances[0] }
}

describe('App issue routing', () => {
  it('routes an unaddressed snapshot to its first available space', () => {
    const { target, socket } = mountApp()

    socket.emitModel(model)
    flushSync()

    expect(window.location.hash).toBe('#s=first')
    expect(target.querySelector('.star-map')).not.toBeNull()
  })

  it('restores a valid deep link after the snapshot arrives', () => {
    window.history.replaceState(null, '', '/#s=second&i=22')
    const { target, socket } = mountApp()

    socket.emitModel(model)
    flushSync()

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

describe('App space lifecycle', () => {
  it('routes Sidebar selection to that space without carrying an issue', () => {
    window.history.replaceState(null, '', '/#s=first&i=11')
    const { target, socket } = mountApp()
    socket.emitModel(model)
    flushSync()

    target
      .querySelector<HTMLButtonElement>('button[data-space-id="second"]')!
      .dispatchEvent(new MouseEvent('click', { bubbles: true }))

    expect(window.location.hash).toBe('#s=second')
  })

  it('keeps a successfully added space selected while its snapshot is pending', async () => {
    window.history.replaceState(null, '', '/#s=first&i=11')
    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValue(
        new Response(JSON.stringify({ id: 'new-space' }), {
          status: 200,
          headers: { 'content-type': 'application/json' },
        }),
      ),
    )
    const { target, socket } = mountApp()
    socket.emitModel(model)
    flushSync()

    const repo = target.querySelector<HTMLInputElement>('input[name="repo"]')!
    repo.value = 'teloverge/new-space'
    repo.dispatchEvent(new Event('input', { bubbles: true }))
    flushSync()
    target.querySelector('form')!.dispatchEvent(new SubmitEvent('submit', { bubbles: true, cancelable: true }))

    await vi.waitFor(() => expect(window.location.hash).toBe('#s=new-space'))
    window.dispatchEvent(new HashChangeEvent('hashchange'))
    flushSync()
    expect(window.location.hash).toBe('#s=new-space')
  })

  it.each([
    ['first', 'second'],
    ['second', 'third'],
    ['third', 'second'],
  ])('removing active %s falls to %s and clears issue selection', async (removedId, nextId) => {
    window.history.replaceState(null, '', `/#s=${removedId}&i=${removedId === 'first' ? 11 : removedId === 'second' ? 22 : 33}`)
    vi.stubGlobal('fetch', vi.fn().mockImplementation(() => Promise.resolve(new Response(null, { status: 204 }))))
    const threeSpaces: Model = {
      spaces: [space('first', 11), space('second', 22), space('third', 33)],
    }
    const { target, socket } = mountApp()
    socket.emitModel(threeSpaces)
    flushSync()

    target
      .querySelector<HTMLButtonElement>(`[data-space-row="${removedId}"] [data-action="remove"]`)!
      .dispatchEvent(new MouseEvent('click', { bubbles: true }))

    await vi.waitFor(() => expect(window.location.hash).toBe(`#s=${nextId}`))
  })

  it('clears the route when the only active space is removed', async () => {
    window.history.replaceState(null, '', '/#s=only&i=7')
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(new Response(null, { status: 204 })))
    const { target, socket } = mountApp()
    socket.emitModel({ spaces: [space('only', 7)] })
    flushSync()

    target
      .querySelector<HTMLButtonElement>('[data-space-row="only"] [data-action="remove"]')!
      .dispatchEvent(new MouseEvent('click', { bubbles: true }))

    await vi.waitFor(() => expect(window.location.hash).toBe(''))
  })

  it('keeps the route unchanged and shows the local error when removal fails', async () => {
    window.history.replaceState(null, '', '/#s=first&i=11')
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(new Response('cannot remove', { status: 500 })))
    const { target, socket } = mountApp()
    socket.emitModel(model)
    flushSync()

    target
      .querySelector<HTMLButtonElement>('[data-space-row="first"] [data-action="remove"]')!
      .dispatchEvent(new MouseEvent('click', { bubbles: true }))

    await vi.waitFor(() =>
      expect(target.querySelector('[data-space-row="first"] [data-row-error]')?.textContent).toContain(
        'cannot remove',
      ),
    )
    expect(window.location.hash).toBe('#s=first&i=11')
  })

  it('keeps cached stale content navigable with its provider error visible', () => {
    window.history.replaceState(null, '', '/#s=cached&i=22')
    const cached = space('cached', 22)
    cached.stale = true
    cached.error = 'GitHub is unavailable; showing cached data'
    const { target, socket } = mountApp()

    socket.emitModel({ spaces: [cached] })
    flushSync()

    expect(target.querySelector('canvas')).not.toBeNull()
    expect(target.querySelector('[aria-label="Issue details"]')?.textContent).toContain('#22')
    expect(target.textContent).toContain('Stale')
    expect(target.textContent).toContain('GitHub is unavailable; showing cached data')
  })

  it('keeps the add form available beside a clear empty-map message', () => {
    const { target, socket } = mountApp()

    socket.emitModel({ spaces: [] })
    flushSync()

    expect(target.querySelector('form')).not.toBeNull()
    expect(target.textContent).toContain('Add a space to begin')
  })
})

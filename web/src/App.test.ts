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

function mountApp(): { target: HTMLElement; socket: FakeWebSocket; component: object } {
  const target = document.createElement('div')
  document.body.appendChild(target)
  const component = mount(App, { target })
  mounted.push(component)
  flushSync()
  return { target, socket: FakeWebSocket.instances[0], component }
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
    expect(target.querySelector('.star-map')).not.toBeNull()
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

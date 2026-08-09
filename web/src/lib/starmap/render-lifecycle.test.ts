import { afterEach, beforeEach, describe, expect, it, vi, type Mock } from 'vitest'
import type { Ticket } from './model'
import { StarMap } from './starmap'

const ticket = (status: Ticket['status'] = 'open'): Ticket => ({
  num: 1,
  slug: '1',
  title: 'Power-saving map',
  type: 'task',
  status,
  blockedBy: [],
  parentIssue: null,
  frontier: status === 'open',
})

function drawingContext(paint: () => void): CanvasRenderingContext2D {
  const values: Record<PropertyKey, unknown> = {
    createRadialGradient: () => ({ addColorStop: () => undefined }),
    fillRect: paint,
    measureText: () => ({ width: 40 }),
  }
  return new Proxy(values, {
    get(target, property) {
      if (property in target) return target[property]
      const method = () => undefined
      target[property] = method
      return method
    },
    set(target, property, value) {
      target[property] = value
      return true
    },
  }) as unknown as CanvasRenderingContext2D
}

describe('render lifecycle', () => {
  let nextFrame: number
  let frames: Map<number, FrameRequestCallback>
  let hostWidth: number
  let paint: Mock<() => void>
  let resize: ResizeObserverCallback
  let renderers: StarMap[]

  beforeEach(() => {
    nextFrame = 1
    frames = new Map()
    hostWidth = 1000
    paint = vi.fn()
    renderers = []
    vi.useFakeTimers({ toFake: ['performance'] })
    vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(drawingContext(paint))
    vi.stubGlobal('ResizeObserver', class {
      constructor(callback: ResizeObserverCallback) {
        resize = callback
      }
      observe(): void {}
      disconnect(): void {}
    })
    vi.stubGlobal('requestAnimationFrame', vi.fn((callback: FrameRequestCallback) => {
      const id = nextFrame++
      frames.set(id, callback)
      return id
    }))
    vi.stubGlobal('cancelAnimationFrame', vi.fn((id: number) => {
      frames.delete(id)
    }))
  })

  afterEach(() => {
    for (const renderer of renderers) renderer.destroy()
    vi.useRealTimers()
    vi.unstubAllGlobals()
    vi.restoreAllMocks()
    document.body.innerHTML = ''
  })

  function track(renderer: StarMap): StarMap {
    renderers.push(renderer)
    return renderer
  }

  function mounted(): StarMap {
    const host = document.createElement('div')
    Object.defineProperty(host, 'clientWidth', { get: () => hostWidth })
    Object.defineProperty(host, 'clientHeight', { value: 700 })
    document.body.appendChild(host)
    const renderer = track(new StarMap())
    renderer.mount(host)
    return renderer
  }

  function step(milliseconds = 16): void {
    vi.advanceTimersByTime(milliseconds)
    const entry = frames.entries().next().value as [number, FrameRequestCallback] | undefined
    if (!entry) throw new Error('expected a scheduled animation frame')
    frames.delete(entry[0])
    entry[1](performance.now())
  }

  it('cancels every frame while suspended and resumes exactly one loop', () => {
    const renderer = mounted()
    expect(frames.size).toBe(1)

    renderer.suspend()
    expect(frames.size).toBe(0)
    renderer.suspend()
    expect(frames.size).toBe(0)

    renderer.resume()
    expect(frames.size).toBe(1)
    renderer.resume()
    expect(frames.size).toBe(1)

    step()
    expect(frames.size).toBe(1)
    renderer.destroy()
    expect(frames.size).toBe(0)
  })

  it('can mount suspended and preserves map state until resume', () => {
    const renderer = track(new StarMap())
    renderer.suspend()
    const host = document.createElement('div')
    Object.defineProperty(host, 'clientWidth', { value: 1000 })
    Object.defineProperty(host, 'clientHeight', { value: 700 })
    document.body.appendChild(host)
    renderer.mount(host)
    renderer.setModel([ticket()])
    renderer.select(1)
    renderer.restoreCamera({ x: 120, y: 180, s: 1.5 })

    expect(frames.size).toBe(0)
    expect(renderer.positions()[1]).toBeDefined()
    expect(renderer.camera()).toEqual({ x: 120, y: 180, s: 1.5 })

    renderer.resume()
    expect(frames.size).toBe(1)
    expect(renderer.positions()[1]).toBeDefined()
    expect(renderer.camera()).toEqual({ x: 120, y: 180, s: 1.5 })
  })

  it('does not repaint from resize notifications while suspended', () => {
    const renderer = mounted()
    renderer.suspend()
    hostWidth = 800

    resize([], {} as ResizeObserver)

    expect(paint).not.toHaveBeenCalled()
    renderer.resume()
    step()
    expect(paint).toHaveBeenCalled()
  })

  it('freezes animation and ticker time for the full suspension', () => {
    const renderer = mounted()
    renderer.setModel([ticket()])
    step()
    renderer.suspend()
    renderer.setModel([ticket('claimed')])
    const ticker = renderer.ticker()

    vi.advanceTimersByTime(10 * 60 * 1000)
    expect(renderer.ticker()).toBe(ticker)

    renderer.resume()
    step()
    expect(renderer.ticker()).toBe(ticker)
  })
})

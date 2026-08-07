import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { StarMap } from './starmap'
import type { Ticket } from './model'

type Arc = { x: number; y: number; radius: number }
type Gradient = {
  kind: 'radial-gradient'
  origin: { x0: number; y0: number; r0: number; x1: number; y1: number; r1: number }
  stops: Array<{ at: number; color: string }>
}
type Fill = { kind: 'fill'; style: string | Gradient; arc: Arc | undefined }
type Stroke = { kind: 'stroke'; style: string | Gradient; lineWidth: number; arc: Arc | undefined }
type Paint = Fill | Stroke

const RESOLVED: Ticket = {
  num: 1, slug: '1', title: 'Resolved', type: 'issue', status: 'resolved',
  frontier: false, blockedBy: [], parentIssue: null, workPriority: 'terminal',
}
const IN_PROGRESS: Ticket = {
  num: 2, slug: '2', title: 'In progress', type: 'issue', status: 'open',
  frontier: false, blockedBy: [], parentIssue: null, workPriority: 'in_progress',
}
const READY: Ticket = {
  num: 3, slug: '3', title: 'Ready', type: 'issue', status: 'open',
  frontier: true, blockedBy: [], parentIssue: null, readyForAgent: true, workPriority: 'ready',
}
const BLOCKED: Ticket = {
  num: 4, slug: '4', title: 'Blocked', type: 'issue', status: 'open',
  frontier: false, blockedBy: [], parentIssue: null, workPriority: 'blocked',
}
const READY_CHILD: Ticket = { ...READY, num: 5, slug: '5', parentIssue: 99 }

function recordingContext() {
  const paints: Paint[] = []
  const texts: string[] = []
  let arc: Arc | undefined
  const ctx: Record<string, unknown> = {
    createRadialGradient: (
      x0: number, y0: number, r0: number, x1: number, y1: number, r1: number,
    ) => {
      const gradient: Gradient = {
        kind: 'radial-gradient',
        origin: { x0, y0, r0, x1, y1, r1 },
        stops: [],
      }
      Object.defineProperty(gradient, 'addColorStop', {
        value: (at: number, color: string) => gradient.stops.push({ at, color }),
      })
      return gradient
    },
    beginPath: () => { arc = undefined },
    arc: (x: number, y: number, radius: number) => { arc = { x, y, radius } },
    fill: () => paints.push({ kind: 'fill', style: ctx.fillStyle as string | Gradient, arc }),
    stroke: () => paints.push({
      kind: 'stroke',
      style: ctx.strokeStyle as string | Gradient,
      lineWidth: ctx.lineWidth as number,
      arc,
    }),
    measureText: () => ({ width: 40 }),
    fillRect: () => {},
    fillText: (text: string) => texts.push(text),
  }
  for (const method of [
    'setTransform', 'moveTo', 'lineTo', 'closePath', 'quadraticCurveTo', 'setLineDash',
    'save', 'restore', 'translate', 'scale', 'rotate',
  ]) ctx[method] = () => {}
  return { ctx, paints, texts }
}

function bodyFill(paints: Paint[]): Fill {
  return paints.find((paint): paint is Fill =>
    paint.kind === 'fill' &&
    typeof paint.style !== 'string' &&
    paint.style.stops.some((stop) => stop.at === 0.48),
  )!
}

function outerRings(paints: Paint[]): Stroke[] {
  const body = bodyFill(paints)
  return paints.filter((paint): paint is Stroke =>
    paint.kind === 'stroke' && (paint.arc?.radius ?? 0) > body.arc!.radius,
  )
}

describe('issue core visual grammar', () => {
  let frames: FrameRequestCallback[] = []
  let getContext: ReturnType<typeof vi.spyOn>
  let now: ReturnType<typeof vi.spyOn>

  beforeEach(() => {
    frames = []
    now = vi.spyOn(performance, 'now').mockReturnValue(0)
    vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
      frames.push(callback)
      return frames.length
    })
    vi.stubGlobal('cancelAnimationFrame', () => {})
    getContext = vi.spyOn(HTMLCanvasElement.prototype, 'getContext')
  })

  afterEach(() => {
    getContext.mockRestore()
    now.mockRestore()
    vi.unstubAllGlobals()
    document.body.replaceChildren()
  })

  function paint(
    ticket: Ticket,
    currentIssue: number | null = null,
    selectedIssue: number | null = null,
  ) {
    const recording = recordingContext()
    getContext.mockReturnValue(recording.ctx as never)
    const host = document.createElement('div')
    Object.defineProperty(host, 'clientWidth', { value: 1000 })
    Object.defineProperty(host, 'clientHeight', { value: 700 })
    document.body.appendChild(host)
    const map = new StarMap()
    map.mount(host)
    map.setModel([ticket], {}, currentIssue)
    if (selectedIssue !== null) map.select(selectedIssue)
    const frame = frames.pop()
    frames = []
    frame?.(0)
    map.destroy()
    return recording
  }

  it('layers glow, contact shadow, body, specular, and an internal boundary', () => {
    const { paints } = paint(READY)
    const body = bodyFill(paints)
    const bodyIndex = paints.indexOf(body)
    const glow = paints[0] as Fill
    const shadow = paints[1] as Fill
    const specular = paints[bodyIndex + 1] as Fill
    const boundary = paints[bodyIndex + 2] as Stroke

    expect([glow.kind, shadow.kind, body.kind, specular.kind, boundary.kind]).toEqual([
      'fill', 'fill', 'fill', 'fill', 'stroke',
    ])
    expect((body.style as Gradient).stops).toEqual([
      { at: 0, color: 'rgba(138,216,255,1)' },
      { at: 0.48, color: 'rgba(138,216,255,0.98)' },
      { at: 0.82, color: 'rgba(47,155,224,0.92)' },
      { at: 1, color: 'rgba(47,155,224,0.62)' },
    ])
    expect((body.style as Gradient).origin.x0).toBeLessThan(body.arc!.x)
    expect((body.style as Gradient).origin.y0).toBeLessThan(body.arc!.y)
    expect(shadow.arc!.y).toBeGreaterThan(body.arc!.y)
    expect(specular.arc!.x).toBeLessThan(body.arc!.x)
    expect(specular.arc!.y).toBeLessThan(body.arc!.y)
    expect(specular.arc!.radius).toBeLessThan(body.arc!.radius / 3)
    expect(boundary.arc!.radius).toBeLessThan(body.arc!.radius)
  })

  it('uses at most one status ring and one selection ring', () => {
    expect(outerRings(paint(IN_PROGRESS).paints)).toHaveLength(1)
    expect(outerRings(paint(READY_CHILD).paints)).toHaveLength(1)
    expect(outerRings(paint(BLOCKED).paints)).toHaveLength(0)
    expect(outerRings(paint(RESOLVED).paints)).toHaveLength(0)
    expect(outerRings(paint({ ...BLOCKED, parentIssue: 99 }).paints)).toHaveLength(0)
    expect(outerRings(paint(READY_CHILD, null, READY_CHILD.num).paints)).toHaveLength(2)
    expect(outerRings(paint(BLOCKED, null, BLOCKED.num).paints)).toHaveLength(1)
  })

  it('keeps CURRENT as label semantics without drawing CURRENT rings', () => {
    const { paints, texts } = paint(BLOCKED, BLOCKED.num)

    expect(outerRings(paints)).toHaveLength(0)
    expect(texts.some((text) => text.startsWith('CURRENT'))).toBe(true)
  })

  it('does not turn a selected state-change flare into another ring', () => {
    const recording = recordingContext()
    getContext.mockReturnValue(recording.ctx as never)
    const host = document.createElement('div')
    Object.defineProperty(host, 'clientWidth', { value: 1000 })
    Object.defineProperty(host, 'clientHeight', { value: 700 })
    document.body.appendChild(host)
    const map = new StarMap()
    map.mount(host)
    map.setModel([BLOCKED])
    map.setModel([{ ...BLOCKED, workPriority: 'in_progress' }])
    map.select(BLOCKED.num)
    now.mockReturnValue(16)
    const frame = frames.pop()
    frames = []
    frame?.(0)

    const rings = outerRings(recording.paints)
    const bodyRadius = bodyFill(recording.paints).arc!.radius
    map.destroy()

    expect(rings).toHaveLength(2)
    expect(Math.min(...rings.map((ring) => ring.arc!.radius))).toBeGreaterThanOrEqual(bodyRadius + 7)
  })

  it('holds animated glow geometry and alpha still under reduced motion', () => {
    const recording = recordingContext()
    getContext.mockReturnValue(recording.ctx as never)
    const host = document.createElement('div')
    Object.defineProperty(host, 'clientWidth', { value: 1000 })
    Object.defineProperty(host, 'clientHeight', { value: 700 })
    document.body.appendChild(host)
    const map = new StarMap()
    map.setReducedMotion(true)
    map.mount(host)
    map.setModel([BLOCKED])
    map.setModel([{ ...BLOCKED, workPriority: 'in_progress' }])

    const takeGlow = () => {
      recording.paints.length = 0
      const frame = frames.shift()
      frame?.(0)
      return structuredClone(recording.paints[0])
    }

    const first = takeGlow()
    const second = takeGlow()
    map.destroy()
    expect(second).toEqual(first)
  })
})

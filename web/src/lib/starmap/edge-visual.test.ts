import { afterEach, describe, expect, it, vi } from 'vitest'
import { StarMap } from './starmap'
import type { Ticket } from './model'

type Point = { x: number; y: number }
type Curve = { control: Point; end: Point }
type Stroke = {
  color: string
  width: number
  dash: number[]
  cap: string
  alpha: number
  points: Point[]
  curves: Curve[]
}
type Fill = { color: unknown; alpha: number; arcs: Array<{ x: number; y: number; radius: number }>; points: Point[] }

function recordingContext(): { ctx: Record<string, unknown>; strokes: Stroke[]; fills: Fill[] } {
  const strokes: Stroke[] = []
  const fills: Fill[] = []
  let color = ''
  let width = 1
  let dash: number[] = []
  let cap = 'butt'
  let alpha = 1
  let points: Point[] = []
  let curves: Curve[] = []
  let arcs: Array<{ x: number; y: number; radius: number }> = []
  const saved: Array<{ alpha: number }> = []
  const ctx: Record<string, unknown> = {
    createRadialGradient: () => ({ addColorStop: () => {} }),
    measureText: (text: string) => ({ width: text.length * 6 }),
    setTransform: () => {},
    fillRect: () => {},
    translate: () => {},
    scale: () => {},
    rotate: () => {},
    beginPath: () => { points = []; curves = []; arcs = [] },
    moveTo: (x: number, y: number) => points.push({ x, y }),
    lineTo: (x: number, y: number) => points.push({ x, y }),
    quadraticCurveTo: (cx: number, cy: number, x: number, y: number) => {
      curves.push({ control: { x: cx, y: cy }, end: { x, y } })
      points.push({ x, y })
    },
    arc: (x: number, y: number, radius: number) => arcs.push({ x, y, radius }),
    closePath: () => {},
    stroke: () => strokes.push({ color, width, dash: [...dash], cap, alpha, points: [...points], curves: [...curves] }),
    fill: () => fills.push({ color, alpha, arcs: [...arcs], points: [...points] }),
    setLineDash: (next: number[]) => { dash = [...next] },
    save: () => saved.push({ alpha }),
    restore: () => { alpha = saved.pop()?.alpha ?? 1 },
    fillText: () => {},
  }
  Object.defineProperties(ctx, {
    strokeStyle: { set: (next: string) => { color = next } },
    fillStyle: { set: (next: string) => { color = next } },
    lineWidth: { set: (next: number) => { width = next } },
    lineCap: { set: (next: string) => { cap = next } },
    globalAlpha: { set: (next: number) => { alpha = next } },
  })
  return { ctx, strokes, fills }
}

const EDGE_FIXTURE: Ticket[] = [
  { num: 1, slug: '1', title: 'resolved blocker', type: 'task', status: 'resolved', blockedBy: [], parentIssue: null, frontier: false, readyForAgent: true },
  { num: 2, slug: '2', title: 'focused dependent', type: 'task', status: 'open', blockedBy: [1], parentIssue: null, frontier: false },
  { num: 3, slug: '3', title: 'unresolved blocker', type: 'task', status: 'open', blockedBy: [], parentIssue: null, frontier: true },
  { num: 4, slug: '4', title: 'context dependent', type: 'task', status: 'open', blockedBy: [3], parentIssue: null, frontier: false },
]

describe('dependency-edge visual treatment', () => {
  const realGetContext = HTMLCanvasElement.prototype.getContext
  const realRaf = globalThis.requestAnimationFrame
  const realCancelRaf = globalThis.cancelAnimationFrame
  let frames: FrameRequestCallback[] = []

  function installFrameHarness(): void {
    frames = []
    globalThis.requestAnimationFrame = ((cb: FrameRequestCallback) => {
      frames.push(cb)
      return frames.length
    }) as never
    globalThis.cancelAnimationFrame = (() => {}) as never
    vi.spyOn(performance, 'now').mockReturnValue(1_000)
  }

  function paint(tickets: Ticket[], currentIssue: number | null = null): { strokes: Stroke[]; fills: Fill[] } {
    const { ctx, strokes, fills } = recordingContext()
    HTMLCanvasElement.prototype.getContext = (() => ctx) as never
    frames = []
    const host = document.createElement('div')
    Object.defineProperties(host, {
      clientWidth: { value: 1000 },
      clientHeight: { value: 700 },
    })
    document.body.appendChild(host)
    const map = new StarMap()
    map.mount(host)
    map.setModel(tickets, {}, currentIssue)
    frames.shift()!(1_000)
    map.destroy()
    return { strokes, fills }
  }

  function sequenceFixture(sourceStatus: Ticket['status'], destinationStatus: Ticket['status']): Ticket[] {
    return [
      { num: 16, slug: '16', title: 'parent', type: 'issue', status: 'open', blockedBy: [], parentIssue: null, frontier: false },
      { num: 37, slug: '37', title: 'source', type: 'task', status: sourceStatus, blockedBy: [], parentIssue: 16, frontier: false },
      {
        num: 38,
        slug: '38',
        title: 'destination',
        type: 'task',
        status: destinationStatus,
        blockedBy: [37],
        parentIssue: 16,
        frontier: destinationStatus === 'frontier',
        readyForAgent: destinationStatus === 'frontier',
      },
    ]
  }

  function expectParticleMotion(render: { fills: Fill[] }, edgeAlpha: number, edgePhase = 0.27): void {
    const halos = render.fills.filter((fill) => typeof fill.color === 'string' && fill.color.startsWith('rgba(190,225,200,') && fill.arcs[0]?.radius === 5)
    const cores = render.fills.filter((fill) => typeof fill.color === 'string' && fill.color.startsWith('rgba(220,255,230,') && fill.arcs[0]?.radius === 2.6)
    expect(halos).toHaveLength(3)
    expect(cores).toHaveLength(3)
    for (let index = 0; index < 3; index++) {
      const u = (0.1 + index / 3 + edgePhase) % 1
      const expectedHalo = 0.14 + 0.18 * Math.sin(Math.PI * u)
      const expectedCore = 0.45 + 0.5 * Math.sin(Math.PI * u)
      const halo = halos[index]
      const core = cores[index]
      const haloAlpha = Number((halo.color as string).slice((halo.color as string).lastIndexOf(',') + 1, -1))
      const coreAlpha = Number((core.color as string).slice((core.color as string).lastIndexOf(',') + 1, -1))
      expect(halo.alpha).toBe(edgeAlpha)
      expect(core.alpha).toBe(edgeAlpha)
      expect(haloAlpha * halo.alpha).toBeCloseTo(expectedHalo * edgeAlpha)
      expect(coreAlpha * core.alpha).toBeCloseTo(expectedCore * edgeAlpha)
      expect(core.arcs[0]).toMatchObject({ x: halo.arcs[0].x, y: halo.arcs[0].y })
    }
  }

  afterEach(() => {
    HTMLCanvasElement.prototype.getContext = realGetContext
    globalThis.requestAnimationFrame = realRaf
    globalThis.cancelAnimationFrame = realCancelRaf
    vi.restoreAllMocks()
    document.body.replaceChildren()
  })

  it('renders focused resolved and contextual unresolved dependency edges distinctly', () => {
    installFrameHarness()

    const focused = paint(EDGE_FIXTURE, 2)
    const { strokes, fills } = focused

    const resolved = strokes.find((stroke) => stroke.color === 'rgba(190,225,200,0.82)')!
    const unresolved = strokes.find((stroke) => stroke.color === 'rgba(174,192,218,0.62)')!
    expect(resolved).toMatchObject({ width: 3, dash: [], cap: 'round', alpha: 1 })
    expect(unresolved).toMatchObject({ width: 2.4, dash: [7, 7], cap: 'round', alpha: 0.45 })

    const resolvedArrow = fills.find((fill) => fill.color === '#d9f3df')!
    const unresolvedArrow = fills.find((fill) => fill.color === '#c8d5e8')!
    expect(resolvedArrow.alpha).toBe(1)
    expect(unresolvedArrow.alpha).toBe(0.45)
    for (const arrow of [resolvedArrow, unresolvedArrow]) {
      expect(arrow.points).toHaveLength(3)
      const [tip, baseA, baseB] = arrow.points
      const base = { x: (baseA.x + baseB.x) / 2, y: (baseA.y + baseB.y) / 2 }
      expect(Math.hypot(tip.x - base.x, tip.y - base.y)).toBeCloseTo(12)
      expect(Math.hypot(baseA.x - base.x, baseA.y - base.y)).toBeCloseTo(6.5)
    }

    expectParticleMotion(focused, 1)
    // In this frame the only contextual edge is unresolved (3 → 4), so a
    // contextual radius-5/2.6 paint would prove unresolved motion leaked in.
    expect(fills.filter((fill) => fill.alpha === 0.45 && [5, 2.6].includes(fill.arcs[0]?.radius ?? 0))).toEqual([])

    // Current issue 4 has no ready-to-current path: both edges are context.
    // This makes the resolved 1 → 2 particle flow itself prove the multiplier.
    expectParticleMotion(paint(EDGE_FIXTURE, 4), 0.45)
  })

  it('animates exactly three halo/core pairs on a traversed sequence edge at full path alpha', () => {
    installFrameHarness()

    expectParticleMotion(paint(sequenceFixture('resolved', 'frontier'), 16), 1, 0.47)
  })

  it('does not animate a resolved sequence source whose destination remains blocked', () => {
    installFrameHarness()

    const render = paint(sequenceFixture('resolved', 'blocked'), 16)
    expect(render.fills.filter((fill) => [5, 2.6].includes(fill.arcs[0]?.radius ?? 0))).toEqual([])
  })

  it('does not animate an open sequence source whose destination is frontier', () => {
    installFrameHarness()

    const render = paint(sequenceFixture('open', 'frontier'), 16)
    expect(render.fills.filter((fill) => [5, 2.6].includes(fill.arcs[0]?.radius ?? 0))).toEqual([])
  })

  it('multiplies contextual traversed sequence particles by context edge alpha', () => {
    installFrameHarness()
    const contextual = [
      ...sequenceFixture('resolved', 'frontier'),
      { num: 99, slug: '99', title: 'unrelated current', type: 'task', status: 'open', blockedBy: [], parentIssue: null, frontier: false },
    ] satisfies Ticket[]

    expectParticleMotion(paint(contextual, 99), 0.45, 0.47)
  })

  it('renders lone-child workflow loops as state-aware directed curves with completed motion gated', () => {
    installFrameHarness()

    function paintChild(childStatus: Ticket['status']): { strokes: Stroke[]; fills: Fill[] } {
      return paint([
        { num: 16, slug: '16', title: 'parent', type: 'issue', status: 'open', blockedBy: [], parentIssue: null, frontier: false },
        { num: 37, slug: '37', title: 'child', type: 'task', status: childStatus, blockedBy: [], parentIssue: 16, frontier: false },
      ])
    }

    const incomplete = paintChild('open')
    const violetStrokes = incomplete.strokes.filter((stroke) => stroke.color === 'rgba(170,145,255,0.78)')
    const violetArrows = incomplete.fills.filter((fill) => fill.color === '#c7b8ff')
    expect(violetStrokes).toHaveLength(2)
    expect(violetArrows).toHaveLength(2)
    for (const stroke of violetStrokes) {
      expect(stroke).toMatchObject({ width: 2.6, dash: [8, 7], cap: 'round', alpha: 1 })
      expect(stroke.curves).toHaveLength(1)
    }

    const firstCurve = violetStrokes[0].curves[0]
    const secondCurve = violetStrokes[1].curves[0]
    const sharedMidpoint = {
      x: (violetStrokes[0].points[0].x + firstCurve.end.x) / 2,
      y: (violetStrokes[0].points[0].y + firstCurve.end.y) / 2,
    }
    expect(firstCurve.control.x + secondCurve.control.x).toBeCloseTo(sharedMidpoint.x * 2)
    expect(firstCurve.control.y + secondCurve.control.y).toBeCloseTo(sharedMidpoint.y * 2)

    for (let index = 0; index < violetArrows.length; index++) {
      const [tip, baseA, baseB] = violetArrows[index].points
      const base = { x: (baseA.x + baseB.x) / 2, y: (baseA.y + baseB.y) / 2 }
      const stroke = violetStrokes[index]
      const start = stroke.points[0]
      const control = stroke.curves[0].control
      const end = stroke.curves[0].end
      expect((tip.x - base.x) * (end.x - start.x) + (tip.y - base.y) * (end.y - start.y)).toBeGreaterThan(0)

      const arrowLength = Math.hypot(tip.x - base.x, tip.y - base.y)
      const tangent = {
        x: control.x - start.x + (end.x - control.x),
        y: control.y - start.y + (end.y - control.y),
      }
      const tangentLength = Math.hypot(tangent.x, tangent.y)
      const cross = Math.abs(
        ((tip.x - base.x) / arrowLength) * (tangent.y / tangentLength) -
          ((tip.y - base.y) / arrowLength) * (tangent.x / tangentLength),
      )
      const dot =
        ((tip.x - base.x) / arrowLength) * (tangent.x / tangentLength) +
        ((tip.y - base.y) / arrowLength) * (tangent.y / tangentLength)
      const angle = Math.atan2(cross, dot)
      expect(angle).toBeLessThan(0.01)
    }

    const completed = paintChild('resolved')
    const mintStrokes = completed.strokes.filter((stroke) => stroke.color === 'rgba(190,225,200,0.82)')
    const mintArrows = completed.fills.filter((fill) => fill.color === '#d9f3df')
    expect(mintStrokes).toHaveLength(2)
    expect(mintArrows).toHaveLength(2)
    for (const stroke of mintStrokes) {
      expect(stroke).toMatchObject({ width: 3, dash: [], cap: 'round', alpha: 1 })
      expect(stroke.curves).toHaveLength(1)
    }
    expect(completed.fills.filter((fill) => [5, 2.6].includes(fill.arcs[0]?.radius ?? 0))).toEqual([])
  })
})

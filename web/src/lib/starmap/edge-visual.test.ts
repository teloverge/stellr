import { afterEach, describe, expect, it, vi } from 'vitest'
import { StarMap } from './starmap'
import type { Ticket } from './model'

type Point = { x: number; y: number }
type Stroke = { color: string; width: number; dash: number[]; cap: string; alpha: number; points: Point[] }
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
    beginPath: () => { points = []; arcs = [] },
    moveTo: (x: number, y: number) => points.push({ x, y }),
    lineTo: (x: number, y: number) => points.push({ x, y }),
    quadraticCurveTo: (_cx: number, _cy: number, x: number, y: number) => points.push({ x, y }),
    arc: (x: number, y: number, radius: number) => arcs.push({ x, y, radius }),
    closePath: () => {},
    stroke: () => strokes.push({ color, width, dash: [...dash], cap, alpha, points: [...points] }),
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

  afterEach(() => {
    HTMLCanvasElement.prototype.getContext = realGetContext
    globalThis.requestAnimationFrame = realRaf
    globalThis.cancelAnimationFrame = realCancelRaf
    vi.restoreAllMocks()
    document.body.replaceChildren()
  })

  it('renders focused resolved and contextual unresolved dependency edges distinctly', () => {
    let frames: FrameRequestCallback[] = []
    globalThis.requestAnimationFrame = ((cb: FrameRequestCallback) => {
      frames.push(cb)
      return frames.length
    }) as never
    globalThis.cancelAnimationFrame = (() => {}) as never
    vi.spyOn(performance, 'now').mockReturnValue(1_000)

    function paint(currentIssue: number): { strokes: Stroke[]; fills: Fill[] } {
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
      map.setModel(EDGE_FIXTURE, {}, currentIssue)
      frames.shift()!(1_000)
      return { strokes, fills }
    }

    const focused = paint(2)
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

    function expectResolvedMotion(render: { fills: Fill[] }, edgeAlpha: number): void {
      const halos = render.fills.filter((fill) => typeof fill.color === 'string' && fill.color.startsWith('rgba(190,225,200,') && fill.arcs[0]?.radius === 5)
      const cores = render.fills.filter((fill) => typeof fill.color === 'string' && fill.color.startsWith('rgba(220,255,230,') && fill.arcs[0]?.radius === 2.6)
      expect(halos).toHaveLength(3)
      expect(cores).toHaveLength(3)
      for (let index = 0; index < 3; index++) {
        const u = (0.1 + index / 3 + 0.27) % 1
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

    expectResolvedMotion(focused, 1)
    // In this frame the only contextual edge is unresolved (3 → 4), so a
    // contextual radius-5/2.6 paint would prove unresolved motion leaked in.
    expect(fills.filter((fill) => fill.alpha === 0.45 && [5, 2.6].includes(fill.arcs[0]?.radius ?? 0))).toEqual([])

    // Current issue 4 has no ready-to-current path: both edges are context.
    // This makes the resolved 1 → 2 particle flow itself prove the multiplier.
    expectResolvedMotion(paint(4), 0.45)
  })
})

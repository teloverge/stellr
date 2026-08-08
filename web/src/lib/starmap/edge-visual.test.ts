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

const SELECTION_FIXTURE: Ticket[] = [
  { num: 1, slug: '1', title: 'resolved source', type: 'task', status: 'resolved', blockedBy: [], parentIssue: null, frontier: false },
  { num: 2, slug: '2', title: 'selected middle', type: 'task', status: 'open', blockedBy: [1], parentIssue: null, frontier: false },
  { num: 3, slug: '3', title: 'blocked destination', type: 'task', status: 'open', blockedBy: [2], parentIssue: null, frontier: false },
  { num: 4, slug: '4', title: 'unrelated source', type: 'task', status: 'open', blockedBy: [], parentIssue: null, frontier: true },
  { num: 5, slug: '5', title: 'unrelated current', type: 'task', status: 'open', blockedBy: [4], parentIssue: null, frontier: false },
  { num: 6, slug: '6', title: 'isolated', type: 'task', status: 'open', blockedBy: [], parentIssue: null, frontier: true },
  { num: 7, slug: '7', title: 'transitive destination', type: 'task', status: 'open', blockedBy: [3], parentIssue: null, frontier: false },
]

function edgeStrokes(render: { strokes: Stroke[] }): Stroke[] {
  return render.strokes.filter((stroke) =>
    ['rgba(150,178,160,0.36)', 'rgba(174,192,218,0.62)', 'rgba(170,145,255,0.78)'].includes(stroke.color),
  )
}

function arrowDimensions(fill: Fill): { length: number; halfWidth: number } {
  const [tip, baseA, baseB] = fill.points
  const base = { x: (baseA.x + baseB.x) / 2, y: (baseA.y + baseB.y) / 2 }
  return {
    length: Math.hypot(tip.x - base.x, tip.y - base.y),
    halfWidth: Math.hypot(baseA.x - base.x, baseA.y - base.y),
  }
}

function expectArrowDimensions(fill: Fill, length: number, halfWidth: number): void {
  const dimensions = arrowDimensions(fill)
  expect(dimensions.length).toBeCloseTo(length)
  expect(dimensions.halfWidth).toBeCloseTo(halfWidth)
}

describe('dependency-edge visual treatment', () => {
  const realGetContext = HTMLCanvasElement.prototype.getContext
  const realRaf = globalThis.requestAnimationFrame
  const realCancelRaf = globalThis.cancelAnimationFrame
  let frames: FrameRequestCallback[] = []

  function installFrameHarness(): ReturnType<typeof vi.spyOn> {
    frames = []
    globalThis.requestAnimationFrame = ((cb: FrameRequestCallback) => {
      frames.push(cb)
      return frames.length
    }) as never
    globalThis.cancelAnimationFrame = (() => {}) as never
    return vi.spyOn(performance, 'now').mockReturnValue(1_000)
  }

  function paintFrames(
    tickets: Ticket[],
    currentIssue: number | null = null,
    steps: readonly ((map: StarMap) => void)[],
  ): Array<{ strokes: Stroke[]; fills: Fill[] }> {
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
    const renders = steps.map((step) => {
      step(map)
      strokes.length = 0
      fills.length = 0
      frames.shift()!(1_000)
      return { strokes: [...strokes], fills: [...fills] }
    })
    map.destroy()
    return renders
  }

  function paint(
    tickets: Ticket[],
    currentIssue: number | null = null,
    selections: readonly (number | null)[] = [],
  ): { strokes: Stroke[]; fills: Fill[] } {
    return paintFrames(tickets, currentIssue, [
      (map) => {
        for (const selection of selections) map.select(selection)
      },
    ])[0]
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
    const particles = render.fills.filter((fill) =>
      typeof fill.color === 'string' &&
      fill.color.startsWith('rgba(190,218,198,') &&
      fill.arcs[0]?.radius === 1.8,
    )
    expect(particles).toHaveLength(2)
    for (let index = 0; index < 2; index++) {
      const u = (0.1 + index / 2 + edgePhase) % 1
      const expectedAlpha = 0.35 + 0.4 * Math.sin(Math.PI * u)
      const particle = particles[index]
      const particleAlpha = Number(
        (particle.color as string).slice((particle.color as string).lastIndexOf(',') + 1, -1),
      )
      expect(particle.alpha).toBe(edgeAlpha)
      expect(particleAlpha * particle.alpha).toBeCloseTo(expectedAlpha * edgeAlpha)
    }
  }

  afterEach(() => {
    HTMLCanvasElement.prototype.getContext = realGetContext
    globalThis.requestAnimationFrame = realRaf
    globalThis.cancelAnimationFrame = realCancelRaf
    vi.restoreAllMocks()
    vi.unstubAllGlobals()
    document.body.replaceChildren()
  })

  it('renders focused resolved and contextual unresolved dependency edges distinctly', () => {
    installFrameHarness()

    const focused = paint(EDGE_FIXTURE, 2)
    const { strokes, fills } = focused

    const resolved = strokes.find((stroke) => stroke.color === 'rgba(150,178,160,0.36)')!
    const unresolved = strokes.find((stroke) => stroke.color === 'rgba(174,192,218,0.62)')!
    expect(resolved).toMatchObject({ width: 1.6, dash: [], cap: 'round', alpha: 1 })
    expect(unresolved).toMatchObject({ width: 2.4, dash: [7, 7], cap: 'round', alpha: 0.45 })

    const resolvedArrow = fills.find((fill) => fill.color === 'rgba(190,218,198,0.52)')!
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
    expect(paint(EDGE_FIXTURE, 4).fills.filter((fill) => fill.arcs[0]?.radius === 1.8)).toEqual([])
  })

  it('scopes selection emphasis to direct dependency edges and restores ordinary treatment', () => {
    installFrameHarness()

    const [selected, deselectedRender] = paintFrames(SELECTION_FIXTURE, 5, [
      (map) => map.select(2),
      (map) => map.select(null),
    ])
    const selectedEdges = edgeStrokes(selected)
    expect(selectedEdges.map(({ color, width, alpha }) => ({ color, width, alpha }))).toEqual([
      { color: 'rgba(174,192,218,0.62)', width: 2.4, alpha: 0.45 },
      { color: 'rgba(174,192,218,0.62)', width: 2.4, alpha: 0.45 },
      { color: 'rgba(150,178,160,0.36)', width: 2.72, alpha: 1 },
      { color: 'rgba(174,192,218,0.62)', width: 4.08, alpha: 1 },
    ])
    expect(
      selectedEdges.find((stroke) => stroke.color === 'rgba(150,178,160,0.36)'),
    ).toMatchObject({ width: 2.72, dash: [], alpha: 1 })
    const unresolvedEdges = selectedEdges.filter(
      (stroke) => stroke.color === 'rgba(174,192,218,0.62)',
    )
    expect(unresolvedEdges.find((stroke) => stroke.width === 4.08)).toMatchObject({
      dash: [7, 7],
      alpha: 1,
    })
    expect(unresolvedEdges.find((stroke) => stroke.width === 2.4)).toMatchObject({
      dash: [7, 7],
      alpha: 0.45,
    })

    const resolvedArrow = selected.fills.find((fill) => fill.color === 'rgba(190,218,198,0.52)')!
    const unresolvedArrows = selected.fills.filter((fill) => fill.color === '#c8d5e8')
    expect(resolvedArrow.alpha).toBe(1)
    expectArrowDimensions(resolvedArrow, 15, 8.125)
    const selectedUnresolvedArrow = unresolvedArrows.find((fill) => fill.alpha === 1)!
    const contextUnresolvedArrow = unresolvedArrows.find((fill) => fill.alpha === 0.45)!
    expectArrowDimensions(selectedUnresolvedArrow, 15, 8.125)
    expectArrowDimensions(contextUnresolvedArrow, 12, 6.5)
    expect(selected.fills.filter((fill) => fill.arcs[0]?.radius === 1.8)).toEqual([])

    const deselected = edgeStrokes(deselectedRender)
    expect(deselected.map(({ width, alpha }) => ({ width, alpha }))).toEqual([
      { width: 1.6, alpha: 0.45 },
      { width: 2.4, alpha: 0.45 },
      { width: 2.4, alpha: 0.45 },
      { width: 2.4, alpha: 0.45 },
    ])
    const isolated = edgeStrokes(paint(SELECTION_FIXTURE, 5, [6]))
    expect(isolated.map(({ width, alpha }) => ({ width, alpha }))).toEqual(
      deselected.map(({ width, alpha }) => ({ width, alpha })),
    )
  })

  it('does not restore edge emphasis when a removed selection returns in a later model', () => {
    installFrameHarness()

    const [, , restored] = paintFrames(SELECTION_FIXTURE, 5, [
      (map) => map.select(2),
      (map) => map.setModel(SELECTION_FIXTURE.filter((ticket) => ticket.num !== 2), {}, 5),
      (map) => map.setModel(SELECTION_FIXTURE, {}, 5),
    ])

    expect(edgeStrokes(restored).map(({ width, alpha }) => ({ width, alpha }))).toEqual([
      { width: 1.6, alpha: 0.45 },
      { width: 2.4, alpha: 0.45 },
      { width: 2.4, alpha: 0.45 },
      { width: 2.4, alpha: 0.45 },
    ])
  })

  it('animates exactly two subtle particles on a traversed edge into available work', () => {
    installFrameHarness()

    expectParticleMotion(paint(sequenceFixture('resolved', 'frontier'), 16), 1, 0.47)
  })

  it('renders traversed history as a subtle static line unless it points into active work', () => {
    installFrameHarness()

    const render = paint(sequenceFixture('resolved', 'blocked'))
    const history = render.strokes.find(
      (stroke) => stroke.color === 'rgba(150,178,160,0.36)',
    )!
    const arrow = render.fills.find(
      (fill) => fill.color === 'rgba(190,218,198,0.52)',
    )!

    expect(history).toMatchObject({ width: 1.6, dash: [], cap: 'round', alpha: 1 })
    expect(arrow).toBeDefined()
    expect(render.fills.filter((fill) => fill.arcs[0]?.radius === 1.8)).toHaveLength(0)

    for (const destination of [
      { status: 'claimed' as const, assignedToViewer: true },
      { status: 'claimed' as const, assignedToViewer: false },
    ]) {
      const tickets = sequenceFixture('resolved', destination.status).map((ticket) =>
        ticket.num === 38 ? { ...ticket, ...destination } : ticket,
      )
      expect(paint(tickets).fills.filter((fill) => fill.arcs[0]?.radius === 1.8)).toEqual([])
    }
  })

  it('animates exactly two small particles only into doing, my-next, and available work', () => {
    installFrameHarness()

    const variants: Array<{ tickets: Ticket[]; current: number | null }> = [
      { tickets: sequenceFixture('resolved', 'blocked'), current: 38 },
      {
        tickets: sequenceFixture('resolved', 'claimed').map((ticket) =>
          ticket.num === 38
            ? { ...ticket, readyForAgent: true, assignedToViewer: true }
            : ticket,
        ),
        current: null,
      },
      { tickets: sequenceFixture('resolved', 'frontier'), current: null },
    ]

    for (const variant of variants) {
      const particles = paint(variant.tickets, variant.current).fills.filter(
        (fill) => fill.arcs[0]?.radius === 1.8,
      )
      expect(particles).toHaveLength(2)
      for (const particle of particles) {
        expect(particle.color).toMatch(/^rgba\(190,218,198,0\.[0-9]+\)$/)
      }
    }
  })

  it('keeps motion directional, direct, and independent from selection', () => {
    installFrameHarness()
    const chain: Ticket[] = [
      { num: 1, slug: '1', title: 'history', type: 'task', status: 'resolved', blockedBy: [], parentIssue: null, frontier: false },
      { num: 2, slug: '2', title: 'available', type: 'task', status: 'frontier', blockedBy: [1], parentIssue: null, frontier: true, readyForAgent: true },
      { num: 3, slug: '3', title: 'later', type: 'task', status: 'blocked', blockedBy: [2], parentIssue: null, frontier: false },
    ]

    expect(paint(chain).fills.filter((fill) => fill.arcs[0]?.radius === 1.8)).toHaveLength(2)

    const selectedStatic = paint(sequenceFixture('resolved', 'blocked'), null, [38])
    expect(selectedStatic.fills.filter((fill) => fill.arcs[0]?.radius === 1.8)).toHaveLength(0)
    expect(
      selectedStatic.strokes.find(
        (stroke) => stroke.color === 'rgba(150,178,160,0.36)' && stroke.width > 1.6,
      ),
    ).toMatchObject({ width: 2.72, alpha: 1 })
  })

  it('freezes eligible edge particles when reduced motion is requested', () => {
    const clock = installFrameHarness()
    vi.stubGlobal('matchMedia', () => ({
      matches: true,
      addEventListener: () => {},
      removeEventListener: () => {},
    }))
    const [first, second] = paintFrames(sequenceFixture('resolved', 'frontier'), null, [
      () => clock.mockReturnValue(1_000),
      () => clock.mockReturnValue(2_000),
    ])
    const particlePositions = (render: { fills: Fill[] }) =>
      render.fills
        .filter((fill) => fill.arcs[0]?.radius === 1.8)
        .map((fill) => ({ x: fill.arcs[0].x, y: fill.arcs[0].y }))

    expect(particlePositions(first)).toHaveLength(2)
    expect(particlePositions(second)).toEqual(particlePositions(first))
  })

  it('does not animate a resolved sequence source whose destination remains blocked', () => {
    installFrameHarness()

    const render = paint(sequenceFixture('resolved', 'blocked'), 16)
    expect(render.fills.filter((fill) => fill.arcs[0]?.radius === 1.8)).toEqual([])
  })

  it('does not animate an open sequence source whose destination is frontier', () => {
    installFrameHarness()

    const render = paint(sequenceFixture('open', 'frontier'), 16)
    expect(render.fills.filter((fill) => fill.arcs[0]?.radius === 1.8)).toEqual([])
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

    const selectedChild = paint(
      [
        { num: 16, slug: '16', title: 'parent', type: 'issue', status: 'open', blockedBy: [], parentIssue: null, frontier: false },
        { num: 37, slug: '37', title: 'child', type: 'task', status: 'open', blockedBy: [], parentIssue: 16, frontier: false },
      ],
      null,
      [37],
    )
    const selectedMiniStrokes = edgeStrokes(selectedChild)
    expect(selectedMiniStrokes).toHaveLength(2)
    for (const stroke of selectedMiniStrokes) {
      expect(stroke).toMatchObject({
        color: 'rgba(170,145,255,0.78)',
        width: 4.42,
        dash: [8, 7],
        alpha: 1,
      })
    }
    const selectedMiniArrows = selectedChild.fills.filter((fill) => fill.color === '#c7b8ff')
    expect(selectedMiniArrows).toHaveLength(2)
    for (const arrow of selectedMiniArrows) {
      expectArrowDimensions(arrow, 15, 8.125)
    }

    const completed = paintChild('resolved')
    const mintStrokes = completed.strokes.filter((stroke) => stroke.color === 'rgba(150,178,160,0.36)')
    const mintArrows = completed.fills.filter((fill) => fill.color === 'rgba(190,218,198,0.52)')
    expect(mintStrokes).toHaveLength(2)
    expect(mintArrows).toHaveLength(2)
    for (const stroke of mintStrokes) {
      expect(stroke).toMatchObject({ width: 1.6, dash: [], cap: 'round', alpha: 1 })
      expect(stroke.curves).toHaveLength(1)
    }
    expect(completed.fills.filter((fill) => fill.arcs[0]?.radius === 1.8)).toEqual([])
  })
})

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { StarMap } from './starmap'
import type { Ticket } from './model'

type Arc = { radius: number }
type Gradient = { kind: 'radial-gradient'; stops: Array<{ at: number; color: string }> }
type Fill = { style: string | Gradient; arc: Arc | undefined }
type Stroke = { style: string | Gradient; lineWidth: number; arc: Arc | undefined }

const RESOLVED: Ticket = {
  num: 1,
  slug: '1',
  title: 'Resolved core',
  type: 'issue',
  status: 'resolved',
  frontier: false,
  blockedBy: [],
  parentIssue: null,
}
const FRONTIER: Ticket = {
  num: 2,
  slug: '2',
  title: 'Frontier core',
  type: 'issue',
  status: 'open',
  frontier: true,
  blockedBy: [],
  parentIssue: null,
}
const CLAIMED: Ticket = {
  num: 3,
  slug: '3',
  title: 'Claimed core',
  type: 'issue',
  status: 'claimed',
  frontier: false,
  blockedBy: [],
  parentIssue: null,
}
const BLOCKED: Ticket = {
  num: 4,
  slug: '4',
  title: 'Blocked core',
  type: 'issue',
  status: 'open',
  frontier: false,
  blockedBy: [1],
  parentIssue: null,
}
const OUT_OF_SCOPE: Ticket = {
  num: 5,
  slug: '5',
  title: 'Out of scope core',
  type: 'issue',
  status: 'out_of_scope',
  frontier: false,
  blockedBy: [],
  parentIssue: null,
}
const INCOMPLETE_CHILD: Ticket = { ...BLOCKED, num: 6, slug: '6', parentIssue: 99 }
const RESOLVED_CHILD: Ticket = { ...RESOLVED, num: 7, slug: '7', parentIssue: 99 }
const OUT_OF_SCOPE_CHILD: Ticket = { ...OUT_OF_SCOPE, num: 9, slug: '9', parentIssue: 99 }
const READY_CHILD: Ticket = {
  ...FRONTIER,
  num: 8,
  slug: '8',
  parentIssue: 99,
  readyForAgent: true,
}
const MY_NEXT: Ticket = {
  ...CLAIMED,
  num: 10,
  slug: '10',
  title: 'My next work',
  readyForAgent: true,
  assignedToViewer: true,
}

function recordingContext(): {
  ctx: Record<string, unknown>
  fills: Fill[]
  strokes: Stroke[]
} {
  const fills: Fill[] = []
  const strokes: Stroke[] = []
  let arc: Arc | undefined
  const ctx: Record<string, unknown> = {
    createRadialGradient: () => {
      const gradient: Gradient = { kind: 'radial-gradient', stops: [] }
      Object.defineProperty(gradient, 'addColorStop', {
        value: (at: number, color: string) => gradient.stops.push({ at, color }),
      })
      return gradient
    },
    beginPath: () => {
      arc = undefined
    },
    arc: (_x: number, _y: number, radius: number) => {
      arc = { radius }
    },
    fill: () => fills.push({ style: ctx.fillStyle as string | Gradient, arc }),
    stroke: () =>
      strokes.push({
        style: ctx.strokeStyle as string | Gradient,
        lineWidth: ctx.lineWidth as number,
        arc,
      }),
    measureText: () => ({ width: 40 }),
    fillRect: () => {},
    fillText: () => {},
  }
  for (const method of [
    'setTransform',
    'moveTo',
    'lineTo',
    'closePath',
    'quadraticCurveTo',
    'setLineDash',
    'save',
    'restore',
    'translate',
    'scale',
    'rotate',
  ]) {
    ctx[method] = () => {}
  }
  return { ctx, fills, strokes }
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

  function paint(ticket: Ticket, currentIssue: number | null = null): { fills: Fill[]; strokes: Stroke[] } {
    const recording = recordingContext()
    getContext.mockReturnValue(recording.ctx as never)
    const host = document.createElement('div')
    Object.defineProperty(host, 'clientWidth', { value: 1000 })
    Object.defineProperty(host, 'clientHeight', { value: 700 })
    document.body.appendChild(host)
    const map = new StarMap()
    map.mount(host)
    map.setModel([ticket], {}, currentIssue)
    const frame = frames.pop()
    frames = []
    frame?.(0)
    map.destroy()
    return recording
  }

  it('keeps resolved as the existing solid radial-gradient core without a black disk', () => {
    const { fills } = paint(RESOLVED)

    expect(fills).toHaveLength(2)
    expect(fills[1]).toEqual({
      style: {
        kind: 'radial-gradient',
        stops: [
          { at: 0, color: 'rgba(185,214,196,1)' },
          { at: 0.6, color: 'rgba(185,214,196,0.92)' },
          { at: 0.82, color: 'rgba(185,214,196,0.45)' },
          { at: 1, color: 'rgba(185,214,196,0)' },
        ],
      },
      arc: { radius: 9.1125 },
    })
    expect(fills.some((fill) => fill.style === '#000')).toBe(false)
  })

  it('paints My next work as a solid blue core without a black disk', () => {
    const { fills, strokes } = paint(MY_NEXT)

    expect(fills[1]).toEqual({ style: '#8ad8ff', arc: { radius: 11 } })
    expect(fills.some((fill) => fill.style === '#000')).toBe(false)
    expect(strokes.some((stroke) => stroke.style === 'rgba(138,216,255,0.95)')).toBe(false)
  })

  it('paints every incomplete state with its unchanged glow, black disk, and status rim', () => {
    for (const expected of [
      {
        ticket: FRONTIER,
        glow: [
          { at: 0, color: 'rgba(47,155,224,0.765)' },
          { at: 0.4, color: 'rgba(47,155,224,0.198)' },
          { at: 1, color: 'rgba(47,155,224,0)' },
        ],
        blackRadius: 10.125,
        rim: 'rgba(138,216,255,0.95)',
        width: 3.24,
      },
      {
        ticket: CLAIMED,
        glow: [
          { at: 0, color: 'rgba(255,176,32,0.85)' },
          { at: 0.4, color: 'rgba(255,176,32,0.22)' },
          { at: 1, color: 'rgba(255,176,32,0)' },
        ],
        blackRadius: 9,
        rim: 'rgba(255,216,115,0.95)',
        width: 2.88,
      },
      {
        ticket: BLOCKED,
        glow: [
          { at: 0, color: 'rgba(154,111,111,0.85)' },
          { at: 0.4, color: 'rgba(154,111,111,0.22)' },
          { at: 1, color: 'rgba(154,111,111,0)' },
        ],
        blackRadius: 5.625,
        rim: 'rgba(226,195,195,0.95)',
        width: 2.2,
      },
      {
        ticket: OUT_OF_SCOPE,
        glow: [
          { at: 0, color: 'rgba(107,100,120,0.85)' },
          { at: 0.4, color: 'rgba(107,100,120,0.22)' },
          { at: 1, color: 'rgba(107,100,120,0)' },
        ],
        blackRadius: 5.625,
        rim: 'rgba(148,141,164,0.95)',
        width: 2.2,
      },
    ]) {
      const { fills, strokes } = paint(expected.ticket)
      const glow = fills[0].style as Gradient

      expect(fills).toHaveLength(2)
      expect(glow.kind).toBe('radial-gradient')
      expect(glow.stops).toEqual(expected.glow)
      expect(fills[1]).toEqual({ style: '#000', arc: { radius: expected.blackRadius } })
      expect(fills.filter((fill) => fill.style === '#000')).toHaveLength(1)
      expect(fills.filter((fill) => typeof fill.style !== 'string')).toHaveLength(1)
      expect(strokes).toContainEqual({
        style: expected.rim,
        lineWidth: expected.width,
        arc: { radius: expected.blackRadius - expected.width / 2 },
      })
    }
  })

  it('keeps both CURRENT rings around an incomplete issue after its hollow core', () => {
    const { fills, strokes } = paint(BLOCKED, BLOCKED.num)

    expect(fills).toHaveLength(2)
    expect(fills[1]).toEqual({ style: '#000', arc: { radius: 5.625 } })
    expect(strokes.map((stroke) => ({ style: stroke.style, lineWidth: stroke.lineWidth, radius: stroke.arc?.radius }))).toEqual([
      { style: 'rgba(226,195,195,0.95)', lineWidth: 2.2, radius: 4.525 },
      { style: 'rgba(255,255,255,0.95)', lineWidth: 2, radius: 13.625 },
      { style: 'rgba(255,255,255,0.55)', lineWidth: 1, radius: 18.625 },
    ])
  })

  it('adds one relationship rim outside an incomplete child core and none to a resolved child', () => {
    const incomplete = paint(INCOMPLETE_CHILD)
    const relationshipRims = incomplete.strokes.filter((stroke) => stroke.style === 'rgba(170,145,255,0.82)')
    const blackCore = incomplete.fills.find((fill) => fill.style === '#000')!

    expect(relationshipRims).toHaveLength(1)
    expect(relationshipRims[0].arc!.radius).toBeGreaterThan(blackCore.arc!.radius)
    expect(incomplete.strokes).toContainEqual({
      style: 'rgba(226,195,195,0.95)',
      lineWidth: 2.2,
      arc: { radius: 4.525 },
    })
    expect(paint(RESOLVED_CHILD).strokes.some((stroke) => stroke.style === 'rgba(170,145,255,0.82)')).toBe(false)
    expect(paint(OUT_OF_SCOPE_CHILD).strokes.some((stroke) => stroke.style === 'rgba(170,145,255,0.82)')).toBe(false)
  })

  it('keeps READY and CURRENT emphasis strokes dominant over the relationship rim', () => {
    const ready = paint(READY_CHILD)
    const readyStyles = ready.strokes.map((stroke) => stroke.style)
    const readyRelationshipIndex = readyStyles.indexOf('rgba(170,145,255,0.82)')
    const readyRelationshipRadius = ready.strokes[readyRelationshipIndex].arc!.radius
    const readyEmphasisIndexes = ready.strokes
      .map((stroke, index) =>
        stroke.style === 'rgba(138,216,255,0.95)' &&
        (stroke.arc?.radius ?? 0) > readyRelationshipRadius
          ? index
          : -1,
      )
      .filter((index) => index >= 0)
    expect(readyRelationshipIndex).toBeGreaterThanOrEqual(0)
    expect(readyEmphasisIndexes).toHaveLength(1)
    expect(readyEmphasisIndexes[0]).toBeGreaterThan(readyRelationshipIndex)

    const current = paint(INCOMPLETE_CHILD, INCOMPLETE_CHILD.num)
    const currentStyles = current.strokes.map((stroke) => stroke.style)
    const relationshipIndex = currentStyles.indexOf('rgba(170,145,255,0.82)')
    const whiteRingIndexes = currentStyles
      .map((style, index) => typeof style === 'string' && style.startsWith('rgba(255,255,255,') ? index : -1)
      .filter((index) => index >= 0)
    expect(whiteRingIndexes).toHaveLength(2)
    expect(whiteRingIndexes.every((index) => index > relationshipIndex)).toBe(true)
    const relationshipRadius = current.strokes[relationshipIndex].arc!.radius
    for (const index of whiteRingIndexes) {
      expect(current.strokes[index].arc!.radius).toBeGreaterThan(relationshipRadius)
    }
  })
})

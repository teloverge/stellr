// The star-map renderer: one coherent imperative canvas island behind a narrow
// seam — mount, receive the pushed model, emit selection — never decomposed into
// components (ADR 0010). The Svelte chrome hosts it but never reaches inside.
//
// Two guarantees the seam tests pin (spec, Testing Decisions):
//   1. Deterministic layout — the same ticket data always lays out the same way.
//   2. Zero star movement across the lifecycle — a model push changes a star's
//      appearance (its derived status) but never its position. Layout is
//      recomputed only when the map's *structure* (its tickets and edges)
//      changes; a pure status push keeps every position byte-for-byte.
//
// Rendering (the starfield, glow, pulse, easing) is the part that can only be
// judged by eye (starmap-design.md, Open risk), and it degrades gracefully: when
// no 2D context is available (a headless test), the island still ingests the
// model, holds deterministic positions, and emits selection — it simply draws
// nothing. That is what lets the seam be tested without a browser canvas.

// Derived from chartr (https://github.com/rengwu/chartr), MIT, Copyright (c) 2026 John Goh.

import { computeLayout, structureSignature, TAU } from './layout'
import { STAR, LABEL, SESSION_HUE, visualState, hexA, type VisualState } from './theme'
import { GRAMMAR, type SessionState } from './session'
import { analyzeFocus, type Focus } from './focus'
import { edgeKey, isMiniWorkflowEdge, workflowEdges, type WorkflowEdge } from './workflow'
import { writeMiniEdgeCurve, type MutableMiniCurve } from './workflow-geometry'
import {
  reverseEdgeKeys,
  workflowVisualState,
  type WorkflowVisualState,
} from './workflow-visual'
import {
  boxesOverlap,
  clipTitle,
  issueNumberText,
  labelBox,
  orbitLabelFontSize,
  ORBIT_LABEL_REFERENCE_SCALE,
  outwardLabelGeometryAtScale,
  subissueLabelText,
  subissueMarker,
  type LabelAlign,
  type LabelBox,
} from './label-geometry'
import { validOrbitNodeNumbers } from './parent-topology'
import type { Ticket } from './model'

export { clipTitle } from './label-geometry'

export type SelectHandler = (num: number | null) => void

// Fallback only — used until the wrapper's first setBackground() call. Ticket
// 04 moves the real value to the token-derived `--card` colour, resolved by
// StarMap.svelte through tokens.ts and handed in at the seam; the renderer
// itself never reads CSS (ADR 0010).
const DEFAULT_BG = '#05070d'
const ISSUE_RADIUS_SCALE = 1.25
const CONTEXT_ALPHA = 0.3
const CONTEXT_EDGE_ALPHA = 0.45
const SELECTED_EDGE_WIDTH_SCALE = 1.7
const SELECTED_EDGE_ARROW_SCALE = 1.25
const SUBISSUE_RIM = 'rgba(170,145,255,0.82)'
const HIT_DISTANCE_EPSILON = 1e-9

interface RenderEdge extends WorkflowEdge {
  state: WorkflowVisualState
  satisfied: boolean
  reverseExists: boolean
}

interface Node {
  num: number
  title: string
  type: string
  parentIssue: number | null
  vstate: VisualState
  // The session overlay riding this star, or null when no session speaks for it
  // (ticket 13). Strictly additive: it never touches layout or the base star.
  sstate: SessionState | null
  x: number
  y: number
  _x: number
  _y: number
  flare: number
}

// One label, already solved: where it goes and what it says. Held in the cache so
// a resting map re-uses the solve instead of redoing it 60 times a second.
interface LabelDraw {
  text: string
  x: number
  y: number
  align: LabelAlign
  fontSize: number
  fill: string
  alpha: number
}

// A screen-space rectangle. Both stars and already-placed labels become one of
// these, so the solver has a single kind of thing to test against.
type Box = LabelBox

// Who gets first pick of the good slots. A contested slot should go to the star
// the operator is most likely to be reading — the selected one, then the bright
// actionable states — rather than to whichever ticket happens to be numbered
// lowest, which is what array order gave us.
const LABEL_PRIORITY: Record<VisualState, number> = {
  frontier: 0,
  claimed: 1,
  resolved: 2,
  blocked: 3,
  out_of_scope: 4,
}

// A star just off-screen can still own a label that reaches back on-screen, so
// the viewport cull keeps a generous skirt around the canvas.
const CULL_MARGIN = 140

// The hysteresis band on which side of its star a label hangs, counted in
// candidate steps. A label that went above stays above until the below slots
// are clear by this many steps — so the near-tie that flips frame to frame as
// the camera eases has to become a decisive win before the label crosses over.
// Four steps exhaust every same-side candidate before the solver crosses the
// star. Larger labels need that full search: accepting a near slot across the
// star while a farther same-side slot is free can make a label cross and return
// as higher-priority neighbours rearrange during zoom.
const SIDE_HYSTERESIS = 4
// Where a label may sit relative to its star, as a signed side.
const BELOW = 1
const ABOVE = -1
type Side = typeof BELOW | typeof ABOVE

// How much title a label may carry, as the camera pulls back. Zoomed out, stars
// crowd together and the same words need far more room than there is — so the
// title is rationed rather than the label dropped. Below TITLE_MIN_SCALE only
// the number survives; from there the budget ramps to full by TITLE_FULL_SCALE.
//
// The ceiling is set from real ticket titles across this repo's maps, whose
// lengths run median 30 / p90 60 / max 75. 60 spends the width where it buys
// nine titles in ten whole; the long tail past that is a handful of unusually
// wordy tickets, and a label wide enough for them would be pushing 500px —
// half the pane — which costs its neighbours more than it is worth.
//
// The top of the ramp sits well above the scale a map fits at, so the extra
// length arrives as the operator zooms *in* on a region and the stars have
// spread out to make room. Around s≈0.9 the budget still lands near 30, which
// is what the mid-zoom map showed before the ceiling moved.
// The ramp is eased rather than linear. A straight line spends its budget
// evenly across the whole zoom range, which means it is already deep into
// truncating at scales where the map is still visibly open — the label gets
// short well before the room to hold it runs out. The exponent keeps the two
// anchors (a bare number at the bottom, the full title at the top) and lifts
// everything between them, so the title only shortens sharply once the stars
// have genuinely closed up.
const TITLE_MIN_SCALE = ORBIT_LABEL_REFERENCE_SCALE
const TITLE_FULL_SCALE = 1.6
const TITLE_MIN_CHARS = 12
const TITLE_MAX_CHARS = 60
const TITLE_RAMP_EASE = 0.7

// How many characters of title a label may carry at this camera scale. Exported
// for the tests: the ramp is arithmetic, and pinning it end-to-end through the
// camera only ever pins the number-only cliff underneath it.
export function titleBudget(scale: number): number {
  const u = clamp((scale - TITLE_MIN_SCALE) / (TITLE_FULL_SCALE - TITLE_MIN_SCALE), 0, 1)
  return Math.round(
    TITLE_MIN_CHARS + Math.pow(u, TITLE_RAMP_EASE) * (TITLE_MAX_CHARS - TITLE_MIN_CHARS),
  )
}

// How long one ticker line lingers before it fades out, and how long the fade
// takes — the live map's answer to "what just changed under me?" is one glance,
// then calm again.
const TICKER_HOLD = 4.2
const TICKER_FADE = 0.5

/**
 * A camera pose: the world→screen transform the island paints through. Exported
 * because the chrome carries one across a remount — the island is torn down when
 * the operator switches space and rebuilt when they come back, and handing the
 * pose back is what makes that a return rather than a fresh arrival.
 */
export interface Camera {
  x: number
  y: number
  s: number
}

type Cam = Camera

// --- camera feel ------------------------------------------------------------
// Every camera move — a drag, a zoom, seating a star, a refit — sets a goal, and
// the camera eases toward it on an exponential curve with a fixed time constant.
// Nothing snaps and nothing coasts: the map is always travelling to a place
// something asked for, and it arrives exactly there.
//
// The constant is evaluated against real elapsed time, never per frame. A
// per-frame lerp factor would converge twice as fast on a 120Hz panel as on a
// 60Hz one — the map would literally weigh less on a better display.
const CAM_TAU = 0.12
// Close enough to be over: snapping here spares a long tail of sub-pixel frames.
const CAM_EPS = 0.02
const SCALE_EPS = 0.0005

const MIN_SCALE = 0.12
const MAX_SCALE = 3

// A mouse wheel and a trackpad send the same event in very different units: a
// wheel arrives as chunky notches (often counted in lines or pages), a trackpad
// as a fine stream of pixels, and a pinch as that same stream with ctrl flagged
// on. Normalising every one of them to pixels, clamping the outliers, and giving
// the pinch its own gain is what lets one handler serve all three without one of
// them feeling either dead or violent.
const LINE_PX = 16
const WHEEL_GAIN = 0.0016
const PINCH_GAIN = 0.011
const MAX_WHEEL_PX = 140
const MAX_PINCH_PX = 50

// WebKit's non-standard pinch event, which the Mac shell's WKWebView reports
// instead of Chrome's ctrl-flagged wheel. `scale` is cumulative from the start
// of the gesture, not per-event.
interface GestureLike {
  scale: number
  clientX: number
  clientY: number
  preventDefault(): void
}

function mod(a: number, b: number): number {
  return ((a % b) + b) % b
}
function clamp(v: number, a: number, b: number): number {
  return v < a ? a : v > b ? b : v
}

function focusSignature(focus: Focus): string {
  return [
    focus.current ?? '',
    focus.ready.join(','),
    [...focus.pathNodes].join(','),
    [...focus.pathEdges].join(','),
  ].join('|')
}

export class StarMap {
  #host: HTMLElement | null = null
  #canvas: HTMLCanvasElement | null = null
  #ctx: CanvasRenderingContext2D | null = null
  #dpr = 1
  #w = 0
  #h = 0

  #nodes: Node[] = []
  #byNum = new Map<number, Node>()
  #validOrbitNodes = new Set<number>()
  #edges: RenderEdge[] = []
  #miniCurve: MutableMiniCurve = { control: { x: 0, y: 0 }, bow: 0 }
  #miniCurveStart = { x: 0, y: 0 }
  #miniCurveEnd = { x: 0, y: 0 }
  #miniCurveParent = { x: 0, y: 0 }
  #sig = ''
  #resolved = new Set<number>()

  #cam: Cam = { x: 0, y: 0, s: 1 }
  #goal: Cam = { x: 0, y: 0, s: 1 }
  // The detail pane's footprint. The camera fits the constellation into — and
  // seats a selected star at the centre of — the viewport *minus* these insets,
  // so a right- or bottom-docked pane never covers the star being read (ticket
  // 07). The pane reports its measured size, so the camera eases to the space it
  // actually leaves free, in either docking.
  #insets = { top: 16, right: 16, bottom: 16, left: 16 }
  #clock = 0
  #last = 0
  #raf = 0
  // The live WebKit pinch, if one is in flight — it also mutes the wheel path,
  // so a browser that reports a pinch both ways never zooms twice for one gesture.
  // Stamped, so a gesture that never reports its end (interrupted, or swallowed
  // by the window losing the pointer) cannot leave the wheel muted for good.
  #gesture: { scale: number; at: number } | null = null
  #selected: number | null = null
  // One fading ticker line, drawn by the island itself (never a chrome
  // component — ADR 0010): what just changed under you, then calm again.
  #tickerText = ''
  #tickerAt = -1e9

  #focus: Focus = analyzeFocus([], null)
  // The solved label layout, held between frames. Rebuilt only when its inputs
  // change (see #drawLabels); `#labelEpoch` is what a model push bumps to say the
  // text or the colours moved under it.
  #labelCache: { key: string; fs: number; items: LabelDraw[] } | null = null
  #labelEpoch = 0
  // Which side of its star each label last chose, by ticket number. This is the
  // one piece of state the label solve carries across frames, and the only
  // reason it is not a pure function of camera + model. A label that was dropped
  // keeps its remembered side, so a cluster re-opening puts it back where the
  // operator last saw it rather than defaulting it below.
  #labelSide = new Map<number, Side>()
  #bg = DEFAULT_BG
  #onSelect: SelectHandler = () => {}
  #ro: ResizeObserver | null = null
  #detach: (() => void)[] = []

  // --- seam: mount ----------------------------------------------------------
  mount(host: HTMLElement): void {
    this.#host = host
    const canvas = document.createElement('canvas')
    canvas.className = 'starmap-canvas'
    host.appendChild(canvas)
    this.#canvas = canvas
    // getContext returns null in a headless test env; the island runs on without
    // it (model + layout + selection all work, nothing is drawn).
    this.#ctx = canvas.getContext('2d')
    this.#dpr = Math.max(1, (typeof window !== 'undefined' && window.devicePixelRatio) || 1)

    this.#measure()
    if (typeof ResizeObserver !== 'undefined') {
      this.#ro = new ResizeObserver(() => this.#onResize())
      this.#ro.observe(host)
    }
    this.#bindPointer(canvas)
    this.#refit(true)

    if (this.#ctx) {
      this.#last = now()
      this.#raf = requestAnimationFrame(this.#render)
    }
  }

  // --- seam: receive the pushed model --------------------------------------
  // Ingest the map's tickets. When the structure is unchanged, only per-star
  // appearance is refreshed and positions are untouched — the no-movement
  // guarantee. When tickets or edges change, the layout is recomputed and the
  // camera re-fit, which is the only path that ever moves a star.
  // `sessions` is the session overlay keyed by ticket number (ticket 13),
  // derived by the wrapper from the same snapshot. It rides alongside the
  // tickets rather than through a second seam call, so one push is one visual
  // beat: a state change flares its star once and writes one fading ticker line.
  setModel(
    tickets: Ticket[],
    sessions: Record<number, SessionState> = {},
    currentIssue: number | null = null,
  ): void {
    // Titles and statuses can both move under a push, and both feed the label
    // solve — retire the cached one either way.
    this.#labelEpoch++
    const priorFocus = focusSignature(this.#focus)
    this.#focus = analyzeFocus(tickets, currentIssue)
    const focusChanged = priorFocus !== focusSignature(this.#focus)
    const sig = structureSignature(tickets)
    this.#validOrbitNodes = validOrbitNodeNumbers(tickets)
    this.#resolved = new Set(tickets.filter((t) => t.status === 'resolved').map((t) => t.num))

    if (sig === this.#sig && this.#nodes.length) {
      const changed: string[] = []
      for (const t of tickets) {
        const n = this.#byNum.get(t.num)
        if (!n) continue
        n.title = t.title
        n.type = t.type
        const vstate = visualState(t)
        const sstate = sessions[t.num] ?? null
        if (vstate !== n.vstate || sstate !== n.sstate) {
          n.flare = 1
          changed.push(`#${t.num < 10 ? '0' : ''}${t.num} → ${sstate ?? vstate.replace('_', ' ')}`)
        }
        n.vstate = vstate
        n.sstate = sstate
      }
      if (changed.length) this.#tick(changed.join('   ·   '))
      this.#refreshEdges(tickets)
      if (focusChanged) {
        this.#refit(false)
        this.#settleIfHeadless()
      }
      return
    }

    // The stars are about to move, which is the one event that makes a
    // remembered side meaningless — it was chosen against a constellation that
    // no longer exists. Every other epoch bump (a retitle, a resize, the camera
    // easing to a new fit) leaves the stars where they were, and there the
    // memory is exactly what we want kept.
    this.#labelSide.clear()
    this.#sig = sig
    const pts = computeLayout(
      tickets.map(({ num, title, blockedBy, parentIssue }) => ({
        num,
        title,
        blockedBy,
        parentIssue,
      })),
    )
    this.#nodes = tickets.map((t) => {
      const p = pts[t.num]
      return {
        num: t.num,
        title: t.title,
        type: t.type,
        parentIssue: t.parentIssue,
        vstate: visualState(t),
        sstate: sessions[t.num] ?? null,
        x: p.x,
        y: p.y,
        _x: p.x,
        _y: p.y,
        flare: 0,
      }
    })
    this.#byNum = new Map(this.#nodes.map((n) => [n.num, n]))
    this.#refreshEdges(tickets)
    if (this.#selected !== null && !this.#byNum.has(this.#selected)) this.#selected = null
    this.#refit(true)
  }

  #refreshEdges(tickets: Ticket[]): void {
    const byNum = new Map(tickets.map((ticket) => [ticket.num, ticket]))
    const edges = workflowEdges(tickets)
    const reverseEdges = reverseEdgeKeys(edges)
    this.#edges = edges.map((edge) => ({
      ...edge,
      state: workflowVisualState(edge, byNum),
      satisfied: this.#resolved.has(edge.from),
      reverseExists: reverseEdges.has(edgeKey(edge.from, edge.to)),
    }))
  }

  // --- seam: emit selection -------------------------------------------------
  onSelect(cb: SelectHandler): void {
    this.#onSelect = cb
  }

  // Programmatic selection (a deep-link naming a star, or ticket 07's pane): the
  // camera eases the star into the space the pane leaves free.
  select(num: number | null): void {
    if (num !== null && !this.#byNum.has(num)) return
    this.#applySelection(num)
  }

  // Frame the whole constellation in the free area, easing there rather than
  // snapping. Since a closing pane no longer re-fits, this is the operator's way
  // back to the whole map after they have panned and zoomed around in it — the
  // chrome's recentre button, and nothing else in the island calls it.
  fit(): void {
    this.#labelEpoch++
    this.#refit(false)
    this.#settleIfHeadless()
  }

  // The detail pane's measured footprint. Updating it re-eases the camera: a
  // selected star re-seats into the new free rect (so a responsive right→bottom
  // re-dock re-seats it), and with nothing selected a pane *claiming* space eases
  // the map to fit what is left — which is how the map-material pane clears room
  // for itself. A pane only giving space back (closing) moves nothing: the
  // operator's pan and zoom are theirs, and a closed pane is not a reason to
  // throw them away and re-fit the whole constellation.
  setInsets(insets: Partial<{ top: number; right: number; bottom: number; left: number }>): void {
    const next = { ...this.#insets, ...insets }
    // The wrapper re-derives this object on every measurement, so the same four
    // numbers arrive again and again. Only a real change may move the camera —
    // otherwise a pane that merely re-measured would re-seat the star out from
    // under a pose the operator (or a restore) had just set.
    if (
      next.top === this.#insets.top &&
      next.right === this.#insets.right &&
      next.bottom === this.#insets.bottom &&
      next.left === this.#insets.left
    ) {
      return
    }
    // Does the pane want *more* room than it had on some edge? That is the only
    // case an unselected map has to answer, by fitting into what is left.
    const claiming =
      next.top > this.#insets.top ||
      next.right > this.#insets.right ||
      next.bottom > this.#insets.bottom ||
      next.left > this.#insets.left
    this.#insets = next
    if (this.#selected !== null && this.#byNum.has(this.#selected)) {
      this.#seat(this.#selected)
    } else if (claiming) {
      this.#refit(false)
      this.#settleIfHeadless()
    }
  }

  // --- seam: camera pose -----------------------------------------------------
  // Where the camera is *going*, not where it currently is: a pose read mid-ease
  // must name the place the operator sent it, or a map read a frame after a drag
  // would come back somewhere they never chose.
  camera(): Camera {
    return { ...this.#goal }
  }

  // Put a pose back — the chrome's half of surviving a remount. Both the live
  // camera and its goal are set, so the map opens *at* where it was left rather
  // than flying there from the fit. Call it last: a fit, a seat, or an insets
  // change arriving afterwards is a newer instruction and rightly wins.
  restoreCamera(cam: Camera): void {
    if (!isFinite(cam.x) || !isFinite(cam.y) || !(cam.s > 0)) return
    const s = clamp(cam.s, MIN_SCALE, MAX_SCALE)
    this.#goal = { x: cam.x, y: cam.y, s }
    this.#cam = { ...this.#goal }
    // The labels are solved against the camera pose; the cached solve was for
    // the fit this just replaced.
    this.#labelEpoch++
  }

  // --- seam: palette ---------------------------------------------------------
  // The card surface colour, resolved from the shared design tokens by the
  // wrapper and handed in here (ticket 04) — the renderer never reads CSS
  // itself (ADR 0010). A no-op on the next-drawn frame's positions; it only
  // changes what colour the field paints on.
  setBackground(color: string): void {
    this.#bg = color || DEFAULT_BG
  }

  destroy(): void {
    if (this.#raf) cancelAnimationFrame(this.#raf)
    this.#raf = 0
    this.#ro?.disconnect()
    for (const off of this.#detach) off()
    this.#detach = []
    this.#canvas?.remove()
    this.#canvas = null
    this.#ctx = null
    this.#host = null
  }

  // --- test seam ------------------------------------------------------------
  // World-space positions of every star, keyed by ticket number. The seam tests
  // read this to assert both determinism (same data → same positions) and the
  // no-movement guarantee (a status push leaves it identical).
  positions(): Record<number, { x: number; y: number }> {
    const out: Record<number, { x: number; y: number }> = {}
    for (const n of this.#nodes) out[n.num] = { x: n.x, y: n.y }
    return out
  }

  // The session overlay each star is currently carrying, keyed by ticket number
  // (absent for a star no session speaks for). The seam tests read this to assert
  // the grammar each state renders in, alongside GRAMMAR's channels.
  overlays(): Record<number, SessionState> {
    const out: Record<number, SessionState> = {}
    for (const n of this.#nodes) if (n.sstate) out[n.num] = n.sstate
    return out
  }

  // The live ticker line, or null once it has faded — one line per pushed change.
  ticker(): string | null {
    return this.#tickerAlpha() > 0 ? this.#tickerText : null
  }

  // Current screen position of a star under the live camera.
  screenOf(num: number): { x: number; y: number } | null {
    const n = this.#byNum.get(num)
    if (!n) return null
    return { x: n.x * this.#cam.s + this.#cam.x, y: n.y * this.#cam.s + this.#cam.y }
  }

  #radius(n: Node): number {
    return STAR[n.vstate].r * ISSUE_RADIUS_SCALE
  }

  // Hit-test a screen point and, if it lands on a star, select and emit it — the
  // exact path a click drives. Returns the selected ticket number, or null for a
  // click on empty space (which deselects).
  selectAtScreen(sx: number, sy: number): number | null {
    let topLevelHit: Node | null = null
    let nearestTopLevelHit: Node | null = null
    let nearestTopLevelDistance = Number.POSITIVE_INFINITY
    let subissueHit: Node | null = null
    let subissueDistance = Number.POSITIVE_INFINITY
    for (const n of this.#nodes) {
      const px = n._x * this.#cam.s + this.#cam.x
      const py = n._y * this.#cam.s + this.#cam.y
      const baseRadius = Math.max(14, this.#radius(n) * this.#cam.s + 10)
      const hasValidParent = this.#validOrbitNodes.has(n.num)
      const radius = hasValidParent ? Math.max(28, baseRadius + 8) : baseRadius
      const distance = Math.hypot(sx - px, sy - py)
      if (distance >= radius) continue
      if (!hasValidParent) {
        topLevelHit = n
        if (
          distance < nearestTopLevelDistance - HIT_DISTANCE_EPSILON ||
          (Math.abs(distance - nearestTopLevelDistance) <= HIT_DISTANCE_EPSILON &&
            (nearestTopLevelHit === null || n.num < nearestTopLevelHit.num))
        ) {
          nearestTopLevelHit = n
          nearestTopLevelDistance = distance
        }
        continue
      }
      if (
        distance < subissueDistance - HIT_DISTANCE_EPSILON ||
        (Math.abs(distance - subissueDistance) <= HIT_DISTANCE_EPSILON &&
          (subissueHit === null || n.num < subissueHit.num))
      ) {
        subissueHit = n
        subissueDistance = distance
      }
    }
    let hit = topLevelHit
    if (subissueHit && nearestTopLevelHit) {
      hit =
        subissueDistance < nearestTopLevelDistance - HIT_DISTANCE_EPSILON ||
        (Math.abs(subissueDistance - nearestTopLevelDistance) <= HIT_DISTANCE_EPSILON &&
          subissueHit.num < nearestTopLevelHit.num)
          ? subissueHit
          : nearestTopLevelHit
    } else if (subissueHit) {
      hit = subissueHit
    }
    const num = hit ? hit.num : null
    if (hit) hit.flare = Math.max(hit.flare, 0.6)
    this.#applySelection(num)
    return num
  }

  #applySelection(num: number | null): void {
    this.#selected = num
    if (num !== null) this.#seat(num)
    this.#onSelect(num)
  }

  // The viewport minus the pane's insets — the free area the camera works in.
  #freeRect(): { cx: number; cy: number; availW: number; availH: number } {
    const left = this.#insets.left,
      right = this.#w - this.#insets.right,
      top = this.#insets.top,
      bottom = this.#h - this.#insets.bottom
    return {
      cx: (left + right) / 2,
      cy: (top + bottom) / 2,
      availW: Math.max(80, right - left),
      availH: Math.max(80, bottom - top),
    }
  }

  #tick(msg: string): void {
    this.#tickerText = msg
    this.#tickerAt = now()
  }

  #tickerAlpha(): number {
    if (!this.#tickerText) return 0
    const age = now() - this.#tickerAt
    if (age < TICKER_HOLD) return 1
    return clamp(1 - (age - TICKER_HOLD) / TICKER_FADE, 0, 1)
  }

  // Ease a star to the centre of the free rect at the current zoom.
  #seat(num: number): void {
    const n = this.#byNum.get(num)
    if (!n) return
    const { cx, cy } = this.#freeRect()
    // Against the goal scale, not the live one: seating mid-zoom must aim at
    // where the camera is going, or the star lands off-centre when it arrives.
    this.#goal.x = cx - n.x * this.#goal.s
    this.#goal.y = cy - n.y * this.#goal.s
    this.#settleIfHeadless()
  }

  // With a live 2D context the render loop eases the camera toward its goal; with
  // none (a headless test) there is no loop, so the camera settles immediately —
  // which also makes screenOf meaningful for the seam tests.
  #settleIfHeadless(): void {
    if (this.#ctx) return
    this.#cam.x = this.#goal.x
    this.#cam.y = this.#goal.y
    this.#cam.s = this.#goal.s
  }

  // Move the camera by a screen-space delta. The drag moves the *goal*, never the
  // camera: the map trails the cursor while the hand is moving and glides in
  // behind it when the hand stops. Because the goal tracks the pointer one-for-
  // one, the map settles exactly where it was dragged — the lag is time, not
  // distance, and none of it accumulates.
  #pan(dx: number, dy: number): void {
    this.#goal.x += dx
    this.#goal.y += dy
    this.#settleIfHeadless()
  }

  // Zoom by `f` about a screen point, keeping the world under that point pinned.
  #zoomAt(sx: number, sy: number, f: number): void {
    if (!(f > 0) || !isFinite(f)) return
    const ns = clamp(this.#goal.s * f, MIN_SCALE, MAX_SCALE)
    const k = ns / this.#goal.s
    this.#goal.x = sx - (sx - this.#goal.x) * k
    this.#goal.y = sy - (sy - this.#goal.y) * k
    this.#goal.s = ns
    this.#settleIfHeadless()
  }

  // Ease the camera toward its goal over real elapsed time.
  //
  // The ease runs in (focus, world-units-per-pixel) rather than in the camera's
  // own (translate, scale): the world point under any given screen point is
  // *linear* in that pair, so interpolating it holds that point exactly still.
  // Interpolating translate and scale directly does not — the world slides out
  // from under the cursor for the whole flight of a zoom, and pinning the focal
  // point is most of what "zoom feels solid" means.
  #easeCamera(dt: number): void {
    const cam = this.#cam,
      goal = this.#goal
    const near =
      Math.abs(goal.x - cam.x) < CAM_EPS &&
      Math.abs(goal.y - cam.y) < CAM_EPS &&
      Math.abs(goal.s - cam.s) < SCALE_EPS
    if (near) {
      cam.x = goal.x
      cam.y = goal.y
      cam.s = goal.s
      return
    }
    const a = 1 - Math.exp(-dt / CAM_TAU)
    // z is world units per screen pixel; f is the world point at screen origin.
    const z = 1 / cam.s,
      zg = 1 / goal.s
    const fx = -cam.x * z,
      fy = -cam.y * z
    const nz = z + (zg - z) * a
    const nfx = fx + (-goal.x * zg - fx) * a
    const nfy = fy + (-goal.y * zg - fy) * a
    cam.s = 1 / nz
    cam.x = -nfx / nz
    cam.y = -nfy / nz
  }

  // --- sizing + camera ------------------------------------------------------
  #measure(): void {
    const host = this.#host
    const w = host?.clientWidth || (typeof window !== 'undefined' ? window.innerWidth : 800)
    const h = host?.clientHeight || (typeof window !== 'undefined' ? window.innerHeight : 600)
    this.#w = w
    this.#h = h
    if (this.#canvas) {
      this.#canvas.width = Math.max(1, Math.round(w * this.#dpr))
      this.#canvas.height = Math.max(1, Math.round(h * this.#dpr))
      this.#canvas.style.width = w + 'px'
      this.#canvas.style.height = h + 'px'
    }
  }

  // A resize re-frames the viewport; it does not re-plan the camera. So the pose
  // is carried across rather than re-fit: the world point that sat at the centre
  // of the free rect stays at the centre of the new one, at the same zoom. Two
  // reasons it works this way. Re-fitting would silently spend the operator's own
  // pan and zoom every time the seam twitched. And it would make this the second
  // writer aiming #goal during a pane drag, racing setInsets — which eases toward
  // a *seated star* and does not touch scale — through two ResizeObservers and an
  // effect flush, in an order neither controls. One writer, and the seam glides.
  #onResize(): void {
    const w = this.#w,
      h = this.#h
    const before = this.#freeRect()
    this.#measure()
    // A ResizeObserver delivers one callback the moment it starts observing,
    // reporting the size mount already measured. Acting on that would throw away
    // the pose the chrome restored a tick earlier, so only an actual change in
    // the viewport moves anything.
    if (this.#w === w && this.#h === h) return
    const after = this.#freeRect()
    const s = this.#goal.s
    const wx = (before.cx - this.#goal.x) / s
    const wy = (before.cy - this.#goal.y) / s
    this.#goal.x = after.cx - wx * s
    this.#goal.y = after.cy - wy * s
    // Snapped, not eased: a dragged seam moves every frame and outruns any ease,
    // so easing here reads as the map rubber-banding behind the operator's hand.
    this.#cam = { ...this.#goal }
    // The label cache is keyed on the camera pose, which the pin above already
    // moved — so the solve re-runs on its own, and #labelEpoch is left to mean
    // only what it says it means: the text or the colours moved.
    this.#draw()
  }

  // Fit the whole constellation into the viewport. `snap` sets the camera
  // immediately (a fresh map, a resize); otherwise the render loop eases to it.
  #refit(snap: boolean): void {
    if (!this.#nodes.length) return
    const focused = this.#focus.emphasized.size > 0
    const fitNodes = focused
      ? this.#nodes.filter((node) => this.#focus.emphasized.has(node.num))
      : this.#nodes
    if (!fitNodes.length) return
    let minx = 1e9,
      miny = 1e9,
      maxx = -1e9,
      maxy = -1e9
    for (const n of fitNodes) {
      minx = Math.min(minx, n.x)
      miny = Math.min(miny, n.y)
      maxx = Math.max(maxx, n.x)
      maxy = Math.max(maxy, n.y)
    }
    const pad = focused ? 150 : 90
    minx -= pad
    miny -= pad
    maxx += pad
    maxy += pad
    const { cx: fcx, cy: fcy, availW, availH } = this.#freeRect()
    const available = Math.min(availW / (maxx - minx || 1), availH / (maxy - miny || 1))
    const s = focused ? clamp(available * 0.8, 0.15, 1) : clamp(available, 0.15, 1.4)
    const cx = (minx + maxx) / 2,
      cy = (miny + maxy) / 2
    this.#goal.s = s
    this.#goal.x = fcx - cx * s
    this.#goal.y = fcy - cy * s
    if (snap) {
      this.#cam.s = this.#goal.s
      this.#cam.x = this.#goal.x
      this.#cam.y = this.#goal.y
    }
  }

  // --- input: pan, zoom, click ---------------------------------------------
  #bindPointer(canvas: HTMLCanvasElement): void {
    let drag: { x: number; y: number; moved: boolean } | null = null
    const rectXY = (e: { clientX: number; clientY: number }) => {
      const r = canvas.getBoundingClientRect()
      return { x: e.clientX - r.left, y: e.clientY - r.top }
    }
    const onDown = (e: MouseEvent) => {
      this.#gesture = null
      drag = { x: e.clientX, y: e.clientY, moved: false }
      canvas.classList.add('drag')
    }
    const onMove = (e: MouseEvent) => {
      if (!drag) return
      const dx = e.clientX - drag.x,
        dy = e.clientY - drag.y
      if (Math.abs(dx) + Math.abs(dy) > 3) drag.moved = true
      this.#pan(dx, dy)
      drag.x = e.clientX
      drag.y = e.clientY
    }
    const onUp = (e: MouseEvent) => {
      canvas.classList.remove('drag')
      // A press that never moved is a click; the release itself adds nothing —
      // the camera is already on its way to where the drag put it.
      if (drag && !drag.moved) {
        const { x, y } = rectXY(e)
        this.selectAtScreen(x, y)
      }
      drag = null
    }
    // One wheel handler for three devices, all normalised to pixels first: a
    // notched wheel reports lines or pages, a trackpad reports pixels, and a
    // trackpad pinch reports pixels with ctrl flagged on (which is how Chrome and
    // Firefox describe a pinch — WebKit uses the gesture events below instead).
    const pixelsOf = (e: WheelEvent, cap: number) => {
      const unit = e.deltaMode === 1 ? LINE_PX : e.deltaMode === 2 ? this.#h : 1
      return clamp(e.deltaY * unit, -cap, cap)
    }
    const onWheel = (e: WheelEvent) => {
      e.preventDefault()
      // A pinch this browser has already reported through the gesture path — the
      // stamp keeps a gesture that never ended from muting the wheel forever.
      if (this.#gesture && now() - this.#gesture.at < 0.5) return
      this.#gesture = null
      const { x, y } = rectXY(e)
      const pinch = e.ctrlKey
      const px = pixelsOf(e, pinch ? MAX_PINCH_PX : MAX_WHEEL_PX)
      this.#zoomAt(x, y, Math.exp(-px * (pinch ? PINCH_GAIN : WHEEL_GAIN)))
    }
    // WebKit's pinch. preventDefault here does double duty: it keeps the zoom
    // ours, and it stops WKWebView magnifying the whole page out from under the
    // canvas.
    const onGestureStart = (ev: Event) => {
      const e = ev as unknown as GestureLike
      e.preventDefault()
      this.#gesture = { scale: e.scale || 1, at: now() }
    }
    const onGestureChange = (ev: Event) => {
      const e = ev as unknown as GestureLike
      e.preventDefault()
      const g = this.#gesture
      if (!g) return
      const s = e.scale || 1
      const { x, y } = rectXY(e)
      this.#zoomAt(x, y, s / (g.scale || 1))
      g.scale = s
      g.at = now()
    }
    const onGestureEnd = (ev: Event) => {
      ev.preventDefault()
      this.#gesture = null
    }
    canvas.addEventListener('mousedown', onDown)
    window.addEventListener('mousemove', onMove)
    window.addEventListener('mouseup', onUp)
    canvas.addEventListener('wheel', onWheel, { passive: false })
    canvas.addEventListener('gesturestart', onGestureStart as EventListener)
    canvas.addEventListener('gesturechange', onGestureChange as EventListener)
    canvas.addEventListener('gestureend', onGestureEnd as EventListener)
    this.#detach.push(
      () => canvas.removeEventListener('mousedown', onDown),
      () => window.removeEventListener('mousemove', onMove),
      () => window.removeEventListener('mouseup', onUp),
      () => canvas.removeEventListener('wheel', onWheel),
      () => canvas.removeEventListener('gesturestart', onGestureStart as EventListener),
      () => canvas.removeEventListener('gesturechange', onGestureChange as EventListener),
      () => canvas.removeEventListener('gestureend', onGestureEnd as EventListener),
    )
  }

  // --- render loop ----------------------------------------------------------
  // One frame: carry the world forward by the time that actually passed, then
  // paint it. The two halves are split because only this one may move the clock —
  // see #draw.
  #render = (): void => {
    this.#raf = requestAnimationFrame(this.#render)
    if (!this.#ctx) return
    const t = now()
    let dt = t - this.#last
    if (dt < 0 || dt > 0.1) dt = 0.016
    this.#last = t
    this.#clock = t

    for (const n of this.#nodes) {
      const ph = n.num * 1.7
      n._x = n.x + Math.sin(this.#clock * 0.7 + ph) * 2.4
      n._y = n.y + Math.cos(this.#clock * 0.55 + ph) * 2.4
      if (n.flare > 0) n.flare = Math.max(0, n.flare - dt / 1.1)
    }
    this.#easeCamera(dt)
    this.#draw()
  }

  // Paint the state as it currently stands, advancing nothing. Holding no time
  // of its own is what lets anything call this out of band without stealing a
  // frame's worth of wobble or flare decay from the loop — which #onResize does,
  // because resizing the backing store empties it and a ResizeObserver runs
  // after the loop has already painted, so the frame would otherwise reach the
  // compositor blank.
  #draw(): void {
    const g = this.#ctx
    if (!g) return
    g.setTransform(this.#dpr, 0, 0, this.#dpr, 0, 0)
    g.fillStyle = this.#bg
    g.fillRect(0, 0, this.#w, this.#h)

    g.save()
    g.translate(this.#cam.x, this.#cam.y)
    g.scale(this.#cam.s, this.#cam.s)
    const focused = this.#focus.emphasized.size > 0
    for (const edge of this.#edges) {
      if (this.#isSelectedEdge(edge)) continue
      g.save()
      if (focused && !this.#focus.pathEdges.has(edgeKey(edge.from, edge.to))) {
        g.globalAlpha = CONTEXT_EDGE_ALPHA
      }
      this.#drawEdge(g, edge)
      g.restore()
    }
    for (const edge of this.#edges) {
      if (!this.#isSelectedEdge(edge)) continue
      g.save()
      g.globalAlpha = 1
      this.#drawEdge(g, edge, true)
      g.restore()
    }
    for (const n of this.#nodes) {
      g.save()
      if (focused && !this.#focus.emphasized.has(n.num)) g.globalAlpha = CONTEXT_ALPHA
      this.#drawStar(g, n, this.#clock)
      g.restore()
    }
    g.restore()
    this.#drawLabels(g)
    this.#drawTicker(g)
  }

  #isSelectedEdge(edge: RenderEdge): boolean {
    return (
      this.#selected !== null &&
      (edge.from === this.#selected || edge.to === this.#selected)
    )
  }

  #drawEdge(g: CanvasRenderingContext2D, e: RenderEdge, selected = false): void {
    const a = this.#byNum.get(e.from),
      b = this.#byNum.get(e.to)
    if (!a || !b) return
    const ax = a._x,
      ay = a._y,
      bx = b._x,
      by = b._y
    const mx = (ax + bx) / 2,
      my = (ay + by) / 2,
      dx = bx - ax,
      dy = by - ay,
      len = Math.hypot(dx, dy) || 1
    const mini = isMiniWorkflowEdge(e)
    const child = e.child === null ? undefined : this.#byNum.get(e.child)
    const parent =
      child?.parentIssue === null || child?.parentIssue === undefined
        ? undefined
        : this.#byNum.get(child.parentIssue)
    let curve: MutableMiniCurve | null = null
    if (mini) {
      this.#miniCurveStart.x = ax
      this.#miniCurveStart.y = ay
      this.#miniCurveEnd.x = bx
      this.#miniCurveEnd.y = by
      if (parent) {
        this.#miniCurveParent.x = parent._x
        this.#miniCurveParent.y = parent._y
      }
      curve = writeMiniEdgeCurve(
        this.#miniCurve,
        e,
        this.#miniCurveStart,
        this.#miniCurveEnd,
        e.reverseExists,
        parent ? this.#miniCurveParent : undefined,
      )
    }
    const nx = -dy / len,
      ny = dx / len,
      bow = Math.min(46, len * 0.13),
      cx = curve?.control.x ?? mx + nx * bow,
      cy = curve?.control.y ?? my + ny * bow
    g.beginPath()
    g.moveTo(ax, ay)
    g.quadraticCurveTo(cx, cy, bx, by)
    g.lineCap = 'round'
    const usesResolvedStyle = mini ? e.state !== 'incomplete' : e.satisfied
    const strokeScale = selected ? SELECTED_EDGE_WIDTH_SCALE : 1
    if (usesResolvedStyle) {
      g.strokeStyle = 'rgba(190,225,200,0.82)'
      g.lineWidth = 3 * strokeScale
      g.setLineDash([])
    } else if (mini) {
      g.strokeStyle = 'rgba(170,145,255,0.78)'
      g.lineWidth = 2.6 * strokeScale
      g.setLineDash([8, 7])
    } else {
      g.strokeStyle = 'rgba(174,192,218,0.62)'
      g.lineWidth = 2.4 * strokeScale
      g.setLineDash([7, 7])
    }
    g.stroke()
    g.setLineDash([])
    // A satisfied edge (blocker resolved) flows particles blocker→dependent, so
    // the frontier visibly ignites as paths clear (starmap-design.md dec. 5).
    if (mini ? e.state === 'traversed' : e.satisfied) {
      for (let k = 0; k < 3; k++) {
        const u = mod(this.#clock * 0.1 + k / 3 + (e.from * 0.13 + e.to * 0.07), 1),
          m = 1 - u
        const fx = m * m * ax + 2 * m * u * cx + u * u * bx,
          fy = m * m * ay + 2 * m * u * cy + u * u * by
        g.fillStyle = 'rgba(190,225,200,' + (0.14 + 0.18 * Math.sin(u * Math.PI)) + ')'
        g.beginPath()
        g.arc(fx, fy, 5, 0, TAU)
        g.fill()
        g.fillStyle = 'rgba(220,255,230,' + (0.45 + 0.5 * Math.sin(u * Math.PI)) + ')'
        g.beginPath()
        g.arc(fx, fy, 2.6, 0, TAU)
        g.fill()
      }
    }
    // Direction arrowhead at the curve's midpoint.
    const midx = 0.25 * ax + 0.5 * cx + 0.25 * bx,
      midy = 0.25 * ay + 0.5 * cy + 0.25 * by
    const tangentX = (cx - ax) + (bx - cx),
      tangentY = (cy - ay) + (by - cy),
      al = Math.hypot(tangentX, tangentY) || 1,
      ux = tangentX / al,
      uy = tangentY / al
    const arrowScale = selected ? SELECTED_EDGE_ARROW_SCALE : 1
    const ah = 12 * arrowScale,
      aw = 6.5 * arrowScale,
      px = -uy,
      py = ux,
      tipx = midx + ux * ah * 0.5,
      tipy = midy + uy * ah * 0.5
    g.beginPath()
    g.moveTo(tipx, tipy)
    g.lineTo(tipx - ux * ah + px * aw, tipy - uy * ah + py * aw)
    g.lineTo(tipx - ux * ah - px * aw, tipy - uy * ah - py * aw)
    g.closePath()
    g.fillStyle = usesResolvedStyle ? '#d9f3df' : mini ? '#c7b8ff' : '#c8d5e8'
    g.fill()
  }

  #drawStar(g: CanvasRenderingContext2D, n: Node, t: number): void {
    const c = STAR[n.vstate]
    const x = n._x,
      y = n._y,
      fl = n.flare || 0
    const isF = n.vstate === 'frontier',
      isC = n.vstate === 'claimed'
    const beat = 0.5 + 0.5 * Math.sin(t * 2.8)
    const pulse = isF ? 0.8 + 0.2 * beat : 1
    const gr = (isF ? c.gr * (0.92 + 0.16 * beat) : c.gr) * (1 + fl * 0.5)

    const grd = g.createRadialGradient(x, y, 0, x, y, gr)
    grd.addColorStop(0, hexA(c.glow, Math.min(1, 0.85 * pulse + fl * 0.5)))
    grd.addColorStop(0.4, hexA(c.glow, 0.22 * pulse))
    grd.addColorStop(1, hexA(c.glow, 0))
    g.fillStyle = grd
    g.beginPath()
    g.arc(x, y, gr, 0, TAU)
    g.fill()

    const cr = this.#radius(n)
    const hasSubissueRim = n.parentIssue !== null && n.vstate !== 'resolved' && n.vstate !== 'out_of_scope'
    if (hasSubissueRim) {
      g.strokeStyle = SUBISSUE_RIM
      g.lineWidth = 1.5
      g.beginPath()
      g.arc(x, y, cr + 4, 0, TAU)
      g.stroke()
    }
    if (n.vstate === 'resolved') {
      const cg = g.createRadialGradient(x, y, 0, x, y, cr * 1.35)
      cg.addColorStop(0, hexA(c.core, 1))
      cg.addColorStop(0.6, hexA(c.core, 0.92))
      cg.addColorStop(0.82, hexA(c.core, 0.45))
      cg.addColorStop(1, hexA(c.core, 0))
      g.fillStyle = cg
      g.beginPath()
      g.arc(x, y, cr * 1.35, 0, TAU)
      g.fill()
    } else {
      g.fillStyle = '#000'
      g.beginPath()
      g.arc(x, y, cr, 0, TAU)
      g.fill()
      const lineWidth = Math.max(2.2, cr * 0.32)
      g.strokeStyle = hexA(c.core, 0.95)
      g.lineWidth = lineWidth
      g.beginPath()
      g.arc(x, y, cr - lineWidth / 2, 0, TAU)
      g.stroke()
    }

    if (hasSubissueRim && this.#focus.readySet.has(n.num) && this.#focus.current !== n.num) {
      g.strokeStyle = hexA(c.core, 0.95)
      g.lineWidth = 2
      g.beginPath()
      g.arc(x, y, cr + 8, 0, TAU)
      g.stroke()
    }

    if (fl > 0) {
      g.strokeStyle = hexA(c.core, fl * 0.7)
      g.lineWidth = 1.5 + 2 * fl
      g.beginPath()
      g.arc(x, y, cr + (1 - fl) * 40, 0, TAU)
      g.stroke()
    }
    // A live claim breathes with two soft rings. A session overlay speaks for
    // the claim when there is one, so the vanilla claimed rings stand down rather
    // than competing with the moon's orbit.
    if (isC && !n.sstate) {
      g.strokeStyle = hexA(c.core, 0.45 + 0.25 * beat)
      g.lineWidth = 1.5
      g.beginPath()
      g.arc(x, y, cr + 5 + 1.2 * beat, 0, TAU)
      g.stroke()
      g.strokeStyle = hexA(c.core, 0.18 + 0.14 * beat)
      g.lineWidth = 1
      g.beginPath()
      g.arc(x, y, cr + 11 + 1.8 * beat, 0, TAU)
      g.stroke()
    }
    if (n.sstate) this.#drawSession(g, n, x, y, cr, t)
    if (this.#focus.current === n.num) {
      g.strokeStyle = 'rgba(255,255,255,0.95)'
      g.lineWidth = 2
      g.beginPath()
      g.arc(x, y, cr + 8, 0, TAU)
      g.stroke()
      g.strokeStyle = 'rgba(255,255,255,0.55)'
      g.lineWidth = 1
      g.beginPath()
      g.arc(x, y, cr + 13, 0, TAU)
      g.stroke()
    }
    if (this.#selected === n.num) {
      g.strokeStyle = 'rgba(255,255,255,0.85)'
      g.lineWidth = 1.5
      g.beginPath()
      g.arc(x, y, cr + (this.#focus.current === n.num ? 18 : 13), 0, TAU)
      g.stroke()
    }
  }

  // The session overlay (ticket 13), drawn straight from the grammar in
  // session.ts: a session is a body — an amber moon orbiting the star it holds.
  // `motion` is the liveness (orbits / crawls / frozen), `moon` is where the moon
  // sits (circling, or frozen mid-orbit), and `marks` are the shapes that ride
  // along. Because motion and shape carry every state, the
  // overlay still reads with the colour taken away.
  #drawSession(
    g: CanvasRenderingContext2D,
    n: Node,
    x: number,
    y: number,
    r: number,
    t: number,
  ): void {
    const s = n.sstate
    if (!s) return
    const gm = GRAMMAR[s]
    const orbR = r + 10
    const frozen = gm.moon === 'frozen'

    // The orbital apparatus. A dead session greys the whole thing, not just the
    // moon — the orbit itself is defunct — and breaks it into a dashed line.
    if (frozen) {
      g.strokeStyle = hexA(SESSION_HUE.dead, 0.3)
      g.setLineDash([3, 5])
    } else {
      g.strokeStyle = hexA(SESSION_HUE.session, 0.16)
    }
    g.lineWidth = 1
    g.beginPath()
    g.arc(x, y, orbR, 0, TAU)
    g.stroke()
    g.setLineDash([])

    // Where the moon sits on its orbit — sweeping, crawling, or stopped where it
    // died.
    const speed = gm.motion === 'orbit' ? 1.5 : gm.motion === 'crawl' ? 0.18 : 0
    const ang = frozen ? n.num * 1.3 : t * speed + n.num
    const mx = x + Math.cos(ang) * orbR,
      my = y + Math.sin(ang) * orbR

    if (gm.marks.includes('trail')) {
      for (let k = 1; k <= 3; k++) {
        const ta = ang - k * 0.22
        g.fillStyle = hexA(SESSION_HUE.session, 0.3 - k * 0.09)
        g.beginPath()
        g.arc(x + Math.cos(ta) * orbR, y + Math.sin(ta) * orbR, 1.6, 0, TAU)
        g.fill()
      }
    }

    const moonCol = frozen ? SESSION_HUE.dead : SESSION_HUE.session
    // A blocked session blinks: the moon fades in and out as it crawls, so it reads
    // even where the crawl is too slow to see.
    const moonA = gm.marks.includes('blink') ? 0.35 + 0.3 * Math.sin(t * 1.2) : frozen ? 0.9 : 0.95
    g.fillStyle = hexA(moonCol, moonA)
    g.beginPath()
    g.arc(mx, my, frozen ? 2.4 : 2.7, 0, TAU)
    g.fill()

    // A dead moon wears a grey halo — a body stopped, ringed like a marker.
    if (gm.marks.includes('halo')) {
      g.strokeStyle = hexA(SESSION_HUE.dead, 0.7)
      g.lineWidth = 1
      g.beginPath()
      g.arc(mx, my, 4.6, 0, TAU)
      g.stroke()
    }
  }

  // One fading line naming what just changed — drawn by the island, in the
  // island's idiom, so the chrome hosts no state of its own (ADR 0010).
  #drawTicker(g: CanvasRenderingContext2D): void {
    const a = this.#tickerAlpha()
    if (a <= 0) return
    const { cx } = this.#freeRect()
    g.font = '11px ui-monospace,SFMono-Regular,Menlo,monospace'
    g.textAlign = 'center'
    g.shadowColor = 'rgba(0,0,0,0.85)'
    g.shadowBlur = 4
    g.fillStyle = hexA(SESSION_HUE.gold, a)
    g.fillText('▸ ' + this.#tickerText, cx, this.#insets.top + 20)
    g.shadowBlur = 0
  }

  // Labels are anchored to a star's *settled* position (n.x/n.y), never to the
  // drifting render position (n._x/n._y). The drift is only ±2.4 world units, but
  // feeding it into a greedy first-fit solver made the solve unstable: two labels
  // sitting near the overlap threshold would swap slots frame to frame and snap a
  // whole line-height apart. Solving against a position that does not move makes
  // the answer identical every frame, so the stars breathe and the labels hold
  // still.
  //
  // That also makes the solve depend on nothing that moves per frame, so it is
  // cached and only redone when the camera or the model actually changes. The
  // cache is a memo of the last solve, not a lookup table by pose, which is what
  // lets the solve carry one piece of history across frames (`#labelSide`):
  // re-solving the same pose reached a different way may legitimately answer
  // differently, and only the newest answer is ever held.
  #drawLabels(g: CanvasRenderingContext2D): void {
    if (this.#cam.s < 0.22 && this.#focus.emphasized.size === 0) return
    // The viewport belongs in the key alongside the pose: the solve culls to it
    // (see `vis` below), so a resize that happened to leave the camera where it
    // was would otherwise keep placing labels against the old edges.
    const key =
      this.#cam.x.toFixed(2) +
      '|' +
      this.#cam.y.toFixed(2) +
      '|' +
      this.#cam.s.toFixed(4) +
      '|' +
      this.#w +
      'x' +
      this.#h +
      '|' +
      this.#labelEpoch
    let cache = this.#labelCache
    if (!cache || cache.key !== key) {
      cache = this.#solveLabels(g, key)
      this.#labelCache = cache
    }
    g.shadowColor = 'rgba(0,0,0,0.85)'
    g.shadowBlur = 4
    for (const it of cache.items) {
      g.font = it.fontSize.toFixed(1) + 'px ui-sans-serif,system-ui,sans-serif'
      g.globalAlpha = it.alpha
      g.fillStyle = it.fill
      g.textAlign = it.align
      g.fillText(it.text, it.x, it.y)
    }
    g.globalAlpha = 1
    g.shadowBlur = 0
  }

  // Greedy point-feature label placement. Three rules do the work:
  //
  //   1. Stars are obstacles, not just other labels. The old solver only ever
  //      tested a label against labels, so one pushed aside to dodge its
  //      neighbour would happily land on top of a star.
  //   2. A label can climb above the star, not only hang below it. Eight
  //      candidates over four distances on each side, ordered so the side the
  //      label last took is tried first and the other side has to beat it by
  //      SIDE_HYSTERESIS steps to win it over. Placement stays strictly vertical
  //      and always centred on its own star: a sideways slot would buy room in a
  //      cluster, but a label that drifts laterally reads as belonging to
  //      whichever star it drifted toward.
  //   3. A label that fits nowhere is dropped rather than drawn over something.
  //      Which is survivable only because placement runs in priority order, so
  //      what gets dropped is the dimmest star present, never the selected one.
  //
  // Cost is ~16·N² box tests for N visible stars, run once per camera pose (the
  // cache above), which at map scale is microseconds. An easing camera changes
  // pose every frame and so pays that every frame; the hysteresis rides along on
  // it for one map read and one map write per label, which is free by comparison.
  #solveLabels(
    g: CanvasRenderingContext2D,
    key: string,
  ): { key: string; fs: number; items: LabelDraw[] } {
    const numOnly = this.#cam.s < TITLE_MIN_SCALE
    const budget = titleBudget(this.#cam.s)
    const fs = clamp(13 * Math.pow(this.#cam.s, 0.3), 10, 16)
    // measureText reads the context's current font, so set it before measuring.
    g.font = fs.toFixed(1) + 'px ui-sans-serif,system-ui,sans-serif'
    const s = this.#cam.s
    const gap = 4
    const step = fs + 4

    // Everything on screen, with the clearance each star wants kept around it:
    // the core, plus the session overlay's orbit and moon-halo when one is
    // riding it, plus the selection ring. The soft glow (`gr`, up to 49px) is
    // deliberately *not* cleared — it is far too wide to treat as solid, and
    // text over the faint outer falloff reads fine.
    const vis: { n: Node; sx: number; sy: number; rad: number }[] = []
    for (const n of this.#nodes) {
      const sx = n.x * s + this.#cam.x
      const sy = n.y * s + this.#cam.y
      if (sx < -CULL_MARGIN || sx > this.#w + CULL_MARGIN) continue
      if (sy < -CULL_MARGIN || sy > this.#h + CULL_MARGIN) continue
      const core = this.#radius(n)
      let r = core + 2
      if (n.sstate) r = core + 15
      if (
        n.parentIssue !== null &&
        this.#focus.readySet.has(n.num) &&
        this.#focus.current !== n.num
      ) r = Math.max(r, core + 9)
      if (this.#focus.current === n.num) r = Math.max(r, core + 14)
      if (this.#selected === n.num) r = Math.max(r, core + 19)
      vis.push({ n, sx, sy, rad: r * s })
    }

    const starObstacles = new Map<number, Box>(vis.map((v) => [v.n.num, {
      x0: v.sx - v.rad,
      y0: v.sy - v.rad,
      x1: v.sx + v.rad,
      y1: v.sy + v.rad,
    }]))
    const obstacles: Box[] = [...starObstacles.values()]

    const order = [...vis].sort((a, b) => {
      const priority = (n: Node) => {
        if (this.#focus.current === n.num) return 0
        if (this.#focus.readySet.has(n.num)) return 1
        if (this.#selected === n.num) return 2
        if (this.#focus.pathNodes.has(n.num)) return 3
        return 4 + LABEL_PRIORITY[n.vstate]
      }
      const pa = priority(a.n)
      const pb = priority(b.n)
      return pa - pb || a.n.num - b.n.num
    })

    const items: LabelDraw[] = []
    for (const v of order) {
      const parent = this.#validOrbitNodes.has(v.n.num) && v.n.parentIssue !== null
        ? this.#byNum.get(v.n.parentIssue)
        : undefined
      const labelFontSize = parent ? orbitLabelFontSize(s) : fs
      g.font = labelFontSize.toFixed(1) + 'px ui-sans-serif,system-ui,sans-serif'
      const isCurrent = this.#focus.current === v.n.num
      const isReady = this.#focus.readySet.has(v.n.num)
      const marker = subissueMarker(isCurrent, isReady)
      let text = marker + issueNumberText(v.n.num)
      if (!numOnly || marker) text += '  ' + clipTitle(v.n.title, budget)
      const w = g.measureText(text).width

      if (parent) {
        text = marker + subissueLabelText(v.n.num, v.n.title)
        const worldGeometry = outwardLabelGeometryAtScale({
          parent: { x: parent.x, y: parent.y },
          child: { x: v.n.x, y: v.n.y },
          number: v.n.num,
          title: v.n.title,
          scale: s,
          starRadius: v.rad / s,
        })
        const geometry = {
          x: worldGeometry.x * s + this.#cam.x,
          y: worldGeometry.y * s + this.#cam.y,
          align: worldGeometry.align,
          box: {
            x0: worldGeometry.box.x0 * s + this.#cam.x,
            y0: worldGeometry.box.y0 * s + this.#cam.y,
            x1: worldGeometry.box.x1 * s + this.#cam.x,
            y1: worldGeometry.box.y1 * s + this.#cam.y,
          },
        }
        // The outward anchor already clears the visible glyphs from this star.
        // Its padded reservation can graze the square star obstacle on diagonal
        // spokes, which must not make the label disappear. Every other star and
        // every previously placed label remain hard obstacles.
        const ownStar = starObstacles.get(v.n.num)
        if (!obstacles.some((obstacle) =>
          obstacle !== ownStar && boxesOverlap(geometry.box, obstacle),
        )) {
          obstacles.push(geometry.box)
          items.push({
            text,
            x: geometry.x,
            y: geometry.y,
            align: geometry.align,
            fontSize: labelFontSize,
            fill: LABEL[v.n.vstate],
            alpha:
              this.#focus.emphasized.size === 0 || this.#focus.emphasized.has(v.n.num)
                ? 1
                : CONTEXT_ALPHA,
          })
        }
        continue
      }

      // Every candidate is centred on the star and differs only in how far above
      // or below it sits. A label always reads as hanging off its own star, and
      // never drifts sideways toward a neighbour's.
      const slot = (side: Side, k: number) =>
        side === BELOW
          ? v.sy + v.rad + gap + fs * 0.82 + k * step
          : v.sy - v.rad - gap - fs * 0.22 - k * step

      // The remembered side goes first at every distance, and the other side
      // enters SIDE_HYSTERESIS steps late. With the full four-step band the
      // order is kept0..kept3, then other0..other3: crossing the star is allowed
      // only after every slot on the current side is unavailable. A label with
      // no history prefers below, where the solver has always begun.
      const kept = this.#labelSide.get(v.n.num) ?? BELOW
      const other: Side = kept === BELOW ? ABOVE : BELOW
      const cands: { y: number; side: Side }[] = []
      for (let k = 0; k <= 3 + SIDE_HYSTERESIS; k++) {
        if (k <= 3) cands.push({ y: slot(kept, k), side: kept })
        const j = k - SIDE_HYSTERESIS
        if (j >= 0 && j <= 3) cands.push({ y: slot(other, j), side: other })
      }

      for (const c of cands) {
        const box = labelBox(v.sx, c.y, 'center', w, fs)
        let ok = true
        for (const o of obstacles) {
          if (boxesOverlap(box, o)) {
            ok = false
            break
          }
        }
        if (!ok) continue
        obstacles.push(box)
        items.push({
          text,
          x: v.sx,
          y: c.y,
          align: 'center',
          fontSize: labelFontSize,
          fill: LABEL[v.n.vstate],
          alpha:
            this.#focus.emphasized.size === 0 || this.#focus.emphasized.has(v.n.num)
              ? 1
              : CONTEXT_ALPHA,
        })
        this.#labelSide.set(v.n.num, c.side)
        break
      }
      // No candidate cleared: the label is dropped. It comes back as soon as the
      // operator zooms in and the cluster opens up.
    }
    return { key, fs, items }
  }
}

function now(): number {
  return (typeof performance !== 'undefined' ? performance.now() : Date.now()) / 1000
}

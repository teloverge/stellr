import type { Point } from './layout'
import type { WorkflowEdge } from './workflow'
import { curveSide } from './workflow-visual'

export const MINI_EDGE_BOW_CAP = 28

export interface QuadraticCurve {
  start: Point
  control: Point
  end: Point
  bow: number
}

export interface Segment {
  start: Point
  end: Point
}

export interface MiniEdgeCurveInput {
  edge: WorkflowEdge
  start: Point
  end: Point
  reverseExists: boolean
  parent?: Point
}

export function miniEdgeCurve({
  edge,
  start,
  end,
  reverseExists,
  parent,
}: MiniEdgeCurveInput): QuadraticCurve {
  const midpoint = { x: (start.x + end.x) / 2, y: (start.y + end.y) / 2 }
  const edgeLength = Math.hypot(end.x - start.x, end.y - start.y) || 1
  const bow = Math.min(MINI_EDGE_BOW_CAP, edgeLength * 0.18)

  let normal: Point
  if (edge.roles.includes('sequence') && parent) {
    const towardParent = { x: parent.x - midpoint.x, y: parent.y - midpoint.y }
    const length = Math.hypot(towardParent.x, towardParent.y)
    normal =
      length > 0
        ? { x: towardParent.x / length, y: towardParent.y / length }
        : { x: -(end.y - start.y) / edgeLength, y: (end.x - start.x) / edgeLength }
  } else {
    const low = edge.from < edge.to ? start : end
    const high = edge.from < edge.to ? end : start
    const canonical = { x: high.x - low.x, y: high.y - low.y }
    const canonicalLength = Math.hypot(canonical.x, canonical.y) || 1
    const side = curveSide(edge, reverseExists)
    normal = {
      x: (-canonical.y / canonicalLength) * side,
      y: (canonical.x / canonicalLength) * side,
    }
  }

  return {
    start: { ...start },
    control: { x: midpoint.x + normal.x * bow, y: midpoint.y + normal.y * bow },
    end: { ...end },
    bow,
  }
}

export function quadraticPoint(curve: QuadraticCurve, amount: number): Point {
  const t = Math.max(0, Math.min(1, amount))
  const inverse = 1 - t
  return {
    x:
      inverse * inverse * curve.start.x +
      2 * inverse * t * curve.control.x +
      t * t * curve.end.x,
    y:
      inverse * inverse * curve.start.y +
      2 * inverse * t * curve.control.y +
      t * t * curve.end.y,
  }
}

export function sampleQuadratic(curve: QuadraticCurve, segments = 16): Point[] {
  const count = Math.max(1, Math.floor(segments))
  return Array.from({ length: count + 1 }, (_, index) => quadraticPoint(curve, index / count))
}

export function pointToSegmentDistance(point: Point, segment: Segment): number {
  const dx = segment.end.x - segment.start.x
  const dy = segment.end.y - segment.start.y
  const lengthSquared = dx * dx + dy * dy
  if (lengthSquared === 0) return Math.hypot(point.x - segment.start.x, point.y - segment.start.y)
  const projection = Math.max(
    0,
    Math.min(
      1,
      ((point.x - segment.start.x) * dx + (point.y - segment.start.y) * dy) / lengthSquared,
    ),
  )
  return Math.hypot(
    point.x - (segment.start.x + projection * dx),
    point.y - (segment.start.y + projection * dy),
  )
}

export function curveToSegmentClearance(curve: QuadraticCurve, segment: Segment): number {
  return Math.min(
    ...sampleQuadratic(curve).map((point) => pointToSegmentDistance(point, segment)),
  )
}

function orientation(a: Point, b: Point, c: Point): number {
  return (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
}

function segmentsCross(left: Segment, right: Segment): boolean {
  const abC = orientation(left.start, left.end, right.start)
  const abD = orientation(left.start, left.end, right.end)
  const cdA = orientation(right.start, right.end, left.start)
  const cdB = orientation(right.start, right.end, left.end)
  return abC * abD < 0 && cdA * cdB < 0
}

export function curveCrossesSegment(curve: QuadraticCurve, segment: Segment): boolean {
  const points = sampleQuadratic(curve)
  for (let index = 1; index < points.length; index++) {
    if (segmentsCross({ start: points[index - 1], end: points[index] }, segment)) return true
  }
  return false
}

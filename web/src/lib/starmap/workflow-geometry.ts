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

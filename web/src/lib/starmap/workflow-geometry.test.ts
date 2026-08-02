import { describe, expect, it } from 'vitest'
import {
  miniEdgeCurve,
  MINI_EDGE_BOW_CAP,
  writeMiniEdgeCurve,
  type MutableMiniCurve,
} from './workflow-geometry'
import type { WorkflowEdge } from './workflow'

const entry: WorkflowEdge = { from: 16, to: 37, roles: ['entry'], child: 37 }
const returned: WorkflowEdge = { from: 37, to: 16, roles: ['return'], child: 37 }

describe('mini-edge geometry', () => {
  it('caps the bow and puts reverse entry and return curves on opposite sides', () => {
    const points = { start: { x: 0, y: 0 }, end: { x: 200, y: 0 } }
    const outward = miniEdgeCurve({ edge: entry, ...points, reverseExists: true })
    const inward = miniEdgeCurve({
      edge: returned,
      start: points.end,
      end: points.start,
      reverseExists: true,
    })

    expect(outward.bow).toBe(MINI_EDGE_BOW_CAP)
    expect(inward.bow).toBe(MINI_EDGE_BOW_CAP)
    expect(outward.control).toEqual({ x: 100, y: 28 })
    expect(inward.control).toEqual({ x: 100, y: -28 })
  })

  it('bends a sibling sequence toward the parent-side interior', () => {
    const sequence: WorkflowEdge = { from: 37, to: 38, roles: ['dependency', 'sequence'], child: 38 }
    const curve = miniEdgeCurve({
      edge: sequence,
      start: { x: -40, y: 90 },
      end: { x: 40, y: 90 },
      parent: { x: 0, y: 0 },
      reverseExists: false,
    })

    expect(curve.control.x).toBeCloseTo(0)
    expect(curve.control.y).toBeLessThan(90)
    expect(curve.bow).toBeCloseTo(14.4)
  })

  it('writes renderer geometry into reusable storage without allocating a result', () => {
    const scratch: MutableMiniCurve = { control: { x: 0, y: 0 }, bow: 0 }

    const result = writeMiniEdgeCurve(scratch, entry, 0, 0, 200, 0, true)

    expect(result).toBe(scratch)
    expect(scratch).toEqual({ control: { x: 100, y: 28 }, bow: 28 })
  })
})

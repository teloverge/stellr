import { describe, expect, it } from 'vitest'
import { computeLayout, rankOf, structureSignature, type LayoutNode } from './layout'

const workflowCycle: LayoutNode[] = [
  { num: 16, blockedBy: [], parentIssue: 38 },
  { num: 37, blockedBy: [], parentIssue: 16 },
  { num: 38, blockedBy: [37], parentIssue: 37 },
]

describe('workflow-aware layout', () => {
  it('keeps dependency rank blocker-only through a workflow cycle', () => {
    expect(rankOf(workflowCycle)).toEqual({ 16: 0, 37: 0, 38: 1 })
  })

  it('uses parent topology for clustering and structural identity, not dependency rank or status', () => {
    const withoutParent: LayoutNode[] = [
      { num: 16, blockedBy: [], parentIssue: null },
      { num: 37, blockedBy: [16], parentIssue: null },
    ]
    const withParent: LayoutNode[] = [
      { num: 16, blockedBy: [], parentIssue: null },
      { num: 37, blockedBy: [16], parentIssue: 16 },
    ]
    const statuses = withParent.map((node) => ({ ...node, status: 'resolved' }))

    expect(computeLayout(withParent)).not.toEqual(computeLayout(withoutParent))
    expect(structureSignature(withParent)).not.toBe(structureSignature(withoutParent))
    expect(structureSignature(statuses)).toBe(structureSignature(withParent))
  })

  it('places direct children on a compact parent-local arc without moving broad anchors', () => {
    const broad: LayoutNode[] = [
      { num: 16, blockedBy: [], parentIssue: null },
      { num: 34, blockedBy: [], parentIssue: null },
      { num: 35, blockedBy: [], parentIssue: null },
      { num: 99, blockedBy: [], parentIssue: null },
    ]
    const clustered = broad.map((node) =>
      node.num === 34 || node.num === 35 ? { ...node, parentIssue: 16 } : node,
    )

    const broadPoints = computeLayout(broad)
    const clusterPoints = computeLayout(clustered)

    expect(clusterPoints[16]).toEqual(broadPoints[16])
    expect(clusterPoints[99]).toEqual(broadPoints[99])
    expect(
      Math.hypot(
        clusterPoints[34].x - clusterPoints[16].x,
        clusterPoints[34].y - clusterPoints[16].y,
      ),
    ).toBeCloseTo(92, 6)
    expect(
      Math.hypot(
        clusterPoints[35].x - clusterPoints[16].x,
        clusterPoints[35].y - clusterPoints[16].y,
      ),
    ).toBeCloseTo(92, 6)
    expect(
      Math.hypot(
        clusterPoints[35].x - clusterPoints[34].x,
        clusterPoints[35].y - clusterPoints[34].y,
      ),
    ).toBeCloseTo(47.62, 2)
  })

  it('orders a sibling blocker sequence around the arc independently of snapshot order', () => {
    const sequence: LayoutNode[] = [
      { num: 16, blockedBy: [], parentIssue: null },
      { num: 50, blockedBy: [], parentIssue: 16 },
      { num: 10, blockedBy: [50], parentIssue: 16 },
      { num: 40, blockedBy: [10], parentIssue: 16 },
    ]

    const points = computeLayout(sequence)
    const reversed = computeLayout([...sequence].reverse())
    const vector = (number: number) => ({
      x: points[number].x - points[16].x,
      y: points[number].y - points[16].y,
    })
    const cross = (left: number, right: number) => {
      const a = vector(left)
      const b = vector(right)
      return a.x * b.y - a.y * b.x
    }

    expect(reversed).toEqual(points)
    expect(cross(50, 10)).toBeGreaterThan(0)
    expect(cross(10, 40)).toBeGreaterThan(0)
  })

  it('fits five direct children on the compact first arc with the approved clearance', () => {
    const nodes: LayoutNode[] = [
      { num: 16, blockedBy: [], parentIssue: null },
      ...[31, 32, 33, 34, 35].map((num) => ({ num, blockedBy: [], parentIssue: 16 })),
    ]
    const points = computeLayout(nodes)

    for (const number of [31, 32, 33, 34, 35]) {
      expect(
        Math.hypot(points[number].x - points[16].x, points[number].y - points[16].y),
      ).toBeCloseTo(92, 6)
    }
    for (const [left, right] of [[31, 32], [32, 33], [33, 34], [34, 35]]) {
      expect(
        Math.hypot(points[left].x - points[right].x, points[left].y - points[right].y),
      ).toBeGreaterThanOrEqual(44)
    }
  })
})

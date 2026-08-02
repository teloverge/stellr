import { describe, expect, it } from 'vitest'
import { edgesOf, type LayoutNode, type Point } from './layout'
import { placeDirectChildClusters } from './cluster-layout'

describe('compact cluster sector selection', () => {
  it('moves a cluster away from an unrelated dependency through its default sector', () => {
    const nodes: LayoutNode[] = [
      { num: 16, blockedBy: [], parentIssue: null },
      { num: 34, blockedBy: [], parentIssue: 16 },
      { num: 35, blockedBy: [], parentIssue: 16 },
      { num: 90, blockedBy: [], parentIssue: null },
      { num: 91, blockedBy: [90], parentIssue: null },
    ]
    const broadPoints: Record<number, Point> = {
      16: { x: 0, y: 0 },
      34: { x: -300, y: -300 },
      35: { x: 300, y: 300 },
      90: { x: 80, y: -200 },
      91: { x: 80, y: 200 },
    }

    const points = placeDirectChildClusters(nodes, broadPoints, edgesOf(nodes))
    const centerX = (points[34].x + points[35].x) / 2

    expect(centerX).toBeLessThan(0)
    expect(Math.abs(points[34].x - 80)).toBeGreaterThanOrEqual(18)
    expect(Math.abs(points[35].x - 80)).toBeGreaterThanOrEqual(18)
  })

  it('redirects around a nearby unrelated node and ignores snapshot input order', () => {
    const nodes: LayoutNode[] = [
      { num: 16, blockedBy: [], parentIssue: null },
      { num: 34, blockedBy: [], parentIssue: 16 },
      { num: 35, blockedBy: [], parentIssue: 16 },
      { num: 99, blockedBy: [], parentIssue: null },
    ]
    const broadPoints: Record<number, Point> = {
      16: { x: 0, y: 0 },
      34: { x: -300, y: -300 },
      35: { x: 300, y: 300 },
      99: { x: 92, y: 0 },
    }

    const forward = placeDirectChildClusters(nodes, broadPoints, [])
    const reversed = placeDirectChildClusters([...nodes].reverse(), broadPoints, [])

    expect(reversed).toEqual(forward)
    expect((forward[34].x + forward[35].x) / 2).toBeLessThan(0)
    expect(Math.hypot(forward[34].x - 92, forward[34].y)).toBeGreaterThanOrEqual(42)
    expect(Math.hypot(forward[35].x - 92, forward[35].y)).toBeGreaterThanOrEqual(42)
  })

  it('uses the same finite fallback repeatedly when every sector is obstructed', () => {
    const children: LayoutNode[] = [
      { num: 34, blockedBy: [], parentIssue: 16 },
      { num: 35, blockedBy: [], parentIssue: 16 },
    ]
    const obstacles: LayoutNode[] = Array.from({ length: 16 }, (_, index) => ({
      num: 100 + index,
      blockedBy: [],
      parentIssue: null,
    }))
    const nodes: LayoutNode[] = [
      { num: 16, blockedBy: [], parentIssue: null },
      ...children,
      ...obstacles,
    ]
    const broadPoints: Record<number, Point> = {
      16: { x: 0, y: 0 },
      34: { x: -300, y: -300 },
      35: { x: 300, y: 300 },
    }
    for (let index = 0; index < obstacles.length; index++) {
      const angle = (index / obstacles.length) * Math.PI * 2
      broadPoints[obstacles[index].num] = { x: Math.cos(angle) * 92, y: Math.sin(angle) * 92 }
    }

    const first = placeDirectChildClusters(nodes, broadPoints, [])
    const second = placeDirectChildClusters([...nodes].reverse(), broadPoints, [])

    expect(second).toEqual(first)
    expect(Number.isFinite(first[34].x)).toBe(true)
    expect(Number.isFinite(first[34].y)).toBe(true)
    expect(Number.isFinite(first[35].x)).toBe(true)
    expect(Number.isFinite(first[35].y)).toBe(true)
  })
})

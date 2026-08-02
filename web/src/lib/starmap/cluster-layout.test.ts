import { describe, expect, it } from 'vitest'
import { edgesOf, type LayoutNode, type Point } from './layout'
import { placeDirectChildClusters } from './cluster-layout'
import { workflowEdges } from './workflow'
import { reverseEdgeKeys } from './workflow-visual'
import {
  curveToSegmentClearance,
  miniEdgeCurve,
  sampleQuadratic,
} from './workflow-geometry'

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

  it('places six children on bounded first and second arcs with minimum clearance', () => {
    const children = [31, 32, 33, 34, 35, 36]
    const nodes: LayoutNode[] = [
      { num: 16, blockedBy: [], parentIssue: null },
      ...children.map((num) => ({ num, blockedBy: [], parentIssue: 16 })),
    ]
    const broadPoints: Record<number, Point> = { 16: { x: 0, y: 0 } }
    for (const [index, number] of children.entries()) {
      broadPoints[number] = { x: 400 + index * 100, y: 400 }
    }

    const points = placeDirectChildClusters(nodes, broadPoints, [])
    const radii = children.map((number) => Math.hypot(points[number].x, points[number].y))

    expect(radii.filter((radius) => Math.abs(radius - 92) < 0.001)).toHaveLength(4)
    expect(radii.filter((radius) => Math.abs(radius - 126) < 0.001)).toHaveLength(2)
    for (let left = 0; left < children.length; left++) {
      for (let right = left + 1; right < children.length; right++) {
        expect(
          Math.hypot(
            points[children[left]].x - points[children[right]].x,
            points[children[left]].y - points[children[right]].y,
          ),
        ).toBeGreaterThanOrEqual(44)
      }
    }
  })

  it('places nested groups parent-first around each immediate parent final position', () => {
    const nodes: LayoutNode[] = [
      { num: 50, blockedBy: [], parentIssue: null },
      { num: 10, blockedBy: [], parentIssue: 50 },
      { num: 5, blockedBy: [], parentIssue: 10 },
    ]
    const broadPoints: Record<number, Point> = {
      50: { x: 0, y: 0 },
      10: { x: 500, y: 500 },
      5: { x: -500, y: -500 },
    }

    const points = placeDirectChildClusters(nodes, broadPoints, [])

    expect(
      Math.hypot(points[10].x - points[50].x, points[10].y - points[50].y),
    ).toBeCloseTo(92, 6)
    expect(
      Math.hypot(points[5].x - points[10].x, points[5].y - points[10].y),
    ).toBeCloseTo(92, 6)
  })

  it('retains broad coordinates for invalid hierarchy and numeric geometry without blocking valid groups', () => {
    const nodes: LayoutNode[] = [
      { num: 1, blockedBy: [], parentIssue: null },
      { num: 2, blockedBy: [], parentIssue: 1 },
      { num: 3, blockedBy: [], parentIssue: 1 },
      { num: 4, blockedBy: [], parentIssue: 4 },
      { num: 5, blockedBy: [], parentIssue: 99 },
      { num: 6, blockedBy: [], parentIssue: 7 },
      { num: 7, blockedBy: [], parentIssue: 6 },
    ]
    const broadPoints: Record<number, Point> = {
      1: { x: 0, y: 0 },
      2: { x: Number.NaN, y: 50 },
      3: { x: 300, y: 300 },
      4: { x: 40, y: 40 },
      5: { x: 50, y: 50 },
      6: { x: 60, y: 60 },
      7: { x: 70, y: 70 },
    }

    const points = placeDirectChildClusters(nodes, broadPoints, [])

    expect(Number.isNaN(points[2].x)).toBe(true)
    expect(
      Math.hypot(points[3].x - points[1].x, points[3].y - points[1].y),
    ).toBeCloseTo(92, 6)
    expect(points[4]).toEqual(broadPoints[4])
    expect(points[5]).toEqual(broadPoints[5])
    expect(points[6]).toEqual(broadPoints[6])
    expect(points[7]).toEqual(broadPoints[7])
  })

  it('keeps nested mini-curves clear of nonincident ancestor mini-curves', () => {
    const nodes: LayoutNode[] = [
      { num: 50, blockedBy: [], parentIssue: null },
      { num: 10, blockedBy: [], parentIssue: 50 },
      { num: 20, blockedBy: [], parentIssue: 50 },
      { num: 5, blockedBy: [], parentIssue: 10 },
    ]
    const broadPoints: Record<number, Point> = {
      50: { x: 0, y: 0 },
      10: { x: 500, y: 500 },
      20: { x: -500, y: 500 },
      5: { x: -500, y: -500 },
    }
    const points = placeDirectChildClusters(nodes, broadPoints, [])
    const edges = workflowEdges(nodes)
    const reversed = reverseEdgeKeys(edges)
    const byNumber = new Map(nodes.map((node) => [node.num, node]))
    const curveFor = (edge: (typeof edges)[number]) => {
      const child = edge.child === null ? undefined : byNumber.get(edge.child)
      const parent =
        child?.parentIssue === null || child?.parentIssue === undefined
          ? undefined
          : points[child.parentIssue]
      return miniEdgeCurve({
        edge,
        start: points[edge.from],
        end: points[edge.to],
        reverseExists: reversed.has(`${edge.from}>${edge.to}`),
        parent,
      })
    }
    const ancestorCurves = edges
      .filter(
        (edge) =>
          [edge.from, edge.to].includes(20) && [edge.from, edge.to].includes(50),
      )
      .map(curveFor)
    const nestedCurves = edges
      .filter(
        (edge) =>
          [edge.from, edge.to].includes(5) && [edge.from, edge.to].includes(10),
      )
      .map(curveFor)

    for (const nested of nestedCurves) {
      for (const ancestor of ancestorCurves) {
        const samples = sampleQuadratic(ancestor)
        const clearance = Math.min(
          ...samples.slice(1).map((end, index) =>
            curveToSegmentClearance(nested, { start: samples[index], end }),
          ),
        )
        expect(clearance).toBeGreaterThanOrEqual(18)
      }
    }
  })

  it('places the acyclic sibling prefix before a deterministic cycle fallback', () => {
    const nodes: LayoutNode[] = [
      { num: 16, blockedBy: [], parentIssue: null },
      { num: 8, blockedBy: [7], parentIssue: 16 },
      { num: 5, blockedBy: [], parentIssue: 16 },
      { num: 7, blockedBy: [8], parentIssue: 16 },
    ]
    const broadPoints: Record<number, Point> = {
      16: { x: 0, y: 0 },
      5: { x: 500, y: 500 },
      7: { x: -500, y: 500 },
      8: { x: -500, y: -500 },
    }
    const points = placeDirectChildClusters(nodes, broadPoints, [])
    const reversed = placeDirectChildClusters([...nodes].reverse(), broadPoints, [])
    const vector = (number: number) => points[number]
    const cross = (left: number, right: number) =>
      vector(left).x * vector(right).y - vector(left).y * vector(right).x

    expect(reversed).toEqual(points)
    expect(cross(5, 7)).toBeGreaterThan(0)
    expect(cross(7, 8)).toBeGreaterThan(0)
  })

  it('keeps every child in a large group within the bounded cluster', () => {
    const children = Array.from({ length: 14 }, (_, index) => 31 + index)
    const nodes: LayoutNode[] = [
      { num: 16, blockedBy: [], parentIssue: null },
      ...children.map((num) => ({ num, blockedBy: [], parentIssue: 16 })),
    ]
    const broadPoints: Record<number, Point> = { 16: { x: 0, y: 0 } }
    for (const [index, number] of children.entries()) {
      broadPoints[number] = { x: 1_000 + index * 100, y: 1_000 }
    }

    const points = placeDirectChildClusters(nodes, broadPoints, [])

    for (const number of children) {
      expect(Math.hypot(points[number].x, points[number].y)).toBeLessThanOrEqual(126.001)
    }
  })
})

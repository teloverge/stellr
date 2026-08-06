import { describe, expect, it } from 'vitest'
import { edgesOf, type LayoutNode, type Point } from './layout'
import {
  INNER_RING_RADIUS,
  MIN_CHILD_CENTER_CLEARANCE,
  compareCandidateScores,
  orbitRingCounts,
  placeDirectChildClusters,
} from './cluster-layout'
import { workflowEdges } from './workflow'
import { reverseEdgeKeys } from './workflow-visual'
import {
  curveToSegmentClearance,
  miniEdgeCurve,
  sampleQuadratic,
} from './workflow-geometry'
import { boxesOverlap, estimateLabelWidth, outwardLabelGeometry } from './label-geometry'

describe('adaptive orbit cluster selection', () => {
  it('distributes a small sibling group around a complete parent-centred orbit', () => {
    const children = [31, 32, 33, 34]
    const nodes: LayoutNode[] = [
      { num: 16, blockedBy: [], parentIssue: null },
      ...children.map((num) => ({ num, blockedBy: [], parentIssue: 16 })),
    ]
    const broadPoints: Record<number, Point> = {
      16: { x: 25, y: -40 },
      31: { x: -400, y: -400 },
      32: { x: 400, y: -400 },
      33: { x: 400, y: 400 },
      34: { x: -400, y: 400 },
    }

    const points = placeDirectChildClusters(nodes, broadPoints, [])
    const vectors = children.map((number) => ({
      x: points[number].x - points[16].x,
      y: points[number].y - points[16].y,
    }))
    const centroid = vectors.reduce(
      (sum, point) => ({ x: sum.x + point.x, y: sum.y + point.y }),
      { x: 0, y: 0 },
    )

    expect(centroid.x / children.length).toBeCloseTo(0, 6)
    expect(centroid.y / children.length).toBeCloseTo(0, 6)
    expect(new Set(vectors.map((point) => Math.hypot(point.x, point.y).toFixed(6))).size).toBe(1)
  })

  it('expands a dense labelled sibling group across clear concentric rings', () => {
    const children = Array.from({ length: 14 }, (_, index) => 31 + index)
    const title = (number: number) => `Subissue ${number} with a descriptive operator-facing title`
    const nodes = [
      { num: 16, title: 'Parent', blockedBy: [], parentIssue: null },
      ...children.map((num) => ({ num, title: title(num), blockedBy: [], parentIssue: 16 })),
    ]
    const broadPoints: Record<number, Point> = { 16: { x: 0, y: 0 } }
    for (const [index, number] of children.entries()) {
      broadPoints[number] = { x: 1_000 + index * 100, y: 1_000 }
    }

    const points = placeDirectChildClusters(nodes, broadPoints, [])
    const reversed = placeDirectChildClusters([...nodes].reverse(), broadPoints, [])
    const radii = children.map((number) =>
      Math.hypot(points[number].x - points[16].x, points[number].y - points[16].y),
    )
    const boxes = children.map((number) =>
      outwardLabelGeometry({
        parent: points[16],
        child: points[number],
        textWidth: estimateLabelWidth(number, title(number)),
        fontSize: 14,
        starRadius: 14,
      }).box,
    )

    expect(reversed).toEqual(points)
    expect(new Set(radii.map((radius) => radius.toFixed(3))).size).toBeGreaterThan(1)
    for (let left = 0; left < boxes.length; left++) {
      for (let right = left + 1; right < boxes.length; right++) {
        const a = boxes[left]
        const b = boxes[right]
        expect(
          boxesOverlap(a, b),
          `labels ${children[left]} and ${children[right]} overlap`,
        ).toBe(false)
      }
    }
  })

  it('derives the inner orbit radius from the occupied label footprint', () => {
    const children = Array.from({ length: 7 }, (_, index) => 31 + index)
    const nodes = (title: string): LayoutNode[] => [
      { num: 16, title: 'Parent', blockedBy: [], parentIssue: null },
      ...children.map((num) => ({ num, title, blockedBy: [], parentIssue: 16 })),
    ]
    const broadPoints: Record<number, Point> = { 16: { x: 0, y: 0 } }
    for (const [index, number] of children.entries()) {
      broadPoints[number] = { x: 1_000 + index * 100, y: 1_000 }
    }

    const short = placeDirectChildClusters(nodes(''), broadPoints, [])
    const long = placeDirectChildClusters(nodes('A descriptive title that consumes the full label budget'), broadPoints, [])
    const radius = (points: Record<number, Point>) =>
      Math.hypot(points[31].x - points[16].x, points[31].y - points[16].y)

    expect(radius(short)).toBeCloseTo(INNER_RING_RADIUS, 6)
    expect(radius(long)).toBeGreaterThan(radius(short))
  })

  it('allocates rings from each child label footprint', () => {
    const children = Array.from({ length: 8 }, (_, index) => index + 31)
    const nodes: LayoutNode[] = children.map((num, index) => ({
      num,
      title: index === children.length - 1
        ? 'A descriptive title that consumes the full label budget'
        : '',
      blockedBy: [],
      parentIssue: 16,
    }))

    expect(orbitRingCounts(nodes)).toEqual([7, 1])
  })

  it('keeps a later orbit clear of labels already placed by an unrelated family', () => {
    const leftChildren = [31, 32, 33, 34]
    const rightChildren = [41, 42, 43, 44]
    const title = (number: number) => `Subissue ${number} with a descriptive operator-facing title`
    const nodes: LayoutNode[] = [
      { num: 10, title: 'Left parent', blockedBy: [], parentIssue: null },
      ...leftChildren.map((num) => ({ num, title: title(num), blockedBy: [], parentIssue: 10 })),
      { num: 20, title: 'Right parent', blockedBy: [], parentIssue: null },
      ...rightChildren.map((num) => ({ num, title: title(num), blockedBy: [], parentIssue: 20 })),
    ]
    const broadPoints: Record<number, Point> = {
      10: { x: -240, y: 0 },
      20: { x: 240, y: 0 },
    }
    for (const [index, number] of [...leftChildren, ...rightChildren].entries()) {
      broadPoints[number] = { x: 1_000 + index * 100, y: 1_000 }
    }

    const points = placeDirectChildClusters(nodes, broadPoints, [])
    const labelBox = (number: number, parent: number) =>
      outwardLabelGeometry({
        parent: points[parent],
        child: points[number],
        textWidth: estimateLabelWidth(number, title(number)),
        fontSize: 14,
        starRadius: 14,
      }).box

    for (const left of leftChildren) {
      for (const right of rightChildren) {
        expect(
          boxesOverlap(labelBox(left, 10), labelBox(right, 20)),
          `labels ${left} and ${right} overlap`,
        ).toBe(false)
      }
    }
  })

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

  it('gives crossing penalties precedence over available clearance', () => {
    expect(
      compareCandidateScores(
        { crossings: 0, nodeClearance: 42, dependencyClearance: 18 },
        {
          crossings: 1,
          nodeClearance: Number.POSITIVE_INFINITY,
          dependencyClearance: Number.POSITIVE_INFINITY,
        },
      ),
    ).toBeGreaterThan(0)
  })

  it('orders competing clearance scores and preserves exact ties', () => {
    expect(
      compareCandidateScores(
        { crossings: 0, nodeClearance: 60, dependencyClearance: 25 },
        { crossings: 0, nodeClearance: 50, dependencyClearance: 25 },
      ),
    ).toBeGreaterThan(0)
    expect(
      compareCandidateScores(
        { crossings: 0, nodeClearance: 60, dependencyClearance: 20 },
        { crossings: 0, nodeClearance: 50, dependencyClearance: 30 },
      ),
    ).toBe(0)
  })

  it('places six children on one complete orbit with minimum clearance', () => {
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

    expect(radii.filter((radius) => Math.abs(radius - INNER_RING_RADIUS) < 0.001)).toHaveLength(6)
    for (let left = 0; left < children.length; left++) {
      for (let right = left + 1; right < children.length; right++) {
        expect(
          Math.hypot(
            points[children[left]].x - points[children[right]].x,
            points[children[left]].y - points[children[right]].y,
          ),
        ).toBeGreaterThanOrEqual(MIN_CHILD_CENTER_CLEARANCE)
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
    ).toBeCloseTo(INNER_RING_RADIUS, 6)
    expect(
      Math.hypot(points[5].x - points[10].x, points[5].y - points[10].y),
    ).toBeCloseTo(INNER_RING_RADIUS, 6)
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
    ).toBeCloseTo(INNER_RING_RADIUS, 6)
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

  it('keeps every child in a large group on bounded, clear concentric orbits', () => {
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

    const radii = children.map((number) => Math.hypot(points[number].x, points[number].y))
    const distinctRadii = [...new Set(radii.map((radius) => radius.toFixed(3)))].map(Number)
    expect(distinctRadii).toHaveLength(2)
    expect(Math.min(...distinctRadii)).toBeGreaterThanOrEqual(INNER_RING_RADIUS)
    expect(Math.min(...distinctRadii)).toBeLessThanOrEqual(INNER_RING_RADIUS + 210)
    expect(Math.max(...distinctRadii) - Math.min(...distinctRadii)).toBeGreaterThanOrEqual(220)

    for (const number of children) {
      expect(Number.isFinite(points[number].x)).toBe(true)
      expect(Number.isFinite(points[number].y)).toBe(true)
    }
    for (let left = 0; left < children.length; left++) {
      for (let right = left + 1; right < children.length; right++) {
        expect(
          Math.hypot(
            points[children[left]].x - points[children[right]].x,
            points[children[left]].y - points[children[right]].y,
          ),
        ).toBeGreaterThanOrEqual(MIN_CHILD_CENTER_CLEARANCE)
      }
    }
  })
})

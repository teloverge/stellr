import type { Edge, LayoutNode, Point } from './layout'
import { workflowEdges, type WorkflowEdge } from './workflow'
import { reverseEdgeKeys } from './workflow-visual'
import {
  curveCrossesSegment,
  curveToSegmentClearance,
  miniEdgeCurve,
  sampleQuadratic,
  type QuadraticCurve,
  type Segment,
} from './workflow-geometry'

export const FIRST_ARC_RADIUS = 92
export const FIRST_ARC_CAPACITY = 5
export const SECOND_ARC_RADIUS = 126
export const SECOND_ARC_CAPACITY = 8
export const ARC_STEP = Math.PI / 6
export const MIN_CHILD_CENTER_CLEARANCE = 44
export const UNRELATED_NODE_CLEARANCE = 42
export const DEPENDENCY_LINE_CLEARANCE = 18
export const CANDIDATE_SECTORS = 16

interface Candidate {
  childPoints: Record<number, Point>
  collisionFree: boolean
  crossings: number
  nodeClearance: number
  dependencyClearance: number
}

interface CurveWithEdge {
  edge: WorkflowEdge
  curve: QuadraticCurve
}

interface SegmentObstacle {
  edge: Pick<WorkflowEdge, 'from' | 'to'>
  segment: Segment
}

function siblingOrder(children: LayoutNode[]): LayoutNode[] {
  const orderedChildren = [...children].sort((left, right) => left.num - right.num)
  const siblings = new Set(orderedChildren.map((child) => child.num))
  const byNumber = new Map(orderedChildren.map((child) => [child.num, child]))
  const outgoing = new Map<number, Set<number>>()
  const incoming = new Map(orderedChildren.map((child) => [child.num, 0]))

  for (const child of orderedChildren) {
    for (const blocker of new Set(child.blockedBy)) {
      if (blocker === child.num || !siblings.has(blocker)) continue
      const dependents = outgoing.get(blocker) ?? new Set<number>()
      if (dependents.has(child.num)) continue
      dependents.add(child.num)
      outgoing.set(blocker, dependents)
      incoming.set(child.num, (incoming.get(child.num) ?? 0) + 1)
    }
  }

  const ready = orderedChildren.filter((child) => incoming.get(child.num) === 0).map((child) => child.num)
  const result: LayoutNode[] = []
  while (ready.length > 0) {
    const current = ready.shift()!
    result.push(byNumber.get(current)!)
    for (const dependent of [...(outgoing.get(current) ?? [])].sort((left, right) => left - right)) {
      const next = (incoming.get(dependent) ?? 0) - 1
      incoming.set(dependent, next)
      if (next === 0) {
        ready.push(dependent)
        ready.sort((left, right) => left - right)
      }
    }
  }

  const emitted = new Set(result.map((child) => child.num))
  return [...result, ...orderedChildren.filter((child) => !emitted.has(child.num))]
}

function startingSector(parentNumber: number): number {
  return (Math.imul(parentNumber, 0x9e3779b1) >>> 0) % CANDIDATE_SECTORS
}

function hierarchyDepth(
  number: number,
  byNumber: Map<number, LayoutNode>,
  cache: Map<number, number | null>,
  visiting = new Set<number>(),
): number | null {
  if (cache.has(number)) return cache.get(number) ?? null
  if (visiting.has(number)) {
    cache.set(number, null)
    return null
  }
  const node = byNumber.get(number)
  if (!node) {
    cache.set(number, 0)
    return 0
  }
  const parent = node.parentIssue
  if (parent === null || parent === number || !byNumber.has(parent)) {
    cache.set(number, 0)
    return 0
  }

  visiting.add(number)
  const parentDepth = hierarchyDepth(parent, byNumber, cache, visiting)
  visiting.delete(number)
  const depth = parentDepth === null ? null : parentDepth + 1
  cache.set(number, depth)
  return depth
}

function centeredOffsets(count: number): number[] {
  return Array.from({ length: count }, (_, index) => (index - (count - 1) / 2) * ARC_STEP)
}

function staggeredSecondArcOffsets(count: number): number[] {
  const sideCount = count / 2
  return [
    ...Array.from({ length: sideCount }, (_, index) => -(sideCount - index) * ARC_STEP),
    ...Array.from({ length: sideCount }, (_, index) => (index + 1) * ARC_STEP),
  ]
}

function firstArcCount(childCount: number): number {
  if (childCount <= FIRST_ARC_CAPACITY) return childCount
  return childCount % 2 === 0 ? FIRST_ARC_CAPACITY - 1 : FIRST_ARC_CAPACITY
}

function arcPoints(
  parent: Point,
  children: LayoutNode[],
  sector: number,
  expanded = false,
): Record<number, Point> {
  const childPoints: Record<number, Point> = {}
  const centerAngle = (sector / CANDIDATE_SECTORS) * Math.PI * 2
  if (expanded && children.length <= FIRST_ARC_CAPACITY) {
    const offsets = centeredOffsets(children.length)
    for (let index = 0; index < offsets.length; index++) {
      const offset = offsets[index]
      const angle = centerAngle + offset
      childPoints[children[index].num] = {
        x: parent.x + Math.cos(angle) * SECOND_ARC_RADIUS,
        y: parent.y + Math.sin(angle) * SECOND_ARC_RADIUS,
      }
    }
    return childPoints
  }

  const firstCount = firstArcCount(children.length)
  const secondCount = children.length - firstCount
  const firstOffsets = centeredOffsets(firstCount)
  const secondOffsets = firstCount % 2 === 0
    ? staggeredSecondArcOffsets(secondCount)
    : centeredOffsets(secondCount)
  const slots = [
    ...firstOffsets.map((offset) => ({ offset, radius: FIRST_ARC_RADIUS })),
    ...secondOffsets.map((offset) => ({ offset, radius: SECOND_ARC_RADIUS })),
  ]

  for (let index = 0; index < children.length; index++) {
    const angle = centerAngle + slots[index].offset
    childPoints[children[index].num] = {
      x: parent.x + Math.cos(angle) * slots[index].radius,
      y: parent.y + Math.sin(angle) * slots[index].radius,
    }
  }
  return childPoints
}

function distance(left: Point, right: Point): number {
  return Math.hypot(left.x - right.x, left.y - right.y)
}

function isFinitePoint(point: Point | undefined): point is Point {
  return point !== undefined && Number.isFinite(point.x) && Number.isFinite(point.y)
}

function clusterCurves(
  parentNode: LayoutNode,
  children: LayoutNode[],
  parentPoint: Point,
  childPoints: Record<number, Point>,
): CurveWithEdge[] {
  const edges = workflowEdges([parentNode, ...children]).filter((edge) =>
    edge.roles.some((role) => role === 'entry' || role === 'sequence' || role === 'return'),
  )
  const reversed = reverseEdgeKeys(edges)
  const points = { ...childPoints, [parentNode.num]: parentPoint }
  return edges.flatMap((edge) => {
    const start = points[edge.from]
    const end = points[edge.to]
    if (!start || !end) return []
    return [
      {
        edge,
        curve: miniEdgeCurve({
          edge,
          start,
          end,
          reverseExists: reversed.has(`${edge.from}>${edge.to}`),
          parent: parentPoint,
        }),
      },
    ]
  })
}

function dependencyObstacles(
  edges: Edge[],
  clusterNumbers: Set<number>,
  points: Record<number, Point>,
): SegmentObstacle[] {
  return edges.flatMap((edge) => {
    if (clusterNumbers.has(edge.from) && clusterNumbers.has(edge.to)) return []
    const start = points[edge.from]
    const end = points[edge.to]
    if (!start || !end) return []
    return [{ edge, segment: { start, end } }]
  })
}

function minimum(values: number[]): number {
  return values.length === 0 ? Number.POSITIVE_INFINITY : Math.min(...values)
}

function evaluateCandidate(
  nodes: LayoutNode[],
  edges: Edge[],
  parentNode: LayoutNode,
  children: LayoutNode[],
  parentPoint: Point,
  childPoints: Record<number, Point>,
  currentPoints: Record<number, Point>,
  placedCurveObstacles: SegmentObstacle[],
): Candidate {
  const clusterNumbers = new Set([parentNode.num, ...children.map((child) => child.num)])
  const proposedPoints = { ...currentPoints, ...childPoints }
  const unrelatedPoints = nodes
    .filter((node) => !clusterNumbers.has(node.num))
    .map((node) => proposedPoints[node.num])
    .filter(isFinitePoint)
  const childPointList = children.map((child) => childPoints[child.num])
  const childClearances: number[] = []
  for (let left = 0; left < childPointList.length; left++) {
    for (let right = left + 1; right < childPointList.length; right++) {
      childClearances.push(distance(childPointList[left], childPointList[right]))
    }
  }

  const curves = clusterCurves(parentNode, children, parentPoint, childPoints)
  const nodeClearances = childPointList.flatMap((child) =>
    unrelatedPoints.map((other) => distance(child, other)),
  )
  const curveNodeClearances = curves.flatMap(({ curve }) =>
    sampleQuadratic(curve).flatMap((point) => unrelatedPoints.map((other) => distance(point, other))),
  )
  const obstacles = [
    ...dependencyObstacles(edges, clusterNumbers, proposedPoints),
    ...placedCurveObstacles,
  ]
  const dependencyClearances: number[] = []
  let crossings = 0
  for (const { edge: miniEdge, curve } of curves) {
    for (const obstacle of obstacles) {
      const sharesEndpoint =
        obstacle.edge.from === miniEdge.from ||
        obstacle.edge.from === miniEdge.to ||
        obstacle.edge.to === miniEdge.from ||
        obstacle.edge.to === miniEdge.to
      if (sharesEndpoint) continue
      dependencyClearances.push(curveToSegmentClearance(curve, obstacle.segment))
      if (curveCrossesSegment(curve, obstacle.segment)) crossings++
    }
  }

  const childClearance = minimum(childClearances)
  const nodeClearance = Math.min(minimum(nodeClearances), minimum(curveNodeClearances))
  const dependencyClearance = minimum(dependencyClearances)
  return {
    childPoints,
    collisionFree:
      crossings === 0 &&
      childClearance >= MIN_CHILD_CENTER_CLEARANCE &&
      nodeClearance >= UNRELATED_NODE_CLEARANCE &&
      dependencyClearance >= DEPENDENCY_LINE_CLEARANCE,
    crossings,
    nodeClearance,
    dependencyClearance,
  }
}

function finiteClearance(value: number): number {
  return Number.isFinite(value) ? value : 1_000_000
}

function isBetter(candidate: Candidate, current: Candidate | null): boolean {
  if (!current) return true
  if (candidate.collisionFree !== current.collisionFree) return candidate.collisionFree
  if (candidate.crossings !== current.crossings) return candidate.crossings < current.crossings
  const candidateScore =
    finiteClearance(candidate.nodeClearance) + finiteClearance(candidate.dependencyClearance)
  const currentScore =
    finiteClearance(current.nodeClearance) + finiteClearance(current.dependencyClearance)
  return candidateScore > currentScore
}

export function placeDirectChildClusters(
  nodes: LayoutNode[],
  broadPoints: Record<number, Point>,
  dependencyEdges: Edge[] = [],
): Record<number, Point> {
  const points = Object.fromEntries(
    Object.entries(broadPoints).map(([number, point]) => [number, { ...point }]),
  ) as Record<number, Point>
  const present = new Set(nodes.map((node) => node.num))
  const byNumber = new Map(nodes.map((node) => [node.num, node]))
  const depthCache = new Map<number, number | null>()
  const childrenByParent = new Map<number, LayoutNode[]>()
  const placedCurveObstacles: SegmentObstacle[] = []

  for (const node of nodes) {
    const parent = node.parentIssue
    if (parent === null || parent === node.num || !present.has(parent)) continue
    const children = childrenByParent.get(parent) ?? []
    children.push(node)
    childrenByParent.set(parent, children)
  }

  const orderedGroups = [...childrenByParent].flatMap(([parentNumber, children]) => {
    const depth = hierarchyDepth(parentNumber, byNumber, depthCache)
    return depth === null ? [] : [{ parentNumber, children, depth }]
  })
    .sort((left, right) => left.depth - right.depth || left.parentNumber - right.parentNumber)

  for (const { parentNumber, children } of orderedGroups) {
    const parent = points[parentNumber]
    const parentNode = byNumber.get(parentNumber)
    if (!isFinitePoint(parent) || !parentNode || children.length === 0) continue
    const ordered = siblingOrder(children)
      .filter((child) => isFinitePoint(broadPoints[child.num]))
      .slice(0, FIRST_ARC_CAPACITY + SECOND_ARC_CAPACITY)
    if (ordered.length === 0) continue
    const start = startingSector(parentNumber)
    const candidates = (expanded: boolean) =>
      Array.from({ length: CANDIDATE_SECTORS }, (_, offset) => {
        const sector = (start + offset) % CANDIDATE_SECTORS
        return evaluateCandidate(
          nodes,
          dependencyEdges,
          parentNode,
          ordered,
          parent,
          arcPoints(parent, ordered, sector, expanded),
          points,
          placedCurveObstacles,
        )
      })
    const compactCandidates = candidates(false)
    let pool = compactCandidates
    if (
      ordered.length <= FIRST_ARC_CAPACITY &&
      !compactCandidates.some((candidate) => candidate.collisionFree)
    ) {
      pool = [...compactCandidates, ...candidates(true)]
    }
    let selected: Candidate | null = null
    for (const candidate of pool) {
      if (isBetter(candidate, selected)) selected = candidate
    }
    if (selected) {
      Object.assign(points, selected.childPoints)
      for (const { edge, curve } of clusterCurves(
        parentNode,
        ordered,
        parent,
        selected.childPoints,
      )) {
        const samples = sampleQuadratic(curve)
        for (let index = 1; index < samples.length; index++) {
          placedCurveObstacles.push({
            edge,
            segment: { start: samples[index - 1], end: samples[index] },
          })
        }
      }
    }
  }

  return points
}

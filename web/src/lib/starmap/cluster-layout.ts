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

function arcPoints(parent: Point, children: LayoutNode[], sector: number): Record<number, Point> {
  const childPoints: Record<number, Point> = {}
  const centerAngle = (sector / CANDIDATE_SECTORS) * Math.PI * 2
  for (let index = 0; index < children.length; index++) {
    const offset = (index - (children.length - 1) / 2) * ARC_STEP
    const angle = centerAngle + offset
    childPoints[children[index].num] = {
      x: parent.x + Math.cos(angle) * FIRST_ARC_RADIUS,
      y: parent.y + Math.sin(angle) * FIRST_ARC_RADIUS,
    }
  }
  return childPoints
}

function distance(left: Point, right: Point): number {
  return Math.hypot(left.x - right.x, left.y - right.y)
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
): Array<{ edge: Edge; segment: Segment }> {
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
): Candidate {
  const clusterNumbers = new Set([parentNode.num, ...children.map((child) => child.num)])
  const proposedPoints = { ...currentPoints, ...childPoints }
  const unrelatedPoints = nodes
    .filter((node) => !clusterNumbers.has(node.num))
    .map((node) => proposedPoints[node.num])
    .filter((point): point is Point => point !== undefined)
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
  const obstacles = dependencyObstacles(edges, clusterNumbers, proposedPoints)
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
  const childrenByParent = new Map<number, LayoutNode[]>()

  for (const node of nodes) {
    const parent = node.parentIssue
    if (parent === null || parent === node.num || !present.has(parent)) continue
    const children = childrenByParent.get(parent) ?? []
    children.push(node)
    childrenByParent.set(parent, children)
  }

  for (const [parentNumber, children] of [...childrenByParent].sort(
    ([left], [right]) => left - right,
  )) {
    const parent = points[parentNumber]
    const parentNode = byNumber.get(parentNumber)
    if (!parent || !parentNode || children.length === 0 || children.length > FIRST_ARC_CAPACITY) continue
    const ordered = siblingOrder(children)
    const start = startingSector(parentNumber)
    let selected: Candidate | null = null
    for (let offset = 0; offset < CANDIDATE_SECTORS; offset++) {
      const sector = (start + offset) % CANDIDATE_SECTORS
      const candidate = evaluateCandidate(
        nodes,
        dependencyEdges,
        parentNode,
        ordered,
        parent,
        arcPoints(parent, ordered, sector),
        points,
      )
      if (isBetter(candidate, selected)) selected = candidate
    }
    if (selected) Object.assign(points, selected.childPoints)
  }

  return points
}

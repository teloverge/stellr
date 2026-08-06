import type { Edge, LayoutNode, Point } from './layout'
import {
  boxesOverlap,
  estimateLabelWidth,
  labelBox,
  orbitLabelFontSize,
  ORBIT_LABEL_REFERENCE_SCALE,
  outwardLabelGeometryAtScale,
  segmentIntersectsBox,
  type LabelBox,
} from './label-geometry'
import { validOrbitNodeNumbers } from './parent-topology'
import { isMiniWorkflowEdge, workflowEdges, type WorkflowEdge } from './workflow'
import { reverseEdgeKeys } from './workflow-visual'
import {
  curveCrossesSegment,
  curveToSegmentClearance,
  miniEdgeCurve,
  sampleQuadratic,
  type QuadraticCurve,
  type Segment,
} from './workflow-geometry'

export const INNER_RING_RADIUS = 180
export const MIN_CHILD_CENTER_CLEARANCE = 72
export const UNRELATED_NODE_CLEARANCE = 64
export const DEPENDENCY_LINE_CLEARANCE = 18
export const CANDIDATE_SECTORS = 16
export const CLEARANCE_SCORE_CAP = 800
export const NODE_CLEARANCE_SCORE_WEIGHT = 1
export const DEPENDENCY_CLEARANCE_SCORE_WEIGHT = 1
export const CROSSING_SCORE_PENALTY =
  CLEARANCE_SCORE_CAP *
    (NODE_CLEARANCE_SCORE_WEIGHT + DEPENDENCY_CLEARANCE_SCORE_WEIGHT) +
  1

export interface CandidateScoreInput {
  crossings: number
  nodeClearance: number
  dependencyClearance: number
  labelCollisions?: number
}

interface Candidate extends CandidateScoreInput {
  childPoints: Record<number, Point>
  collisionFree: boolean
}

interface CurveWithEdge {
  edge: WorkflowEdge
  curve: QuadraticCurve
}

interface SegmentObstacle {
  edge: Pick<WorkflowEdge, 'from' | 'to'>
  segment: Segment
}

interface EvaluationContext {
  nodes: LayoutNode[]
  edges: Edge[]
  parentNode: LayoutNode
  children: LayoutNode[]
  parentPoint: Point
  currentPoints: Record<number, Point>
  placedCurveObstacles: SegmentObstacle[]
  placedLabelObstacles: LabelBox[]
  topLevelLabelObstacles: LabelBox[]
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

const MIN_RING_GAP = 220
const MIN_RING_SLOT_ARC = 112
const MAX_RING_SLOT_ARC = 170
const TARGET_INNER_RING_CAPACITY = 7
const LAYOUT_FONT_SIZE = 14
const LAYOUT_STAR_RADIUS = 14
const EXPANSION_STEPS = [0, 70, 140, 210] as const

interface RingMetrics {
  innerRadius: number
  ringGap: number
}

function childSlotArc(child: LayoutNode): number {
  const fontSize = orbitLabelFontSize(ORBIT_LABEL_REFERENCE_SCALE) / ORBIT_LABEL_REFERENCE_SCALE
  const width = estimateLabelWidth(child.num, child.title ?? '', fontSize)
  return Math.min(MAX_RING_SLOT_ARC, Math.max(MIN_RING_SLOT_ARC, 70 + width * 0.25))
}

function ringMetrics(children: LayoutNode[]): RingMetrics {
  const fontSize = orbitLabelFontSize(ORBIT_LABEL_REFERENCE_SCALE) / ORBIT_LABEL_REFERENCE_SCALE
  const widths = children.map((child) =>
    estimateLabelWidth(child.num, child.title ?? '', fontSize),
  )
  const maximumWidth = widths.length === 0 ? 0 : Math.max(...widths)
  const innerFootprint = children
    .slice(0, TARGET_INNER_RING_CAPACITY)
    .reduce((sum, child) => sum + childSlotArc(child), 0)
  return {
    innerRadius: Math.max(INNER_RING_RADIUS, innerFootprint / (Math.PI * 2)),
    ringGap: Math.max(MIN_RING_GAP, Math.min(340, 80 + maximumWidth * 0.55)),
  }
}

function orbitRings(children: LayoutNode[], minimumRingCount = 1): LayoutNode[][] {
  const metrics = ringMetrics(children)
  const rings: LayoutNode[][] = []
  let occupiedArc = 0
  for (const child of children) {
    let ring = rings.at(-1)
    const radius = metrics.innerRadius + Math.max(0, rings.length - 1) * metrics.ringGap
    const footprint = childSlotArc(child)
    if (!ring || (ring.length > 0 && occupiedArc + footprint > Math.PI * 2 * radius)) {
      ring = []
      rings.push(ring)
      occupiedArc = 0
    }
    ring.push(child)
    occupiedArc += footprint
  }

  while (rings.length < minimumRingCount) {
    let splitIndex = -1
    for (let index = 0; index < rings.length; index++) {
      if (rings[index].length > 1 && (splitIndex < 0 || rings[index].length > rings[splitIndex].length)) {
        splitIndex = index
      }
    }
    if (splitIndex < 0) break
    const ring = rings[splitIndex]
    const splitAt = Math.ceil(ring.length / 2)
    rings.splice(splitIndex, 1, ring.slice(0, splitAt), ring.slice(splitAt))
  }
  return rings
}

export function orbitRingCounts(children: LayoutNode[]): number[] {
  return orbitRings(children).map((ring) => ring.length)
}

function orbitPoints(
  parent: Point,
  children: LayoutNode[],
  sector: number,
  radialExpansion = 0,
  minimumRingCount = 1,
): Record<number, Point> {
  const childPoints: Record<number, Point> = {}
  const centerAngle = (sector / CANDIDATE_SECTORS) * Math.PI * 2
  const metrics = ringMetrics(children)
  const rings = orbitRings(children, minimumRingCount)
  for (const [ringIndex, ring] of rings.entries()) {
    const radius = metrics.innerRadius + radialExpansion + ringIndex * metrics.ringGap
    const count = ring.length
    const step = (Math.PI * 2) / count
    const stagger = ringIndex % 2 === 0 ? 0 : step / 2
    for (let slot = 0; slot < count; slot++) {
      const child = ring[slot]
      const angle = centerAngle + stagger + slot * step
      childPoints[child.num] = {
        x: parent.x + Math.cos(angle) * radius,
        y: parent.y + Math.sin(angle) * radius,
      }
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

function pointBox(point: Point, radius: number): LabelBox {
  return {
    x0: point.x - radius,
    y0: point.y - radius,
    x1: point.x + radius,
    y1: point.y + radius,
  }
}

function topLevelLabelBoxes(
  nodes: LayoutNode[],
  points: Record<number, Point>,
  validOrbitNodes: Set<number>,
): LabelBox[] {
  const gap = 4
  return nodes.flatMap((node) => {
    if (validOrbitNodes.has(node.num)) return []
    const point = points[node.num]
    if (!isFinitePoint(point)) return []
    const width = estimateLabelWidth(node.num, node.title ?? '', LAYOUT_FONT_SIZE)
    const below = point.y + LAYOUT_STAR_RADIUS + gap + LAYOUT_FONT_SIZE * 0.82
    const above = point.y - LAYOUT_STAR_RADIUS - gap - LAYOUT_FONT_SIZE * 0.22
    return [
      labelBox(point.x, below, 'center', width, LAYOUT_FONT_SIZE),
      labelBox(point.x, above, 'center', width, LAYOUT_FONT_SIZE),
    ]
  })
}

function clusterLabelBoxes(
  children: LayoutNode[],
  parent: Point,
  childPoints: Record<number, Point>,
): Array<{ number: number; box: LabelBox }> {
  return children.map((child) => ({
    number: child.num,
    box: outwardLabelGeometryAtScale({
      parent,
      child: childPoints[child.num],
      number: child.num,
      title: child.title ?? '',
      scale: ORBIT_LABEL_REFERENCE_SCALE,
      starRadius: LAYOUT_STAR_RADIUS,
    }).box,
  }))
}

function clusterCurves(
  parentNode: LayoutNode,
  children: LayoutNode[],
  parentPoint: Point,
  childPoints: Record<number, Point>,
): CurveWithEdge[] {
  const edges = workflowEdges([parentNode, ...children]).filter(isMiniWorkflowEdge)
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
  context: EvaluationContext,
  childPoints: Record<number, Point>,
): Candidate {
  const {
    nodes,
    edges,
    parentNode,
    children,
    parentPoint,
    currentPoints,
    placedCurveObstacles,
    placedLabelObstacles,
    topLevelLabelObstacles,
  } = context
  const clusterNumbers = new Set([parentNode.num, ...children.map((child) => child.num)])
  const proposedPoints = { ...currentPoints, ...childPoints }
  const unrelatedPoints = nodes
    .filter((node) => !clusterNumbers.has(node.num))
    .map((node) => proposedPoints[node.num])
    .filter(isFinitePoint)
  const childPointList = children.map((child) => childPoints[child.num])
  const childLabels = clusterLabelBoxes(children, parentPoint, childPoints)
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
  const relationshipSegments: SegmentObstacle[] = [
    ...obstacles,
    ...curves.flatMap(({ edge, curve }) => {
      const samples = sampleQuadratic(curve)
      return samples.slice(1).map((end, index) => ({
        edge,
        segment: { start: samples[index], end },
      }))
    }),
  ]
  const dependencyClearances: number[] = []
  let crossings = 0
  for (const { edge: miniEdge, curve } of curves) {
    for (const obstacle of obstacles) {
      const sharedNumbers = [miniEdge.from, miniEdge.to].filter(
        (number) => obstacle.edge.from === number || obstacle.edge.to === number,
      )
      const touchesSharedEndpoint = sharedNumbers.some((number) => {
        const point = proposedPoints[number]
        return (
          isFinitePoint(point) &&
          Math.min(
            distance(obstacle.segment.start, point),
            distance(obstacle.segment.end, point),
          ) < DEPENDENCY_LINE_CLEARANCE
        )
      })
      if (touchesSharedEndpoint) continue
      dependencyClearances.push(curveToSegmentClearance(curve, obstacle.segment))
      if (curveCrossesSegment(curve, obstacle.segment)) crossings++
    }
  }

  const childClearance = minimum(childClearances)
  const nodeClearance = Math.min(minimum(nodeClearances), minimum(curveNodeClearances))
  const dependencyClearance = minimum(dependencyClearances)
  let labelCollisions = 0
  const unrelatedLabels = [...placedLabelObstacles, ...topLevelLabelObstacles]
  for (const childPoint of childPointList) {
    const childBox = pointBox(childPoint, LAYOUT_STAR_RADIUS)
    for (const unrelatedLabel of unrelatedLabels) {
      if (boxesOverlap(childBox, unrelatedLabel)) labelCollisions++
    }
  }
  for (let left = 0; left < childLabels.length; left++) {
    for (let right = left + 1; right < childLabels.length; right++) {
      if (boxesOverlap(childLabels[left].box, childLabels[right].box)) labelCollisions++
    }
    for (const child of children) {
      if (child.num === childLabels[left].number) continue
      if (boxesOverlap(childLabels[left].box, pointBox(childPoints[child.num], LAYOUT_STAR_RADIUS))) {
        labelCollisions++
      }
    }
    for (const point of unrelatedPoints) {
      if (boxesOverlap(childLabels[left].box, pointBox(point, LAYOUT_STAR_RADIUS))) {
        labelCollisions++
      }
    }
    for (const unrelatedLabel of placedLabelObstacles) {
      if (boxesOverlap(childLabels[left].box, unrelatedLabel)) labelCollisions++
    }
    for (const topLevelLabel of topLevelLabelObstacles) {
      if (boxesOverlap(childLabels[left].box, topLevelLabel)) labelCollisions++
    }
    for (const { edge, segment } of relationshipSegments) {
      if (edge.from === childLabels[left].number || edge.to === childLabels[left].number) {
        continue
      }
      if (segmentIntersectsBox(segment.start, segment.end, childLabels[left].box)) {
        labelCollisions++
      }
    }
  }
  return {
    childPoints,
    collisionFree:
      crossings === 0 &&
      labelCollisions === 0 &&
      childClearance >= MIN_CHILD_CENTER_CLEARANCE &&
      nodeClearance >= UNRELATED_NODE_CLEARANCE &&
      dependencyClearance >= DEPENDENCY_LINE_CLEARANCE,
    crossings,
    nodeClearance,
    dependencyClearance,
    labelCollisions,
  }
}

function boundedClearanceScore(clearance: number): number {
  if (Number.isNaN(clearance)) return 0
  return Math.min(Math.max(clearance, 0), CLEARANCE_SCORE_CAP)
}

function candidateScore(candidate: CandidateScoreInput): number {
  return (
    boundedClearanceScore(candidate.nodeClearance) * NODE_CLEARANCE_SCORE_WEIGHT +
    boundedClearanceScore(candidate.dependencyClearance) *
      DEPENDENCY_CLEARANCE_SCORE_WEIGHT -
    candidate.crossings * CROSSING_SCORE_PENALTY -
    (candidate.labelCollisions ?? 0) * CROSSING_SCORE_PENALTY
  )
}

export function compareCandidateScores(
  left: CandidateScoreInput,
  right: CandidateScoreInput,
): number {
  return candidateScore(left) - candidateScore(right)
}

function isBetter(candidate: Candidate, current: Candidate | null): boolean {
  if (!current) return true
  if (candidate.collisionFree !== current.collisionFree) return candidate.collisionFree
  return compareCandidateScores(candidate, current) > 0
}

export function placeDirectChildClusters(
  nodes: LayoutNode[],
  broadPoints: Record<number, Point>,
  dependencyEdges: Edge[] = [],
): Record<number, Point> {
  const points = Object.fromEntries(
    Object.entries(broadPoints).map(([number, point]) => [number, { ...point }]),
  ) as Record<number, Point>
  const validOrbitNodes = validOrbitNodeNumbers(nodes)
  const byNumber = new Map(nodes.map((node) => [node.num, node]))
  const depthCache = new Map<number, number | null>()
  const childrenByParent = new Map<number, LayoutNode[]>()
  const placedCurveObstacles: SegmentObstacle[] = []
  const placedLabelObstacles: LabelBox[] = []
  const topLevelLabelObstacles = topLevelLabelBoxes(nodes, points, validOrbitNodes)

  for (const node of nodes) {
    const parent = node.parentIssue
    if (parent === null || !validOrbitNodes.has(node.num)) continue
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
    const ordered = siblingOrder(children).filter((child) =>
      isFinitePoint(broadPoints[child.num]),
    )
    if (ordered.length === 0) continue
    const start = startingSector(parentNumber)
    const evaluationContext: EvaluationContext = {
      nodes,
      edges: dependencyEdges,
      parentNode,
      children: ordered,
      parentPoint: parent,
      currentPoints: points,
      placedCurveObstacles,
      placedLabelObstacles,
      topLevelLabelObstacles,
    }
    const candidates = (radialExpansion: number, minimumRingCount: number) =>
      Array.from({ length: CANDIDATE_SECTORS }, (_, offset) => {
        const sector = (start + offset) % CANDIDATE_SECTORS
        return evaluateCandidate(
          evaluationContext,
          orbitPoints(parent, ordered, sector, radialExpansion, minimumRingCount),
        )
      })
    const naturalRingCount = orbitRingCounts(ordered).length
    const baseCandidates = candidates(EXPANSION_STEPS[0], naturalRingCount)
    let pool = baseCandidates
    if (!baseCandidates.some((candidate) => candidate.collisionFree)) {
      const maximumRingCount = Math.min(ordered.length, naturalRingCount + 3)
      pool = EXPANSION_STEPS.flatMap((radialExpansion) =>
        Array.from(
          { length: maximumRingCount - naturalRingCount + 1 },
          (_, offset) => candidates(radialExpansion, naturalRingCount + offset),
        ).flat(),
      )
    }
    let selected: Candidate | null = null
    for (const candidate of pool) {
      if (isBetter(candidate, selected)) selected = candidate
    }
    if (selected) {
      Object.assign(points, selected.childPoints)
      placedLabelObstacles.push(
        ...clusterLabelBoxes(ordered, parent, selected.childPoints).map(({ box }) => box),
      )
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

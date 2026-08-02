import type { LayoutNode, Point } from './layout'

export const FIRST_ARC_RADIUS = 92
export const FIRST_ARC_CAPACITY = 5
export const ARC_STEP = Math.PI / 6
export const MIN_CHILD_CENTER_CLEARANCE = 44
export const CANDIDATE_SECTORS = 16

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

export function placeDirectChildClusters(
  nodes: LayoutNode[],
  broadPoints: Record<number, Point>,
): Record<number, Point> {
  const points = Object.fromEntries(
    Object.entries(broadPoints).map(([number, point]) => [number, { ...point }]),
  ) as Record<number, Point>
  const present = new Set(nodes.map((node) => node.num))
  const childrenByParent = new Map<number, LayoutNode[]>()

  for (const node of nodes) {
    const parent = node.parentIssue
    if (parent === null || parent === node.num || !present.has(parent)) continue
    const children = childrenByParent.get(parent) ?? []
    children.push(node)
    childrenByParent.set(parent, children)
  }

  for (const [parentNumber, children] of [...childrenByParent].sort(([left], [right]) => left - right)) {
    const parent = points[parentNumber]
    if (!parent || children.length === 0 || children.length > FIRST_ARC_CAPACITY) continue
    const ordered = siblingOrder(children)
    const centerAngle = (startingSector(parentNumber) / CANDIDATE_SECTORS) * Math.PI * 2
    for (let index = 0; index < ordered.length; index++) {
      const offset = (index - (ordered.length - 1) / 2) * ARC_STEP
      const angle = centerAngle + offset
      points[ordered[index].num] = {
        x: parent.x + Math.cos(angle) * FIRST_ARC_RADIUS,
        y: parent.y + Math.sin(angle) * FIRST_ARC_RADIUS,
      }
    }
  }

  return points
}

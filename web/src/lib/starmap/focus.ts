import type { Ticket } from './model'
import { edgeKey, workflowEdges, type WorkflowEdge } from './workflow'

export interface Focus {
  current: number | null
  ready: number[]
  readySet: Set<number>
  pathNodes: Set<number>
  pathEdges: Set<string>
  emphasized: Set<number>
}

function isClosed(ticket: Ticket): boolean {
  const status = ticket.focusStatus ?? ticket.status
  return status === 'resolved' || status === 'out_of_scope'
}

interface Route {
  nodes: number[]
  edges: WorkflowEdge[]
}

interface SearchStep {
  nodes: number[]
  edges: WorkflowEdge[]
  miniOnly: boolean
  throughClosed: boolean
}

function route(
  starts: number[],
  goals: Set<number>,
  byNumber: Map<number, Ticket>,
  adjacency: Map<number, WorkflowEdge[]>,
  allowResolvedMiniRoute: boolean,
): Route | null {
  const queue: SearchStep[] = starts.map((start) => ({
    nodes: [start],
    edges: [],
    miniOnly: true,
    throughClosed: false,
  }))
  const visited = new Set(starts.map((start) => `${start}|true|false`))

  while (queue.length > 0) {
    const step = queue.shift()!
    const number = step.nodes.at(-1)!
    if (goals.has(number)) {
      return { nodes: step.nodes, edges: step.edges }
    }

    for (const edge of adjacency.get(number) ?? []) {
      const ticket = byNumber.get(edge.to)
      if (!ticket) continue
      const goal = goals.has(edge.to)
      const miniEdge = edge.child !== null
      const miniOnly = step.miniOnly && miniEdge
      if (step.throughClosed && !miniEdge) continue
      if (!goal && isClosed(ticket)) {
        const status = ticket.focusStatus ?? ticket.status
        if (!allowResolvedMiniRoute || status !== 'resolved' || !miniOnly) continue
      }
      const throughClosed = step.throughClosed || (!goal && isClosed(ticket))
      const visit = `${edge.to}|${miniOnly}|${throughClosed}`
      if (visited.has(visit)) continue
      visited.add(visit)
      queue.push({
        nodes: [...step.nodes, edge.to],
        edges: [...step.edges, edge],
        miniOnly,
        throughClosed,
      })
    }
  }

  return null
}

export function analyzeFocus(tickets: Ticket[], requestedCurrent: number | null): Focus {
  const byNumber = new Map(tickets.map((ticket) => [ticket.num, ticket]))
  const current = requestedCurrent !== null && byNumber.has(requestedCurrent)
    ? requestedCurrent
    : null
  const actionable = tickets
    .filter((ticket) => ticket.readyForAgent === true)
    .map((ticket) => ticket.num)
    .sort((a, b) => a - b)
  const adjacency = new Map<number, WorkflowEdge[]>()
  for (const edge of workflowEdges(tickets)) {
    const outgoing = adjacency.get(edge.from) ?? []
    outgoing.push(edge)
    adjacency.set(edge.from, outgoing)
  }

  const pathNodes = new Set<number>()
  const pathEdges = new Set<string>()
  let onPath: number | null = null

  if (current !== null) {
    const readyGoals = new Set(actionable)
    let path = route([current], readyGoals, byNumber, adjacency, true)
    let readyOnPath = path?.nodes.at(-1) ?? null
    if (path === null) {
      path = route(actionable, new Set([current]), byNumber, adjacency, false)
      readyOnPath = path?.nodes[0] ?? null
    }
    if (path !== null) {
      onPath = readyOnPath
      for (const number of path.nodes) pathNodes.add(number)
      for (const edge of path.edges) pathEdges.add(edgeKey(edge.from, edge.to))
    }
  }

  const ready = onPath === null
    ? actionable
    : [onPath, ...actionable.filter((number) => number !== onPath)]
  const readySet = new Set(ready)
  const emphasized = new Set<number>()
  if (current !== null) emphasized.add(current)
  for (const number of ready) emphasized.add(number)
  for (const number of pathNodes) emphasized.add(number)

  return { current, ready, readySet, pathNodes, pathEdges, emphasized }
}

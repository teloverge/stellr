import type { Ticket } from './model'
import { edgeKey, type WorkflowEdge } from './workflow'

export type WorkflowVisualState = 'incomplete' | 'completed' | 'traversed'

export function workflowVisualState(
  edge: WorkflowEdge,
  tickets: Map<number, Ticket>,
): WorkflowVisualState {
  if (tickets.get(edge.from)?.status === 'resolved' && tickets.get(edge.to)?.status === 'frontier') {
    return 'traversed'
  }
  if (edge.child !== null && tickets.get(edge.child)?.status === 'resolved') return 'completed'
  return 'incomplete'
}

export function curveSide(edge: WorkflowEdge, reverseExists: boolean): -1 | 1 {
  return reverseExists && edge.from > edge.to ? -1 : 1
}

export function reverseEdgeKeys(edges: WorkflowEdge[]): Set<string> {
  const keys = new Set(edges.map((edge) => edgeKey(edge.from, edge.to)))
  return new Set(
    edges
      .filter((edge) => keys.has(edgeKey(edge.to, edge.from)))
      .map((edge) => edgeKey(edge.from, edge.to)),
  )
}

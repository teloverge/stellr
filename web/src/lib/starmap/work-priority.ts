import type { Status } from '../model'
import type { Ticket } from './model'

export type WorkPriority = 'in_progress' | 'ready' | 'frontier' | 'blocked' | 'terminal'

export interface WorkPriorityInput {
  status: Status
  labels: readonly string[]
  assignees: readonly string[]
}

function hasLabel(labels: readonly string[], expected: string): boolean {
  return labels.some((label) => label.toLowerCase() === expected)
}

export function deriveWorkPriority(input: WorkPriorityInput): WorkPriority {
  if (input.status === 'resolved' || input.status === 'out_of_scope') return 'terminal'
  if (hasLabel(input.labels, 'in-progress')) return 'in_progress'
  if (input.status === 'claimed' || input.assignees.length > 0) return 'in_progress'
  if (input.status === 'frontier' && hasLabel(input.labels, 'ready-for-agent')) return 'ready'
  return input.status === 'blocked' ? 'blocked' : 'frontier'
}

export function ticketWorkPriority(
  ticket: Pick<Ticket, 'status' | 'frontier' | 'readyForAgent' | 'workPriority'>,
): WorkPriority {
  if (ticket.workPriority) return ticket.workPriority
  if (ticket.status === 'resolved' || ticket.status === 'out_of_scope') return 'terminal'
  if (ticket.status === 'claimed') return 'in_progress'
  if (ticket.readyForAgent) return 'ready'
  return ticket.frontier ? 'frontier' : 'blocked'
}

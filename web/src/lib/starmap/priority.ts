import type { Ticket } from './model'
import type { SessionState } from './session'

export type WorkPriority =
  | 'attention'
  | 'doing_now'
  | 'my_next'
  | 'my_future'
  | 'available_next'
  | 'team_work'
  | 'planning'
  | 'resolved'
  | 'out_of_scope'

export function deriveWorkPriority(
  ticket: Ticket,
  session: SessionState | null,
  currentIssue: number | null,
): WorkPriority {
  if (ticket.status === 'resolved') return 'resolved'
  if (ticket.status === 'out_of_scope') return 'out_of_scope'
  if (session === 'blocked' || session === 'dead') return 'attention'
  if (session === 'implementing' || ticket.num === currentIssue) return 'doing_now'
  if (ticket.assignedToViewer) {
    return ticket.readyForAgent ? 'my_next' : 'my_future'
  }
  if (ticket.readyForAgent && ticket.status !== 'claimed') return 'available_next'
  if (ticket.status === 'claimed') return 'team_work'
  return 'planning'
}

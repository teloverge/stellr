// Derived from chartr (https://github.com/rengwu/chartr), MIT, Copyright (c) 2026 John Goh.

import type { WorkPriority } from './work-priority'

export type TicketStatus =
  | 'open'
  | 'blocked'
  | 'frontier'
  | 'claimed'
  | 'resolved'
  | 'out_of_scope'

export interface Ticket {
  num: number
  slug: string
  title: string
  type: string
  status: TicketStatus
  blockedBy: number[]
  parentIssue: number | null
  frontier: boolean
  readyForAgent?: boolean
  assignedToViewer?: boolean
  blocked?: boolean
  visible?: boolean
  focusStatus?: TicketStatus
  milestone?: string | null
  historical?: boolean
  workPriority?: WorkPriority
}

export interface Map {
  slug: string
  name: string
  dir: string
  destination: string
  tickets: Ticket[]
  finished: boolean
}

export interface TerminalSession {
  mapSlug: string
  ticketNum: number
  role: string
  agent: string
}

export interface Terminal {
  id: string
  title: string
  proc: string
  status: 'working' | 'blocked' | 'dead' | 'idle'
  alive: boolean
  session?: TerminalSession
}

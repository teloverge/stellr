export type Status = 'blocked' | 'frontier' | 'claimed' | 'resolved' | 'out_of_scope'

export type HistoryImportState =
  | 'unavailable'
  | 'building'
  | 'complete'
  | 'delayed'
  | 'rate_limited'
  | 'failed'

export interface HistorySummary {
  state: HistoryImportState
  completed_issues: number
  total_issues: number
  earliest_event_at: number | null
  verified_through: number | null
  revision: number
  diagnostic: string | null
  resume_at: number | null
}

export interface Star {
  number: number
  parent_issue: number | null
  title: string
  status: Status
  ready_for_agent?: boolean
  blocked?: boolean
  blocked_by: number[]
  milestone: string | null
  labels: string[]
  assignees: string[]
  url: string
  body: string
}

export interface SpaceModel {
  id: string
  repo: string
  name: string
  viewer_login?: string | null
  stars: Star[]
  synced_at: number | null
  stale: boolean
  error: string | null
  history?: HistorySummary
}

export interface Model {
  spaces: SpaceModel[]
}

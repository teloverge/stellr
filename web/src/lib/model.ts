export type Status = 'blocked' | 'frontier' | 'claimed' | 'resolved' | 'out_of_scope'

export interface Star {
  number: number
  parent_issue: number | null
  title: string
  status: Status
  ready_for_agent?: boolean
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
}

export interface Model {
  spaces: SpaceModel[]
}

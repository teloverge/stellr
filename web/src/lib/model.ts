export type Status = 'blocked' | 'frontier' | 'claimed' | 'resolved' | 'out_of_scope'

export interface Star {
  number: number
  title: string
  status: Status
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
  stars: Star[]
  synced_at: number | null
  stale: boolean
  error: string | null
}

export interface Model {
  spaces: SpaceModel[]
}

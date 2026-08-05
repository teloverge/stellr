import type { HistoryEvent } from './history'
import type { HistorySummary } from './model'

export interface HistoryResponse {
  summary: HistorySummary
  events: HistoryEvent[]
}

export async function fetchHistory(spaceId: string, after = 0): Promise<HistoryResponse> {
  const query = after > 0 ? `?after=${after}` : ''
  const response = await fetch(`/api/spaces/${encodeURIComponent(spaceId)}/history${query}`, {
    credentials: 'same-origin',
  })
  if (!response.ok) throw new Error(`History request failed (${response.status})`)
  return (await response.json()) as HistoryResponse
}

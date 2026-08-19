import type { SpaceModel, Star, Status } from './model'

export interface MilestoneRef {
  id: string | null
  title: string
}

interface HistoryEventBase {
  sequence: number
  repository_id: string
  issue_id: string
  issue_number: number
  provider_event_id: string
  occurred_at: number
}

export type HistoryEvent = HistoryEventBase &
  (
    | { kind: 'issue_created'; milestone: MilestoneRef | null }
    | { kind: 'issue_closed' }
    | { kind: 'issue_reopened' }
    | {
        kind: 'milestone_changed'
        from: MilestoneRef | null
        to: MilestoneRef | null
      }
  )

export interface TemporalStar extends Omit<Star, 'status'> {
  status: Status | 'open'
  live_status: Status
  temporal_visible: boolean
}

export interface TemporalSpace extends Omit<SpaceModel, 'stars'> {
  temporal_active: boolean
  stars: TemporalStar[]
}

export function historyEventSummary(event: HistoryEvent, issueWidth = 0): string {
  const issue = `#${event.issue_number.toString().padStart(issueWidth, '0')}`
  if (event.kind === 'issue_created') return `${issue} created`
  if (event.kind === 'issue_closed') return `${issue} closed`
  if (event.kind === 'issue_reopened') return `${issue} reopened`
  if (event.to === null) return `${issue} removed from milestone`
  return `${issue} moved to ${event.to.title}`
}

export function mergeHistoryEvents(
  current: HistoryEvent[],
  incoming: HistoryEvent[],
): HistoryEvent[] {
  const sequences = new Set<number>()
  const providerEvents = new Map<string, HistoryEvent>()
  for (const event of [...current, ...incoming]) {
    const providerKey = `${event.repository_id}\u0000${event.provider_event_id}`
    if (sequences.has(event.sequence)) continue
    sequences.add(event.sequence)
    const previous = providerEvents.get(providerKey)
    if (previous === undefined || event.sequence > previous.sequence) {
      providerEvents.set(providerKey, event)
    }
  }
  return [...providerEvents.values()].toSorted(
    (left, right) =>
      left.occurred_at - right.occurred_at ||
      left.provider_event_id.localeCompare(right.provider_event_id),
  )
}

export function latestHistorySequence(events: HistoryEvent[]): number {
  return events.reduce((latest, event) => Math.max(latest, event.sequence), 0)
}

export function projectTemporalSpace(
  space: SpaceModel,
  events: HistoryEvent[],
  playhead: number | null,
): TemporalSpace {
  if (playhead === null) {
    return {
      ...space,
      temporal_active: false,
      stars: space.stars.map((star) => ({
        ...star,
        live_status: star.status,
        temporal_visible: true,
      })),
    }
  }

  const creationByIssue = new Map<number, Extract<HistoryEvent, { kind: 'issue_created' }>>()
  const statusByIssue = new Map<number, 'open' | 'resolved'>()
  const milestoneByIssue = new Map<number, MilestoneRef | null>()
  const ordered = events
    .filter((event) => event.occurred_at <= playhead)
    .toSorted(
      (left, right) =>
        left.occurred_at - right.occurred_at ||
        left.provider_event_id.localeCompare(right.provider_event_id),
    )
  for (const event of ordered) {
    if (event.kind === 'issue_created') {
      creationByIssue.set(event.issue_number, event)
      statusByIssue.set(event.issue_number, 'open')
      milestoneByIssue.set(event.issue_number, event.milestone)
    } else if (event.kind === 'issue_closed') {
      statusByIssue.set(event.issue_number, 'resolved')
    } else if (event.kind === 'issue_reopened') {
      statusByIssue.set(event.issue_number, 'open')
    } else if (event.kind === 'milestone_changed') {
      milestoneByIssue.set(event.issue_number, event.to)
    }
  }

  return {
    ...space,
    temporal_active: true,
    stars: space.stars.map((star) => {
      const creation = creationByIssue.get(star.number)
      return {
        ...star,
        live_status: star.status,
        status: statusByIssue.get(star.number) ?? 'open',
        milestone: milestoneByIssue.get(star.number)?.title ?? null,
        temporal_visible: creation !== undefined,
      }
    }),
  }
}

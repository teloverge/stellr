import { describe, expect, it } from 'vitest'
import { projectTemporalSpace, type HistoryEvent } from './history'
import type { SpaceModel } from './model'

const space: SpaceModel = {
  id: 'o-r',
  repo: 'o/r',
  name: 'r',
  synced_at: 300,
  stale: false,
  error: null,
  stars: [
    {
      number: 1,
      parent_issue: null,
      title: 'First',
      status: 'resolved',
      blocked_by: [],
      milestone: null,
      labels: [],
      assignees: [],
      url: 'https://example.test/1',
      body: '',
    },
    {
      number: 2,
      parent_issue: null,
      title: 'Second',
      status: 'frontier',
      blocked_by: [1],
      milestone: null,
      labels: ['ready-for-agent'],
      assignees: [],
      url: 'https://example.test/2',
      body: '',
    },
  ],
}

const events: HistoryEvent[] = [
  {
    sequence: 1,
    repository_id: 'R_repo',
    issue_id: 'I_1',
    issue_number: 1,
    provider_event_id: 'I_1:issue_created',
    occurred_at: 100,
    kind: 'issue_created',
    milestone: null,
  },
  {
    sequence: 2,
    repository_id: 'R_repo',
    issue_id: 'I_2',
    issue_number: 2,
    provider_event_id: 'I_2:issue_created',
    occurred_at: 200,
    kind: 'issue_created',
    milestone: null,
  },
]

describe('temporal creation projection', () => {
  it('keeps the complete current snapshot at Now', () => {
    const projected = projectTemporalSpace(space, events, null)

    expect(projected.stars.map((star) => [star.number, star.status, star.temporal_visible])).toEqual([
      [1, 'resolved', true],
      [2, 'frontier', true],
    ])
  })

  it('uses exact creation boundaries and neutral historical-open status', () => {
    expect(
      projectTemporalSpace(space, events, 99).stars.map((star) => star.temporal_visible),
    ).toEqual([false, false])

    expect(
      projectTemporalSpace(space, events, 100).stars.map((star) => [
        star.number,
        star.status,
        star.temporal_visible,
      ]),
    ).toEqual([
      [1, 'open', true],
      [2, 'open', false],
    ])

    expect(
      projectTemporalSpace(space, events, 200).stars.map((star) => star.temporal_visible),
    ).toEqual([true, true])
  })

  it('preserves every present-day star and structural edge while scrubbing', () => {
    const before = projectTemporalSpace(space, events, 99)
    const after = projectTemporalSpace(space, events, 200)

    expect(before.stars.map((star) => star.number)).toEqual([1, 2])
    expect(after.stars.map((star) => star.number)).toEqual([1, 2])
    expect(before.stars.map((star) => star.blocked_by)).toEqual([[], [1]])
    expect(after.stars.map((star) => star.blocked_by)).toEqual([[], [1]])
  })
})

describe('temporal lifecycle projection', () => {
  const lifecycle: HistoryEvent[] = [
    ...events,
    {
      sequence: 3,
      repository_id: 'R_repo',
      issue_id: 'I_1',
      issue_number: 1,
      provider_event_id: 'E_close',
      occurred_at: 150,
      kind: 'issue_closed',
    },
    {
      sequence: 4,
      repository_id: 'R_repo',
      issue_id: 'I_1',
      issue_number: 1,
      provider_event_id: 'E_reopen',
      occurred_at: 175,
      kind: 'issue_reopened',
    },
  ]

  it('applies close and reopen at their exact boundaries', () => {
    expect(projectTemporalSpace(space, lifecycle, 149).stars[0].status).toBe('open')
    expect(projectTemporalSpace(space, lifecycle, 150).stars[0].status).toBe('resolved')
    expect(projectTemporalSpace(space, lifecycle, 174).stars[0].status).toBe('resolved')
    expect(projectTemporalSpace(space, lifecycle, 175).stars[0].status).toBe('open')
  })

  it('orders same-timestamp transitions by provider event identity', () => {
    const tied: HistoryEvent[] = [
      events[0],
      {
        sequence: 2,
        repository_id: 'R_repo',
        issue_id: 'I_1',
        issue_number: 1,
        provider_event_id: 'z-close',
        occurred_at: 150,
        kind: 'issue_closed',
      },
      {
        sequence: 3,
        repository_id: 'R_repo',
        issue_id: 'I_1',
        issue_number: 1,
        provider_event_id: 'a-reopen',
        occurred_at: 150,
        kind: 'issue_reopened',
      },
    ]

    expect(projectTemporalSpace(space, tied, 150).stars[0].status).toBe('resolved')
  })
})

describe('temporal milestone projection', () => {
  const milestoneEvents: HistoryEvent[] = [
    {
      sequence: 1,
      repository_id: 'R_repo',
      issue_id: 'I_1',
      issue_number: 1,
      provider_event_id: 'I_1:issue_created',
      occurred_at: 100,
      kind: 'issue_created',
      milestone: { id: 'M_alpha', title: 'Alpha' },
    },
    {
      sequence: 2,
      repository_id: 'R_repo',
      issue_id: 'I_1',
      issue_number: 1,
      provider_event_id: 'E_assign_beta',
      occurred_at: 125,
      kind: 'milestone_changed',
      from: { id: null, title: 'Alpha' },
      to: { id: null, title: 'Beta' },
    },
    {
      sequence: 3,
      repository_id: 'R_repo',
      issue_id: 'I_1',
      issue_number: 1,
      provider_event_id: 'E_remove_beta',
      occurred_at: 150,
      kind: 'milestone_changed',
      from: { id: null, title: 'Beta' },
      to: null,
    },
  ]

  it('applies creation membership, movement, and removal at exact boundaries', () => {
    expect(projectTemporalSpace(space, milestoneEvents, 99).stars[0].milestone).toBeNull()
    expect(projectTemporalSpace(space, milestoneEvents, 100).stars[0].milestone).toBe('Alpha')
    expect(projectTemporalSpace(space, milestoneEvents, 124).stars[0].milestone).toBe('Alpha')
    expect(projectTemporalSpace(space, milestoneEvents, 125).stars[0].milestone).toBe('Beta')
    expect(projectTemporalSpace(space, milestoneEvents, 149).stars[0].milestone).toBe('Beta')
    expect(projectTemporalSpace(space, milestoneEvents, 150).stars[0].milestone).toBeNull()
  })

  it('orders same-timestamp milestone transitions by provider event identity', () => {
    const tied: HistoryEvent[] = [
      events[0],
      {
        sequence: 2,
        repository_id: 'R_repo',
        issue_id: 'I_1',
        issue_number: 1,
        provider_event_id: 'a-assign',
        occurred_at: 125,
        kind: 'milestone_changed',
        from: null,
        to: { id: null, title: 'Alpha' },
      },
      {
        sequence: 3,
        repository_id: 'R_repo',
        issue_id: 'I_1',
        issue_number: 1,
        provider_event_id: 'z-move',
        occurred_at: 125,
        kind: 'milestone_changed',
        from: { id: null, title: 'Alpha' },
        to: { id: null, title: 'Beta' },
      },
    ]

    expect(projectTemporalSpace(space, tied, 125).stars[0].milestone).toBe('Beta')
  })
})

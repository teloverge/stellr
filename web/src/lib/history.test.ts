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

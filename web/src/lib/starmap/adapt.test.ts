import { describe, expect, it } from 'vitest'
import type { SpaceModel, Status } from '../model'
import { projectTemporalSpace, type HistoryEvent } from '../history'
import { edgesOf } from './layout'
import { toRendererModel } from './adapt'

const statuses: Status[] = ['frontier', 'resolved', 'claimed', 'out_of_scope', 'blocked']

function space(): SpaceModel {
  return {
    id: 'teloverge-stellr',
    repo: 'teloverge/stellr',
    name: 'stellr',
    synced_at: 1_753_000_000,
    stale: false,
    error: null,
    stars: statuses.map((status, index) => {
      const number = index + 1
      return {
        number,
        parent_issue: null,
        title: `Issue ${number}`,
        status,
        blocked_by: status === 'blocked' ? [1] : [],
        milestone: null,
        labels: [],
        assignees: [],
        url: `https://github.com/teloverge/stellr/issues/${number}`,
        body: '',
      }
    }),
  }
}

describe('toRendererModel', () => {
  it('maps every issue status and directs blocker edges toward the blocked issue', () => {
    const input = space()
    input.stars[0].parent_issue = 16

    const model = toRendererModel(input)

    expect(model).toHaveLength(5)
    expect(model.map((node) => node.status)).toEqual(statuses)
    expect(model[0]).toMatchObject({
      num: 1,
      slug: '1',
      title: 'Issue 1',
      status: 'frontier',
      parentIssue: 16,
    })
    expect(model[1].parentIssue).toBeNull()
    expect(edgesOf(model)).toContainEqual({ from: 1, to: 5 })
  })

  it('marks only an unblocked ready-for-agent issue as actionable', () => {
    const input = space()
    input.stars[0].labels = ['READY-FOR-AGENT']
    input.stars[1].status = 'frontier'
    input.stars[4].labels = ['ready-for-agent']

    const model = toRendererModel(input)

    expect(model.map((ticket) => ticket.readyForAgent)).toEqual([
      true,
      false,
      false,
      false,
      false,
    ])
  })

  it('matches viewer ownership case-insensitively without redefining claimed', () => {
    const input = space() as SpaceModel & { viewer_login: string }
    input.viewer_login = 'octocat'
    input.stars[2].assignees = ['OctoCat']
    Object.assign(input.stars[2], { ready_for_agent: true, blocked: true })
    input.stars[3].assignees = ['hubot']
    Object.assign(input.stars[3], { ready_for_agent: true })

    const model = toRendererModel(input)

    expect(model[2]).toMatchObject({
      status: 'claimed',
      assignedToViewer: true,
      readyForAgent: true,
      blocked: true,
    })
    expect(model[3]).toMatchObject({ assignedToViewer: false })
  })

  it('never guesses ownership when viewer identity is unavailable', () => {
    const input = space()
    input.stars[2].assignees = ['previous-account']
    Object.assign(input.stars[2], { ready_for_agent: true })

    const model = toRendererModel(input)

    expect(model[2]).toMatchObject({
      status: 'claimed',
      assignedToViewer: false,
      readyForAgent: true,
    })
  })

  it('passes temporal milestone membership to the renderer', () => {
    const input = space()
    input.stars[0].milestone = 'Launch <script>alert(1)</script>'

    const model = toRendererModel(input)

    expect(model[0].milestone).toBe('Launch <script>alert(1)</script>')
  })

  it('removes live workflow emphasis while preserving current dependency context', () => {
    const input = space()
    input.stars[0].labels = ['ready-for-agent']
    const events: HistoryEvent[] = input.stars.map((star, index) => ({
      sequence: index + 1,
      repository_id: 'R_repo',
      issue_id: `I_${star.number}`,
      issue_number: star.number,
      provider_event_id: `I_${star.number}:issue_created`,
      occurred_at: 100,
      kind: 'issue_created',
      milestone: null,
    }))

    const model = toRendererModel(projectTemporalSpace(input, events, 100))

    expect(model[0]).toMatchObject({
      status: 'open',
      frontier: false,
      readyForAgent: false,
      focusStatus: 'open',
      historical: true,
    })
    expect(edgesOf(model)).toContainEqual({ from: 1, to: 5 })
  })
})

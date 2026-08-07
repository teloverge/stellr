import { describe, expect, it } from 'vitest'
import type { SpaceModel, Status } from '../model'
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
    input.stars[0].labels = ['IN-PROGRESS']
    input.stars[1].labels = ['in-progress']
    input.stars[1].assignees = ['ada']
    input.stars[3].labels = ['ready-for-agent']
    input.stars[4].labels = ['ready-for-agent']

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
    expect(model.map((ticket) => ticket.workPriority)).toEqual([
      'in_progress',
      'terminal',
      'in_progress',
      'terminal',
      'blocked',
    ])
    expect(model.map((ticket) => ticket.readyForAgent)).toEqual([
      false,
      false,
      false,
      false,
      false,
    ])
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
    expect(model.map((ticket) => ticket.workPriority)).toEqual([
      'ready',
      'frontier',
      'in_progress',
      'terminal',
      'blocked',
    ])
  })
})

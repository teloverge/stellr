import { describe, expect, it } from 'vitest'
import { deriveWorkPriority } from './priority'
import type { Ticket } from './model'

function ticket(overrides: Partial<Ticket> = {}): Ticket {
  return {
    num: 1,
    slug: '1',
    title: 'Work',
    type: 'issue',
    status: 'frontier',
    blockedBy: [],
    parentIssue: null,
    frontier: true,
    readyForAgent: false,
    assignedToViewer: false,
    ...overrides,
  }
}

describe('work priority hierarchy', () => {
  it('puts session trouble ahead of current and implementing work', () => {
    const t = ticket({ num: 7, status: 'claimed', assignedToViewer: true })
    expect(deriveWorkPriority(t, 'blocked', 7)).toBe('attention')
    expect(deriveWorkPriority(t, 'dead', 7)).toBe('attention')
  })

  it('treats current or actively implementing work as doing now', () => {
    expect(deriveWorkPriority(ticket({ num: 7 }), null, 7)).toBe('doing_now')
    expect(deriveWorkPriority(ticket(), 'implementing', null)).toBe('doing_now')
  })

  it('orders owned, available, and team work by the viewer hierarchy', () => {
    expect(
      deriveWorkPriority(
        ticket({ status: 'claimed', assignedToViewer: true, readyForAgent: true }),
        null,
        null,
      ),
    ).toBe('my_next')
    expect(
      deriveWorkPriority(
        ticket({ status: 'claimed', assignedToViewer: true, blocked: true }),
        null,
        null,
      ),
    ).toBe('my_future')
    expect(deriveWorkPriority(ticket({ readyForAgent: true }), null, null)).toBe(
      'available_next',
    )
    expect(deriveWorkPriority(ticket({ status: 'claimed' }), null, null)).toBe('team_work')
    expect(
      deriveWorkPriority(ticket({ status: 'claimed', readyForAgent: true }), null, null),
    ).toBe('team_work')
  })

  it('keeps remaining open work quiet and separates both closure reasons', () => {
    expect(deriveWorkPriority(ticket({ status: 'blocked' }), null, null)).toBe('planning')
    expect(deriveWorkPriority(ticket({ status: 'resolved' }), null, null)).toBe('resolved')
    expect(deriveWorkPriority(ticket({ status: 'out_of_scope' }), null, null)).toBe(
      'out_of_scope',
    )
  })

  it('does not promote a ticket merely because the user selected it', () => {
    const selected = ticket({ status: 'blocked' })
    expect(deriveWorkPriority(selected, null, null)).toBe('planning')
  })
})

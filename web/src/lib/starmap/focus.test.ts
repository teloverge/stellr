import { describe, expect, it } from 'vitest'
import { analyzeFocus } from './focus'
import type { Ticket, TicketStatus } from './model'

function ticket(
  num: number,
  status: TicketStatus,
  blockedBy: number[] = [],
  readyForAgent = false,
): Ticket {
  return {
    num,
    slug: String(num),
    title: `Issue ${num}`,
    type: 'issue',
    status,
    blockedBy,
    frontier: status === 'frontier',
    readyForAgent,
  }
}

describe('session focus analysis', () => {
  it('prioritizes an actionable blocker path before unrelated ready work', () => {
    const focus = analyzeFocus(
      [
        ticket(8, 'frontier', [], true),
        ticket(12, 'blocked', [8]),
        ticket(14, 'blocked', [12]),
        ticket(21, 'frontier', [], true),
      ],
      14,
    )

    expect(focus.current).toBe(14)
    expect(focus.ready).toEqual([8, 21])
    expect([...focus.pathNodes]).toEqual([8, 12, 14])
    expect([...focus.pathEdges]).toEqual(['8>12', '12>14'])
    expect([...focus.emphasized]).toEqual([14, 8, 21, 12])
  })

  it('uses every actionable issue when no current issue is supplied', () => {
    const focus = analyzeFocus(
      [ticket(21, 'frontier', [], true), ticket(8, 'frontier', [], true)],
      null,
    )

    expect(focus.current).toBe(null)
    expect(focus.ready).toEqual([8, 21])
    expect([...focus.pathNodes]).toEqual([])
    expect([...focus.emphasized]).toEqual([8, 21])
  })

  it('marks an actionable current issue as both current and ready', () => {
    const focus = analyzeFocus([ticket(8, 'frontier', [], true)], 8)

    expect(focus.current).toBe(8)
    expect(focus.ready).toEqual([8])
    expect([...focus.pathNodes]).toEqual([8])
    expect([...focus.emphasized]).toEqual([8])
  })

  it('falls back to global ready work when the requested current issue is absent', () => {
    const focus = analyzeFocus([ticket(8, 'frontier', [], true)], 14)

    expect(focus.current).toBe(null)
    expect(focus.ready).toEqual([8])
    expect([...focus.pathEdges]).toEqual([])
    expect([...focus.emphasized]).toEqual([8])
  })

  it('does not route actionable work through a resolved intermediate issue', () => {
    const focus = analyzeFocus(
      [
        ticket(8, 'frontier', [], true),
        ticket(12, 'resolved', [8]),
        ticket(14, 'blocked', [12]),
      ],
      14,
    )

    expect(focus.ready).toEqual([8])
    expect([...focus.pathNodes]).toEqual([])
    expect([...focus.pathEdges]).toEqual([])
    expect([...focus.emphasized]).toEqual([14, 8])
  })

  it('terminates cycles and retains a real route to the current issue', () => {
    const focus = analyzeFocus(
      [
        ticket(1, 'frontier', [], true),
        ticket(2, 'blocked', [1, 3]),
        ticket(3, 'blocked', [2]),
      ],
      3,
    )

    expect(focus.ready).toEqual([1])
    expect([...focus.pathNodes]).toEqual([1, 2, 3])
    expect([...focus.pathEdges]).toEqual(['1>2', '2>3'])
  })
})

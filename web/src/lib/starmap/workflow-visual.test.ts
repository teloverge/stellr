import { describe, expect, it } from 'vitest'
import type { Ticket } from './model'
import type { WorkflowEdge } from './workflow'
import { curveSide, reverseEdgeKeys, workflowVisualState } from './workflow-visual'

function ticket(num: number, status: Ticket['status']): Ticket {
  return {
    num,
    slug: String(num),
    title: `Ticket ${num}`,
    type: 'task',
    status,
    blockedBy: [],
    parentIssue: null,
    frontier: status === 'frontier',
  }
}

describe('workflow visual policy', () => {
  it('derives edge state with traversed taking precedence over child completion', () => {
    const tickets = new Map([
      [1, ticket(1, 'open')],
      [2, ticket(2, 'open')],
      [3, ticket(3, 'resolved')],
      [4, ticket(4, 'frontier')],
      [5, ticket(5, 'blocked')],
    ])
    const entryToOpenChild: WorkflowEdge = { from: 1, to: 2, roles: ['entry'], child: 2 }
    const returnFromResolvedChild: WorkflowEdge = { from: 3, to: 1, roles: ['return'], child: 3 }
    const sequenceFromResolvedToFrontier: WorkflowEdge = {
      from: 3,
      to: 4,
      roles: ['dependency', 'sequence'],
      child: 4,
    }
    const sequenceFromResolvedToBlocked: WorkflowEdge = {
      from: 3,
      to: 5,
      roles: ['dependency', 'sequence'],
      child: 5,
    }

    expect(workflowVisualState(entryToOpenChild, tickets)).toBe('incomplete')
    expect(workflowVisualState(returnFromResolvedChild, tickets)).toBe('traversed')
    expect(workflowVisualState(sequenceFromResolvedToFrontier, tickets)).toBe('traversed')
    expect(workflowVisualState(sequenceFromResolvedToBlocked, tickets)).toBe('traversed')
    expect(
      workflowVisualState(
        { from: 3, to: 1, roles: ['dependency'], child: null },
        tickets,
      ),
    ).toBe('traversed')
  })

  it('bows reverse directions apart and gives single edges a stable default side', () => {
    const entry: WorkflowEdge = { from: 16, to: 37, roles: ['entry'], child: 37 }
    const returned: WorkflowEdge = { from: 37, to: 16, roles: ['return'], child: 37 }

    expect(curveSide(entry, true)).toBe(1)
    expect(curveSide(returned, true)).toBe(-1)
    expect(curveSide({ from: 37, to: 38, roles: ['sequence'], child: 38 }, false)).toBe(1)
  })

  it('indexes reverse partners once for constant-time render lookup', () => {
    const edges: WorkflowEdge[] = [
      { from: 16, to: 37, roles: ['entry'], child: 37 },
      { from: 37, to: 16, roles: ['return'], child: 37 },
      { from: 37, to: 38, roles: ['sequence'], child: 38 },
    ]
    expect(reverseEdgeKeys(edges)).toEqual(new Set(['16>37', '37>16']))
  })
})

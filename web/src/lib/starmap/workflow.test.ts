import { describe, expect, it } from 'vitest'
import { workflowEdges, type WorkflowNode } from './workflow'

function node(num: number, blockedBy: number[] = [], parentIssue: number | null = null): WorkflowNode {
  return { num, blockedBy, parentIssue }
}

describe('workflowEdges', () => {
  it('creates entry and return edges for independent children', () => {
    const independentChildren = [node(14), node(34, [], 14), node(35, [], 14)]

    expect(workflowEdges(independentChildren)).toEqual([
      { from: 14, to: 34, roles: ['entry'], child: 34 },
      { from: 14, to: 35, roles: ['entry'], child: 35 },
      { from: 34, to: 14, roles: ['return'], child: 34 },
      { from: 35, to: 14, roles: ['return'], child: 35 },
    ])
  })

  it('marks sibling dependencies as a sequence', () => {
    const sequentialChildren = [node(16), node(37, [], 16), node(38, [37], 16)]

    expect(workflowEdges(sequentialChildren)).toEqual([
      { from: 16, to: 37, roles: ['entry'], child: 37 },
      { from: 37, to: 38, roles: ['dependency', 'sequence'], child: 38 },
      { from: 38, to: 16, roles: ['return'], child: 38 },
    ])
  })

  it('closes a lone child loop', () => {
    expect(workflowEdges([node(16), node(37, [], 16)])).toEqual([
      { from: 16, to: 37, roles: ['entry'], child: 37 },
      { from: 37, to: 16, roles: ['return'], child: 37 },
    ])
  })

  it('handles nested parent groups independently', () => {
    expect(workflowEdges([node(10), node(20, [], 10), node(21, [], 20)])).toEqual([
      { from: 10, to: 20, roles: ['entry'], child: 20 },
      { from: 20, to: 10, roles: ['return'], child: 20 },
      { from: 20, to: 21, roles: ['entry'], child: 21 },
      { from: 21, to: 20, roles: ['return'], child: 21 },
    ])
  })

  it('ignores self and missing parents without dropping ordinary dependencies', () => {
    expect(workflowEdges([node(1), node(2, [1], 2), node(3, [1], 99)])).toEqual([
      { from: 1, to: 2, roles: ['dependency'], child: null },
      { from: 1, to: 3, roles: ['dependency'], child: null },
    ])
  })

  it('sorts edges and merges duplicate roles while retaining the child identity', () => {
    expect(workflowEdges([node(38, [16, 16], 16), node(16)])).toEqual([
      { from: 16, to: 38, roles: ['dependency', 'entry'], child: 38 },
      { from: 38, to: 16, roles: ['return'], child: 38 },
    ])
  })
})

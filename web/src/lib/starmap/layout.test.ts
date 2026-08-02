import { describe, expect, it } from 'vitest'
import { computeLayout, rankOf, structureSignature, type LayoutNode } from './layout'

const workflowCycle: LayoutNode[] = [
  { num: 16, blockedBy: [], parentIssue: 38 },
  { num: 37, blockedBy: [], parentIssue: 16 },
  { num: 38, blockedBy: [37], parentIssue: 37 },
]

describe('workflow-aware layout', () => {
  it('keeps dependency rank blocker-only through a workflow cycle', () => {
    expect(rankOf(workflowCycle)).toEqual({ 16: 0, 37: 0, 38: 1 })
  })

  it('uses parent workflow edges for springs and structural identity, not status', () => {
    const withoutParent: LayoutNode[] = [
      { num: 16, blockedBy: [], parentIssue: null },
      { num: 37, blockedBy: [16], parentIssue: null },
    ]
    const withParent: LayoutNode[] = [
      { num: 16, blockedBy: [], parentIssue: null },
      { num: 37, blockedBy: [16], parentIssue: 16 },
    ]
    const statuses = withParent.map((node) => ({ ...node, status: 'resolved' }))

    expect(computeLayout(withParent)).not.toEqual(computeLayout(withoutParent))
    expect(structureSignature(withParent)).not.toBe(structureSignature(withoutParent))
    expect(structureSignature(statuses)).toBe(structureSignature(withParent))
  })
})

import { describe, expect, it } from 'vitest'
import { deriveWorkPriority } from './work-priority'

describe('deriveWorkPriority', () => {
  it.each([
    ['resolved overrides stale labels', 'resolved', ['in-progress'], ['ada'], 'terminal'],
    ['out of scope overrides stale labels', 'out_of_scope', ['ready-for-agent'], [], 'terminal'],
    ['literal in progress outranks a blocker', 'blocked', ['IN-PROGRESS'], [], 'in_progress'],
    ['claimed is the assigned fallback', 'claimed', [], ['ada'], 'in_progress'],
    ['an assignee is the assigned fallback', 'frontier', [], ['ada'], 'in_progress'],
    ['ready requires an unblocked frontier', 'frontier', ['READY-FOR-AGENT'], [], 'ready'],
    ['ready cannot override blocked', 'blocked', ['ready-for-agent'], [], 'blocked'],
    ['unlabelled unblocked work stays frontier', 'frontier', [], [], 'frontier'],
  ] as const)('%s', (_name, status, labels, assignees, expected) => {
    expect(deriveWorkPriority({ status, labels, assignees })).toBe(expected)
  })
})

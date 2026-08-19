import { describe, expect, it } from 'vitest'
import { priorityLabelColor, priorityStarStyle } from './theme'

describe('priority visual theme', () => {
  it('orders active work without changing its semantic palette', () => {
    expect(priorityStarStyle('blocked', 'in_progress')).toMatchObject({
      core: '#ffd873',
      r: 8.1,
      gr: 42,
    })
    expect(priorityStarStyle('frontier', 'ready')).toMatchObject({
      core: '#8ad8ff',
      r: 7.2,
      gr: 34,
    })
    expect(priorityStarStyle('frontier', 'frontier')).toMatchObject({
      core: '#8ad8ff',
      r: 6.2,
      gr: 28,
    })
    expect(priorityStarStyle('blocked', 'blocked')).toMatchObject({
      core: '#e2c3c3',
      r: 4.5,
      gr: 20,
    })
    expect(priorityStarStyle('resolved', 'terminal')).toMatchObject({
      core: '#b9d6c4',
      r: 5.4,
      gr: 24,
    })
    expect(priorityStarStyle('out_of_scope', 'terminal')).toMatchObject({
      core: '#948da4',
      r: 4.5,
      gr: 18,
    })
    expect(priorityLabelColor('blocked', 'in_progress')).toBe('#ffe6a0')
    expect(priorityLabelColor('frontier', 'ready')).toBe('#b3e5ff')
  })
})

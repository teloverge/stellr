import { afterEach, describe, expect, it, vi } from 'vitest'
import { flushSync, mount, unmount, type ComponentProps } from 'svelte'
import TemporalTimeline from './TemporalTimeline.svelte'
import type { HistoryEvent } from './history'
import type { HistorySummary } from './model'

const mounted: object[] = []

afterEach(async () => {
  for (const component of mounted.splice(0)) await unmount(component)
  document.body.innerHTML = ''
})

const complete: HistorySummary = {
  state: 'complete',
  completed_issues: 2,
  total_issues: 2,
  earliest_event_at: 100,
  verified_through: 200,
  revision: 2,
  diagnostic: null,
  resume_at: null,
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
]

function render(props: ComponentProps<typeof TemporalTimeline>) {
  const target = document.createElement('div')
  document.body.appendChild(target)
  const component = mount(TemporalTimeline, { target, props })
  mounted.push(component)
  flushSync()
  return target
}

describe('TemporalTimeline creation scrubber', () => {
  it('stays visible but disabled with determinate import progress', () => {
    const target = render({
      summary: { ...complete, state: 'building', completed_issues: 3, total_issues: 10 },
      events: [],
      playhead: null,
    })

    expect(target.querySelector('input[type="range"]')).toHaveProperty('disabled', true)
    expect(target.textContent).toContain('Building history · 3/10 issues')
  })

  it('reports an empty complete ledger without enabling a slider', () => {
    const target = render({
      summary: { ...complete, earliest_event_at: null, revision: 0 },
      events: [],
      playhead: null,
    })

    expect(target.textContent).toContain('No issue history')
    expect(target.querySelector('input[type="range"]')).toHaveProperty('disabled', true)
  })

  it('defaults to Now and emits local scrub and return-to-now changes', () => {
    const change = vi.fn()
    const target = render({ summary: complete, events, playhead: null, change })
    const slider = target.querySelector<HTMLInputElement>('input[type="range"]')!

    expect(target.querySelector('output')?.textContent).toBe('Now')
    expect(slider.value).toBe('200')

    slider.value = '100'
    slider.dispatchEvent(new Event('input', { bubbles: true }))
    flushSync()
    expect(change).toHaveBeenCalledWith(100)

    const pastTarget = render({ summary: complete, events, playhead: 100, change })
    pastTarget.querySelector<HTMLButtonElement>('button')!.click()
    expect(change).toHaveBeenLastCalledWith(null)
  })
})

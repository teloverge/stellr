import { afterEach, describe, expect, it, vi } from 'vitest'
import { flushSync, mount, unmount, type ComponentProps } from 'svelte'
import TemporalTimeline from './TemporalTimeline.svelte'
import type { HistoryEvent } from './history'
import type { HistorySummary } from './model'

const mounted: object[] = []

afterEach(async () => {
  for (const component of mounted.splice(0)) await unmount(component)
  document.body.innerHTML = ''
  vi.restoreAllMocks()
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
  {
    sequence: 2,
    repository_id: 'R_repo',
    issue_id: 'I_2',
    issue_number: 2,
    provider_event_id: 'E_close',
    occurred_at: 150,
    kind: 'issue_closed',
  },
  {
    sequence: 3,
    repository_id: 'R_repo',
    issue_id: 'I_2',
    issue_number: 2,
    provider_event_id: 'E_reopen',
    occurred_at: 150,
    kind: 'issue_reopened',
  },
  {
    sequence: 4,
    repository_id: 'R_repo',
    issue_id: 'I_1',
    issue_number: 1,
    provider_event_id: 'E_milestone',
    occurred_at: 151,
    kind: 'milestone_changed',
    from: null,
    to: { id: null, title: 'M2 <safe>' },
  },
  {
    sequence: 5,
    repository_id: 'R_repo',
    issue_id: 'I_2',
    issue_number: 2,
    provider_event_id: 'E_last',
    occurred_at: 200,
    kind: 'issue_closed',
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
      summary: {
        ...complete,
        state: 'building',
        completed_issues: 3,
        total_issues: 10,
        verified_through: null,
      },
      events: [],
      playhead: null,
    })

    expect(target.querySelector('input[type="range"]')).toHaveProperty('disabled', true)
    expect(target.textContent).toContain('Building history · 3/10 issues')
  })

  it('keeps verified local history usable while a delta catch-up is building', () => {
    const target = render({
      summary: { ...complete, state: 'building', completed_issues: 1, total_issues: 2 },
      events,
      playhead: 125,
    })

    expect(target.querySelector('input[type="range"]')).toHaveProperty('disabled', false)
    expect(target.textContent).toContain('Updating history · 1/2 issues')
    expect(target.textContent).toContain('History through')
  })

  it('reports rate-limit reset evidence without disabling verified playback', () => {
    const target = render({
      summary: {
        ...complete,
        state: 'rate_limited',
        diagnostic: 'GitHub rate limit exceeded',
        resume_at: 300,
      },
      events,
      playhead: null,
    })

    const status = target.querySelector('[role="status"]')
    expect(status?.textContent).toContain('GitHub rate limit exceeded')
    expect(status?.textContent).toContain('Retry after')
    expect(status?.textContent).toContain('History through')
    expect(target.querySelector('input[type="range"]')).toHaveProperty('disabled', false)
  })

  it('keeps persistent accessible names across enabled and unavailable states', () => {
    const enabled = render({ summary: complete, events, playhead: null })
    expect(enabled.querySelector('input[type="range"]')?.getAttribute('aria-label')).toBe(
      'Issue history date',
    )
    expect(enabled.querySelector('[data-control="play"]')?.getAttribute('aria-label')).toBe(
      'Play issue history',
    )
    expect(enabled.querySelector('[data-control="speed"]')?.getAttribute('aria-label')).toContain(
      'Playback speed',
    )

    const unavailable = render({
      summary: {
        ...complete,
        state: 'failed',
        earliest_event_at: null,
        verified_through: null,
        diagnostic: 'History unavailable offline',
      },
      events: [],
      playhead: null,
    })
    expect(unavailable.querySelector('input[type="range"]')?.getAttribute('aria-label')).toBe(
      'Issue history date',
    )
    expect(unavailable.querySelector('[data-control="play"]')?.getAttribute('aria-label')).toBe(
      'Play issue history',
    )
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

  it('defaults to Now and emits local scrub changes', () => {
    const change = vi.fn()
    const target = render({ summary: complete, events, playhead: null, change })
    const slider = target.querySelector<HTMLInputElement>('input[type="range"]')!

    expect(target.querySelector('output')?.textContent).toBe('Now')
    expect(slider.value).toBe('200')

    slider.value = '100'
    slider.dispatchEvent(new Event('input', { bubbles: true }))
    flushSync()
    expect(change).toHaveBeenCalledWith(100)
  })

  it('orders date, slider ticks, Play, and speed controls accessibly', () => {
    const target = render({ summary: complete, events, playhead: null })
    const controls = [...target.querySelectorAll<HTMLElement>('[data-control]')]

    expect(controls.map((control) => control.dataset.control)).toEqual([
      'date',
      'track',
      'play',
      'speed',
    ])
    expect(target.querySelector('input[type="range"]')?.getAttribute('aria-valuetext')).toBe('Now')
    expect(target.querySelector('[data-control="play"]')?.textContent).toContain('Play')
    expect(target.querySelector('[data-control="speed"]')?.textContent).toContain('1×')
  })

  it('renders proportional ticks, clusters dense activity, and exposes ordered tooltip text', () => {
    const target = render({ summary: complete, events, playhead: null })
    const ticks = [...target.querySelectorAll<HTMLButtonElement>('.event-tick')]

    expect(ticks).toHaveLength(3)
    expect(ticks[0].style.left).toBe('0%')
    expect(ticks.at(-1)?.style.left).toBe('100%')
    expect(ticks[1].dataset.eventCount).toBe('3')

    ticks[1].focus()
    ticks[1].dispatchEvent(new FocusEvent('focus', { bubbles: true }))
    flushSync()
    const tooltip = target.querySelector('[role="tooltip"]')
    expect(tooltip?.textContent).toContain('#2 closed')
    expect(tooltip?.textContent).toContain('#2 reopened')
    expect(tooltip?.textContent).toContain('#1 moved to M2 <safe>')
    expect(tooltip?.querySelector('script')).toBeNull()
  })

  it('navigates exact event times with ticks and slider keys', () => {
    const change = vi.fn()
    const target = render({ summary: complete, events, playhead: 150, change })
    const slider = target.querySelector<HTMLInputElement>('input[type="range"]')!
    const ticks = [...target.querySelectorAll<HTMLButtonElement>('.event-tick')]

    ticks[0].click()
    expect(change).toHaveBeenLastCalledWith(100)

    slider.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true }))
    expect(change).toHaveBeenLastCalledWith(151)
    slider.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowLeft', bubbles: true }))
    expect(change).toHaveBeenLastCalledWith(100)
    slider.dispatchEvent(new KeyboardEvent('keydown', { key: 'Home', bubbles: true }))
    expect(change).toHaveBeenLastCalledWith(100)
    slider.dispatchEvent(new KeyboardEvent('keydown', { key: 'End', bubbles: true }))
    expect(change).toHaveBeenLastCalledWith(null)
  })

  it('plays from the beginning at Now, emits crossed events, pauses, and cycles speed', () => {
    const frames: FrameRequestCallback[] = []
    vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
      frames.push(callback)
      return frames.length
    })
    vi.stubGlobal('cancelAnimationFrame', vi.fn())
    vi.spyOn(performance, 'now').mockReturnValue(1_000)
    const change = vi.fn()
    const reached = vi.fn()
    const target = render({ summary: complete, events, playhead: null, change, reached })
    const play = target.querySelector<HTMLButtonElement>('[data-control="play"]')!
    const speed = target.querySelector<HTMLButtonElement>('[data-control="speed"]')!

    play.click()
    flushSync()
    expect(change).toHaveBeenLastCalledWith(100)
    expect(reached.mock.calls[0][0].map((event: HistoryEvent) => event.provider_event_id)).toEqual([
      'I_1:issue_created',
    ])
    expect(play.textContent).toContain('Pause')

    speed.click()
    flushSync()
    expect(speed.textContent).toContain('2×')
    play.click()
    flushSync()
    expect(play.textContent).toContain('Play')
  })

  it('offers new activity without moving a historical playhead', () => {
    const returnToNow = vi.fn()
    const target = render({
      summary: { ...complete, revision: 6 },
      events,
      playhead: 125,
      newActivity: true,
      returnToNow,
    })

    expect(target.querySelector<HTMLInputElement>('input[type="range"]')?.value).toBe('125')
    target.querySelector<HTMLButtonElement>('.new-activity')!.click()
    expect(returnToNow).toHaveBeenCalledOnce()
  })
})

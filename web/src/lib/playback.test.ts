import { describe, expect, it } from 'vitest'
import { PlaybackClock, clusterEventTicks, type PlaybackSpeed } from './playback'
import type { HistoryEvent } from './history'

function created(id: string, issue: number, occurredAt: number): HistoryEvent {
  return {
    sequence: issue,
    repository_id: 'R_repo',
    issue_id: `I_${issue}`,
    issue_number: issue,
    provider_event_id: id,
    occurred_at: occurredAt,
    kind: 'issue_created',
    milestone: null,
  }
}

const events = [
  created('E_first', 1, 100),
  created('E_tied_b', 2, 150),
  created('E_tied_a', 3, 150),
  created('E_last', 4, 200),
]

describe('PlaybackClock', () => {
  it.each([
    [0.5, 60_000],
    [1, 30_000],
    [2, 15_000],
    [4, 7_500],
  ] as Array<[PlaybackSpeed, number]>)('maps the full range at %sx to %sms', (speed, duration) => {
    const clock = new PlaybackClock(100, 200, events, speed)
    const started = clock.play(null, 1_000)

    expect(started.playhead).toBe(100)
    expect(started.crossed.map((event) => event.provider_event_id)).toEqual(['E_first'])
    expect(clock.tick(1_000 + duration - 1).playing).toBe(true)
    expect(clock.tick(1_000 + duration)).toMatchObject({ playhead: null, playing: false })
  })

  it('continues from the current absolute playhead and pauses at Now', () => {
    const clock = new PlaybackClock(100, 200, events)
    expect(clock.play(150, 0).playhead).toBe(150)
    expect(clock.tick(7_500).playhead).toBe(175)
    expect(clock.tick(15_000)).toMatchObject({ playhead: null, playing: false })
  })

  it('restarts at the first event when Play is pressed at Now', () => {
    const clock = new PlaybackClock(100, 200, events)
    clock.play(150, 0)
    clock.pause()

    expect(clock.play(null, 10_000)).toMatchObject({ playhead: 100, playing: true })
  })

  it('returns every crossed event in stable atomic order after a slow frame', () => {
    const clock = new PlaybackClock(100, 200, events)
    clock.play(100, 0)

    const frame = clock.tick(20_000)

    expect(frame.playhead).toBeCloseTo(166.666, 2)
    expect(frame.crossed.map((event) => event.provider_event_id)).toEqual([
      'E_tied_a',
      'E_tied_b',
    ])
  })

  it('changes speed without losing elapsed time at the transition', () => {
    const clock = new PlaybackClock(100, 200, events)
    clock.play(100, 0)

    const transition = clock.setSpeed(2, 7_500)
    expect(transition.playhead).toBe(125)
    expect(clock.tick(15_000).playhead).toBe(175)
  })
})

describe('event tick clustering', () => {
  it('keeps proportional positions, groups exact ties, and clusters dense pixels', () => {
    const clusters = clusterEventTicks(
      [
        ...events,
        created('E_dense_1', 5, 151),
        created('E_dense_2', 6, 152),
        created('E_dense_3', 7, 153),
      ],
      100,
      200,
      500,
      8,
    )

    expect(clusters[0].position).toBe(0)
    expect(clusters.at(-1)?.position).toBe(1)
    expect(clusters[1].times).toEqual([150, 151, 152, 153])
    expect(clusters[1].events.map((event) => event.provider_event_id)).toEqual([
      'E_tied_a',
      'E_tied_b',
      'E_dense_1',
      'E_dense_2',
      'E_dense_3',
    ])
  })
})

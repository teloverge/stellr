import type { HistoryEvent } from './history'

export const PLAYBACK_SPEEDS = [0.5, 1, 2, 4] as const
export type PlaybackSpeed = (typeof PLAYBACK_SPEEDS)[number]

export interface PlaybackFrame {
  playhead: number | null
  playing: boolean
  crossed: HistoryEvent[]
}

export interface EventTickCluster {
  position: number
  times: number[]
  events: HistoryEvent[]
}

const FULL_HISTORY_DURATION_MS = 30_000

export class PlaybackClock {
  readonly #start: number
  readonly #end: number
  readonly #events: HistoryEvent[]
  #speed: PlaybackSpeed
  #playhead: number
  #lastAt = 0
  #playing = false

  constructor(
    start: number,
    end: number,
    events: HistoryEvent[],
    speed: PlaybackSpeed = 1,
  ) {
    this.#start = Math.min(start, end)
    this.#end = Math.max(start, end)
    this.#playhead = this.#start
    this.#events = [...events].sort(compareEvents)
    this.#speed = speed
  }

  play(playhead: number | null, monotonicNow: number): PlaybackFrame {
    this.#playhead =
      playhead === null || playhead >= this.#end
        ? this.#start
        : clamp(playhead, this.#start, this.#end)
    this.#lastAt = monotonicNow
    this.#playing = this.#end > this.#start
    return {
      playhead: this.#playing ? this.#playhead : null,
      playing: this.#playing,
      crossed: this.#events.filter((event) => event.occurred_at === this.#playhead),
    }
  }

  pause(): PlaybackFrame {
    this.#playing = false
    return { playhead: this.#playhead, playing: false, crossed: [] }
  }

  tick(monotonicNow: number): PlaybackFrame {
    if (!this.#playing) {
      return { playhead: this.#playhead, playing: false, crossed: [] }
    }

    const elapsed = Math.max(0, monotonicNow - this.#lastAt)
    this.#lastAt = monotonicNow
    const previous = this.#playhead
    const calendarPerMillisecond = (this.#end - this.#start) / FULL_HISTORY_DURATION_MS
    this.#playhead = Math.min(
      this.#end,
      previous + elapsed * calendarPerMillisecond * this.#speed,
    )
    const crossed = this.#events.filter(
      (event) => event.occurred_at > previous && event.occurred_at <= this.#playhead,
    )

    if (this.#playhead >= this.#end) {
      this.#playing = false
      return { playhead: null, playing: false, crossed }
    }
    return { playhead: this.#playhead, playing: true, crossed }
  }

  setSpeed(speed: PlaybackSpeed, monotonicNow: number): PlaybackFrame {
    const frame = this.tick(monotonicNow)
    this.#speed = speed
    return frame
  }
}

export function distinctEventTimes(events: HistoryEvent[]): number[] {
  return [...new Set(events.map((event) => event.occurred_at))].sort((a, b) => a - b)
}

export function clusterEventTicks(
  events: HistoryEvent[],
  start: number,
  end: number,
  width: number,
  clusterDistance = 10,
): EventTickCluster[] {
  if (events.length === 0) return []
  const span = Math.max(1, end - start)
  const byTime = new Map<number, HistoryEvent[]>()
  for (const event of [...events].sort(compareEvents)) {
    const group = byTime.get(event.occurred_at) ?? []
    group.push(event)
    byTime.set(event.occurred_at, group)
  }

  const clusters: EventTickCluster[] = []
  let previousTickPosition: number | null = null
  for (const [time, groupedEvents] of [...byTime].sort(([a], [b]) => a - b)) {
    const position = clamp((time - start) / span, 0, 1)
    const previous = clusters.at(-1)
    if (
      previous &&
      previousTickPosition !== null &&
      (position - previousTickPosition) * Math.max(1, width) < clusterDistance
    ) {
      const count = previous.times.length
      previous.position = (previous.position * count + position) / (count + 1)
      previous.times.push(time)
      previous.events.push(...groupedEvents)
    } else {
      clusters.push({ position, times: [time], events: [...groupedEvents] })
    }
    previousTickPosition = position
  }
  return clusters
}

export function compareEvents(left: HistoryEvent, right: HistoryEvent): number {
  return (
    left.occurred_at - right.occurred_at ||
    left.provider_event_id.localeCompare(right.provider_event_id)
  )
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value))
}

<script lang="ts">
  import { onDestroy, onMount } from 'svelte'
  import { historyEventSummary, type HistoryEvent } from './history'
  import type { HistorySummary } from './model'
  import {
    PLAYBACK_SPEEDS,
    PlaybackClock,
    clusterEventTicks,
    distinctEventTimes,
    type EventTickCluster,
    type PlaybackFrame,
    type PlaybackSpeed,
  } from './playback'

  let {
    summary,
    events,
    playhead = null,
    change = () => {},
    reached = () => {},
    newActivity = false,
    returnToNow = () => {},
  }: {
    summary: HistorySummary
    events: HistoryEvent[]
    playhead?: number | null
    change?: (playhead: number | null) => void
    reached?: (events: HistoryEvent[]) => void
    newActivity?: boolean
    returnToNow?: () => void
  } = $props()

  let speed = $state<PlaybackSpeed>(1)
  let playing = $state(false)
  let track: HTMLDivElement
  let trackWidth = $state(600)
  let tooltip = $state<EventTickCluster | null>(null)
  let clock: PlaybackClock | null = null
  let frameHandle = 0

  const hasHistory = $derived(
    summary.earliest_event_at !== null &&
      summary.verified_through !== null &&
      events.length > 0,
  )
  const start = $derived(summary.earliest_event_at ?? 0)
  const end = $derived(summary.verified_through ?? 0)
  const value = $derived(playhead ?? end)
  const label = $derived(playhead === null ? 'Now' : formatDate(playhead))
  const eventTimes = $derived(distinctEventTimes(events))
  const ticks = $derived(clusterEventTicks(events, start, end, trackWidth))

  function formatDate(epoch: number): string {
    return new Intl.DateTimeFormat(undefined, {
      year: 'numeric',
      month: 'short',
      day: 'numeric',
      hour: 'numeric',
      minute: '2-digit',
    }).format(new Date(epoch * 1_000))
  }

  function verificationEvidence(): string {
    return summary.verified_through === null
      ? ''
      : ` · History through ${formatDate(summary.verified_through)}`
  }

  function importStatus(): string {
    if (summary.state === 'building') {
      const activity = summary.verified_through === null ? 'Building history' : 'Updating history'
      return `${activity} · ${summary.completed_issues}/${summary.total_issues} issues${verificationEvidence()}`
    }
    const state =
      summary.state === 'rate_limited'
        ? 'Rate limited'
        : summary.state === 'delayed'
          ? 'History delayed'
          : summary.state === 'failed'
            ? 'History import failed'
            : 'History unavailable'
    const diagnostic = summary.diagnostic === null ? '' : ` · ${summary.diagnostic}`
    const retry =
      summary.resume_at === null ? '' : ` · Retry after ${formatDate(summary.resume_at)}`
    return `${state}${diagnostic}${retry}${verificationEvidence()}`
  }

  function stop(): void {
    playing = false
    clock?.pause()
    clock = null
    if (frameHandle) cancelAnimationFrame(frameHandle)
    frameHandle = 0
  }

  function publish(frame: PlaybackFrame): void {
    playing = frame.playing
    change(frame.playhead)
    if (frame.crossed.length > 0) reached(frame.crossed)
  }

  function animate(monotonicNow: number): void {
    frameHandle = 0
    if (clock === null) return
    const frame = clock.tick(monotonicNow)
    publish(frame)
    if (frame.playing) frameHandle = requestAnimationFrame(animate)
    else clock = null
  }

  function togglePlayback(): void {
    if (playing) {
      stop()
      return
    }
    if (!hasHistory) return
    clock = new PlaybackClock(start, end, events, speed)
    const frame = clock.play(playhead, performance.now())
    publish(frame)
    if (frame.playing) frameHandle = requestAnimationFrame(animate)
  }

  function cycleSpeed(): void {
    const index = PLAYBACK_SPEEDS.indexOf(speed)
    const next = PLAYBACK_SPEEDS[(index + 1) % PLAYBACK_SPEEDS.length]
    if (clock !== null && playing) {
      const frame = clock.setSpeed(next, performance.now())
      publish(frame)
      if (!frame.playing) {
        if (frameHandle) cancelAnimationFrame(frameHandle)
        frameHandle = 0
        clock = null
      }
    }
    speed = next
  }

  function go(next: number | null): void {
    stop()
    change(next === null || next >= end ? null : next)
  }

  function scrub(event: Event): void {
    go(Number((event.currentTarget as HTMLInputElement).value))
  }

  function navigate(event: KeyboardEvent): void {
    if (!hasHistory) return
    let next: number | null | undefined
    if (event.key === 'Home') next = eventTimes[0] ?? start
    else if (event.key === 'End') next = null
    else if (event.key === 'ArrowLeft') next = eventTimes.filter((time) => time < value).at(-1) ?? start
    else if (event.key === 'ArrowRight') next = eventTimes.find((time) => time > value) ?? null
    else return
    event.preventDefault()
    go(next)
  }

  function clusterLabel(cluster: EventTickCluster): string {
    const dates = cluster.times.map(formatDate).join(' to ')
    return `${dates}: ${cluster.events.map((event) => historyEventSummary(event)).join('; ')}`
  }

  onMount(() => {
    const measure = () => (trackWidth = track.clientWidth || 600)
    measure()
    if (typeof ResizeObserver === 'undefined') return
    const observer = new ResizeObserver(measure)
    observer.observe(track)
    return () => observer.disconnect()
  })

  onDestroy(stop)
</script>

<section class="temporal-timeline" aria-label="Issue history timeline">
  {#if summary.state === 'building'}
    <span class="timeline-state" role="status" aria-live="polite">{importStatus()}</span>
  {:else if summary.state === 'complete' && !hasHistory}
    <span class="timeline-state" role="status" aria-live="polite">No issue history</span>
  {:else if summary.state !== 'complete'}
    <span class="timeline-state" role="status" aria-live="polite">{importStatus()}</span>
  {/if}

  {#if newActivity && playhead !== null}
    <button class="new-activity" type="button" onclick={returnToNow}>New activity</button>
  {/if}

  <output data-control="date" for="issue-history-playhead">{label}</output>
  <div class="timeline-track" data-control="track" bind:this={track}>
    <input
      id="issue-history-playhead"
      type="range"
      aria-label="Issue history date"
      aria-valuetext={label}
      min={start}
      max={end}
      step="any"
      {value}
      disabled={!hasHistory}
      oninput={scrub}
      onkeydown={navigate}
    />
    <div class="event-ticks" aria-label="Issue history events">
      {#each ticks as tick, index}
        <button
          class="event-tick"
          class:clustered={tick.events.length > 1}
          style:left={`${tick.position * 100}%`}
          type="button"
          aria-label={clusterLabel(tick)}
          aria-describedby={tooltip === tick ? 'history-event-tooltip' : undefined}
          data-event-count={tick.events.length}
          disabled={!hasHistory}
          onclick={() => go(tick.times[0])}
          onfocus={() => (tooltip = tick)}
          onblur={() => (tooltip = null)}
          onmouseenter={() => (tooltip = tick)}
          onmouseleave={() => (tooltip = null)}
        >
          <span class="visually-hidden">Event {index + 1}</span>
        </button>
      {/each}
    </div>
    {#if tooltip !== null}
      <div id="history-event-tooltip" class="event-tooltip" role="tooltip">
        <strong>{tooltip.times.map(formatDate).join(' · ')}</strong>
        <ul>
          {#each tooltip.events as event}
            <li>{historyEventSummary(event)}</li>
          {/each}
        </ul>
      </div>
    {/if}
  </div>
  <button
    data-control="play"
    type="button"
    disabled={!hasHistory}
    aria-label={playing ? 'Pause issue history' : 'Play issue history'}
    onclick={togglePlayback}
  >{playing ? 'Pause' : 'Play'}</button>
  <button
    data-control="speed"
    type="button"
    disabled={!hasHistory}
    aria-label={`Playback speed ${speed} times; activate to cycle`}
    onclick={cycleSpeed}
  >{speed}×</button>
</section>

<style>
  .temporal-timeline {
    position: absolute;
    right: clamp(0.75rem, 3vw, 2rem);
    bottom: 1rem;
    left: clamp(0.75rem, 3vw, 2rem);
    z-index: 4;
    display: grid;
    grid-template-columns: auto minmax(8rem, 1fr) auto auto;
    gap: 0.75rem;
    align-items: center;
    min-height: 3.25rem;
    padding: 0.65rem 0.85rem;
    border: 1px solid color-mix(in oklch, var(--border) 72%, transparent);
    border-radius: 0.85rem;
    color: var(--foreground);
    background: color-mix(in oklch, var(--surface-raised) 90%, transparent);
    box-shadow: 0 0.9rem 2.4rem var(--shadow-color);
    backdrop-filter: blur(12px);
  }

  .timeline-state {
    grid-column: 1 / -1;
    font-size: 0.8rem;
    color: var(--muted-foreground);
  }

  .new-activity {
    grid-column: 1 / -1;
    justify-self: center;
    min-height: 2.25rem;
    padding: 0.35rem 0.75rem;
    border: 1px solid var(--ring);
    border-radius: 999px;
    color: inherit;
    background: var(--surface-raised);
    cursor: pointer;
  }

  output {
    min-width: 9rem;
    font-variant-numeric: tabular-nums;
    font-size: 0.82rem;
  }

  .timeline-track {
    position: relative;
    min-width: 0;
  }

  input {
    position: relative;
    z-index: 2;
    width: 100%;
    min-height: 2.5rem;
    margin: 0;
  }

  .event-ticks {
    position: absolute;
    top: 50%;
    right: 0.5rem;
    left: 0.5rem;
    height: 0;
    pointer-events: none;
  }

  .event-tick {
    position: absolute;
    z-index: 3;
    width: 1.5rem;
    min-height: 2rem;
    padding: 0;
    border: 0;
    border-radius: 999px;
    background: transparent;
    transform: translate(-50%, -50%);
    pointer-events: auto;
  }

  .event-tick::after {
    position: absolute;
    top: 50%;
    left: 50%;
    width: 0.22rem;
    height: 1.1rem;
    border-radius: 999px;
    background: var(--muted-foreground);
    content: '';
    transform: translate(-50%, -50%);
  }

  .event-tick.clustered::after {
    width: 0.55rem;
    border: 2px solid var(--surface-raised);
    background: var(--foreground);
  }

  .event-tooltip {
    position: absolute;
    right: 0;
    bottom: calc(100% + 0.6rem);
    z-index: 6;
    width: min(24rem, 80vw);
    padding: 0.65rem 0.75rem;
    border: 1px solid var(--border);
    border-radius: 0.55rem;
    font-size: 0.78rem;
    background: var(--surface-raised);
    box-shadow: 0 0.6rem 1.4rem var(--shadow-color);
  }

  .event-tooltip ul {
    margin: 0.35rem 0 0;
    padding-left: 1.2rem;
  }

  button[data-control] {
    min-width: 2.75rem;
    min-height: 2.75rem;
    padding: 0.4rem 0.7rem;
    border: 1px solid var(--border);
    border-radius: 0.55rem;
    color: inherit;
    background: var(--surface);
    cursor: pointer;
  }

  button:disabled {
    cursor: default;
    opacity: 0.55;
  }

  button:focus-visible,
  input:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: 2px;
  }

  .visually-hidden {
    position: absolute;
    width: 1px;
    height: 1px;
    padding: 0;
    margin: -1px;
    overflow: hidden;
    clip: rect(0, 0, 0, 0);
    white-space: nowrap;
    border: 0;
  }

  @media (max-width: 42rem) {
    .temporal-timeline {
      grid-template-columns: minmax(0, 1fr) auto auto;
    }

    output {
      grid-column: 1 / -1;
      min-width: 0;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .temporal-timeline,
    .event-tick,
    button,
    input {
      scroll-behavior: auto;
      transition: none;
    }
  }
</style>

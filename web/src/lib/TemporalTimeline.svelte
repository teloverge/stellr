<script lang="ts">
  import type { HistoryEvent } from './history'
  import type { HistorySummary } from './model'

  let {
    summary,
    events,
    playhead = null,
    change = () => {},
  }: {
    summary: HistorySummary
    events: HistoryEvent[]
    playhead?: number | null
    change?: (playhead: number | null) => void
  } = $props()

  const hasHistory = $derived(
    summary.state === 'complete' &&
      summary.earliest_event_at !== null &&
      summary.verified_through !== null &&
      events.length > 0,
  )
  const value = $derived(playhead ?? summary.verified_through ?? 0)
  const label = $derived(playhead === null ? 'Now' : formatDate(playhead))

  function formatDate(epoch: number): string {
    return new Intl.DateTimeFormat(undefined, {
      year: 'numeric',
      month: 'short',
      day: 'numeric',
      hour: 'numeric',
      minute: '2-digit',
    }).format(new Date(epoch * 1_000))
  }

  function scrub(event: Event): void {
    change(Number((event.currentTarget as HTMLInputElement).value))
  }
</script>

<section class="temporal-timeline" aria-label="Issue history timeline">
  {#if summary.state === 'building'}
    <span class="timeline-state" role="status">
      Building history · {summary.completed_issues}/{summary.total_issues} issues
    </span>
  {:else if summary.state === 'complete' && !hasHistory}
    <span class="timeline-state" role="status">No issue history</span>
  {:else if summary.state !== 'complete'}
    <span class="timeline-state" role="status">{summary.diagnostic ?? 'History unavailable'}</span>
  {/if}

  <output for="issue-history-playhead">{label}</output>
  <input
    id="issue-history-playhead"
    type="range"
    aria-label="Issue history date"
    min={summary.earliest_event_at ?? 0}
    max={summary.verified_through ?? 0}
    {value}
    disabled={!hasHistory}
    oninput={scrub}
  />
  {#if playhead !== null}
    <button type="button" onclick={() => change(null)}>Return to Now</button>
  {/if}
</section>

<style>
  .temporal-timeline {
    position: absolute;
    right: clamp(0.75rem, 3vw, 2rem);
    bottom: 1rem;
    left: clamp(0.75rem, 3vw, 2rem);
    z-index: 4;
    display: grid;
    grid-template-columns: auto minmax(8rem, 1fr) auto;
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

  output {
    min-width: 9rem;
    font-variant-numeric: tabular-nums;
    font-size: 0.82rem;
  }

  input {
    width: 100%;
    min-height: 2.25rem;
  }

  button {
    min-height: 2.25rem;
    padding: 0.4rem 0.7rem;
    border: 1px solid var(--border);
    border-radius: 0.55rem;
    color: inherit;
    background: var(--surface);
    cursor: pointer;
  }

  button:focus-visible,
  input:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: 2px;
  }

  @media (max-width: 42rem) {
    .temporal-timeline {
      grid-template-columns: minmax(0, 1fr) auto;
    }

    output {
      grid-column: 1 / -1;
    }
  }
</style>

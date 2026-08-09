<script lang="ts">
  let {
    kind,
    projectName,
    elapsedSeconds = 0,
    message = '',
    cancel,
    retry,
  }: {
    kind: 'loading' | 'cancelled' | 'error'
    projectName: string
    elapsedSeconds?: number
    message?: string
    cancel?: () => void
    retry?: () => void
  } = $props()
</script>

<section class="layout-transition" class:loading={kind === 'loading'}>
  {#if kind === 'loading'}
    <div role="status" aria-live="polite">
      <h2>Charting {projectName}...</h2>
      <p>
        First load may take a moment.
        <span data-elapsed aria-live="off">{elapsedSeconds} seconds elapsed.</span>
      </p>
    </div>
    <button type="button" aria-label={`Cancel layout for ${projectName}`} onclick={cancel}>
      Cancel
    </button>
  {:else if kind === 'cancelled'}
    <div role="status">
      <h2>Layout canceled</h2>
      <p>{projectName} was not charted.</p>
    </div>
    <button type="button" onclick={retry}>Retry</button>
  {:else}
    <div role="alert">
      <h2>Could not chart {projectName}</h2>
      <p>{message}</p>
    </div>
    <button type="button" onclick={retry}>Retry</button>
  {/if}
</section>

<style>
  .layout-transition {
    display: grid;
    box-sizing: border-box;
    width: min(32rem, calc(100% - 3rem));
    margin: auto;
    padding: 2rem;
    place-items: center;
    border: 1px solid var(--border);
    border-radius: 1rem;
    color: var(--foreground);
    background: var(--surface-raised);
    box-shadow: 0 1.25rem 3rem var(--shadow-color);
    text-align: center;
  }

  h2 {
    margin: 0;
    font-size: 1rem;
  }

  p {
    max-width: 26rem;
    margin: 0.45rem 0 0;
    color: var(--muted-foreground);
    line-height: 1.5;
  }

  [data-elapsed] {
    display: block;
    margin-top: 0.2rem;
    color: var(--foreground);
    font-variant-numeric: tabular-nums;
  }

  button {
    margin-top: 1.1rem;
    padding: 0.55rem 1rem;
    border: 1px solid var(--border-strong);
    border-radius: 0.6rem;
    color: var(--foreground);
    background: var(--surface);
    font: inherit;
    cursor: pointer;
  }

  button:hover {
    border-color: var(--primary);
    color: var(--primary);
  }

  button:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: 2px;
  }
</style>

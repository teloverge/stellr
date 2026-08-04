<script lang="ts">
  import CircleNotchIcon from 'phosphor-svelte/lib/CircleNotchIcon'
  import CompassIcon from 'phosphor-svelte/lib/CompassIcon'

  let {
    kind,
    title,
    description,
  }: {
    kind: 'loading' | 'empty'
    title: string
    description: string
  } = $props()
</script>

<section class="state-panel" class:loading={kind === 'loading'} role={kind === 'loading' ? 'status' : undefined}>
  <div class="icon" aria-hidden="true">
    {#if kind === 'loading'}
      <CircleNotchIcon size={30} weight="bold" />
    {:else}
      <CompassIcon size={34} weight="duotone" />
    {/if}
  </div>
  <h2>{title}</h2>
  <p>{description}</p>
</section>

<style>
  .state-panel {
    display: grid;
    box-sizing: border-box;
    width: min(30rem, calc(100% - 3rem));
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

  .icon {
    display: grid;
    width: 3rem;
    height: 3rem;
    place-items: center;
    border-radius: 0.8rem;
    color: var(--primary);
    background: var(--primary-soft);
  }

  .loading .icon {
    animation: spin 1s linear infinite;
  }

  h2 {
    margin: 1rem 0 0;
    font-size: 1rem;
  }

  p {
    max-width: 24rem;
    margin: 0.4rem 0 0;
    color: var(--muted-foreground);
    line-height: 1.5;
  }

  @keyframes spin {
    to { transform: rotate(1turn); }
  }

  @media (prefers-reduced-motion: reduce) {
    .loading .icon { animation: none; }
  }
</style>

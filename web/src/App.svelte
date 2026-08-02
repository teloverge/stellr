<script lang="ts">
  import { onMount } from 'svelte'
  import DetailPane from './lib/DetailPane.svelte'
  import StarMap from './lib/StarMap.svelte'
  import { Control, pageIssue, takePageToken } from './lib/control.svelte'
  import { Route } from './lib/route.svelte'
  import { decideDock, type Dock } from './lib/starmap/dock'

  const currentIssue = pageIssue()
  const sessionToken = takePageToken()
  const control = new Control(sessionToken)
  const route = new Route()
  const spaces = $derived(control.model?.spaces ?? [])
  const activeSpace = $derived(
    (route.space === null ? null : spaces.find((space) => space.id === route.space)) ??
      spaces[0] ??
      null,
  )
  const activeStar = $derived(
    activeSpace === null || route.issue === null
      ? null
      : (activeSpace.stars.find((star) => star.number === route.issue) ?? null),
  )
  let workspace: HTMLElement
  let dock = $state<Dock>('right')

  $effect(() => {
    if (control.model === null) return

    if (activeSpace === null) {
      if (route.space !== null || route.issue !== null) route.go(null)
      return
    }

    if (route.space !== activeSpace.id) {
      route.go(activeSpace.id)
      return
    }

    if (route.issue !== null && activeStar === null) {
      route.go(activeSpace.id)
    }
  })

  onMount(() => {
    control.connect()
    const observer = new ResizeObserver(([entry]) => {
      if (!entry) return
      dock = decideDock(
        'hybrid',
        entry.contentRect.width,
        entry.contentRect.height,
        dock,
        true,
      )
    })
    observer.observe(workspace)

    return () => {
      observer.disconnect()
      control.destroy()
      route.destroy()
    }
  })
</script>

<main
  class="workspace"
  class:detail-right={activeStar !== null && dock === 'right'}
  class:detail-bottom={activeStar !== null && dock === 'bottom'}
  bind:this={workspace}
>
  <section class="map-region" aria-label="Issue map">
    {#if activeSpace}
      <StarMap
        space={activeSpace}
        {currentIssue}
        selectedIssue={activeStar?.number ?? null}
        select={(issueNumber) => route.go(activeSpace.id, issueNumber)}
      />
    {:else}
      <p>stellr</p>
    {/if}
  </section>

  {#if activeStar}
    <section class="detail-region">
      <DetailPane star={activeStar} close={() => route.go(activeSpace?.id ?? null)} />
    </section>
  {/if}
</main>

<style>
  .workspace {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    grid-template-rows: minmax(0, 1fr);
    width: 100%;
    height: 100dvh;
    min-height: 0;
  }

  .workspace.detail-right {
    grid-template-columns: minmax(0, 1fr) minmax(18rem, 24rem);
  }

  .workspace.detail-bottom {
    grid-template-rows: minmax(0, 1fr) minmax(14rem, 42dvh);
  }

  .map-region,
  .detail-region {
    min-width: 0;
    min-height: 0;
  }

  .detail-right .detail-region {
    grid-column: 2;
    grid-row: 1;
  }

  .detail-bottom .detail-region {
    grid-column: 1;
    grid-row: 2;
  }
</style>

<script lang="ts">
  import { onMount, untrack } from 'svelte'
  import DetailPane from './lib/DetailPane.svelte'
  import Sidebar from './lib/Sidebar.svelte'
  import StarMap from './lib/StarMap.svelte'
  import { removeSpace } from './lib/api'
  import { Control, pageIssue, takePageToken } from './lib/control.svelte'
  import { Route } from './lib/route.svelte'
  import { decideDock, type Dock } from './lib/starmap/dock'

  interface RemovalIntent {
    id: string
    fallbackId: string | null
    succeeded: boolean
  }

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
  let pendingAddedId = $state<string | null>(null)
  let pendingRemovals = $state.raw<Record<string, RemovalIntent>>({})

  $effect(() => {
    const modelSnapshot = control.model
    if (modelSnapshot === null) return

    untrack(() => {
      if (pendingAddedId !== null) {
        if (modelSnapshot.spaces.some((space) => space.id === pendingAddedId)) {
          pendingAddedId = null
        } else if (route.space === pendingAddedId) {
          return
        } else {
          pendingAddedId = null
        }
      }

      for (const pendingRemoval of Object.values(pendingRemovals)) {
        const removedSpaceStillPresent = modelSnapshot.spaces.some(
          (space) => space.id === pendingRemoval.id,
        )
        if (!removedSpaceStillPresent && pendingRemoval.succeeded) {
          clearRemovalIntent(pendingRemoval.id)
        } else if (
          route.space === pendingRemoval.id ||
          (pendingRemoval.succeeded && route.space === pendingRemoval.fallbackId)
        ) {
          return
        }
      }

      const routedSpace =
        route.space === null
          ? null
          : (modelSnapshot.spaces.find((space) => space.id === route.space) ?? null)
      const fallbackSpace = routedSpace ?? modelSnapshot.spaces[0] ?? null

      if (fallbackSpace === null) {
        if (route.space !== null || route.issue !== null) route.go(null)
        return
      }

      if (route.space !== fallbackSpace.id) {
        route.go(fallbackSpace.id)
        return
      }

      const routedStar =
        route.issue === null
          ? null
          : (fallbackSpace.stars.find((star) => star.number === route.issue) ?? null)
      if (route.issue !== null && routedStar === null) {
        route.go(fallbackSpace.id)
      }
    })
  })

  function addedSpace(addedId: string): void {
    pendingAddedId = addedId
    route.go(addedId)
  }

  function clearRemovalIntent(id: string): void {
    const nextIntents = { ...pendingRemovals }
    delete nextIntents[id]
    pendingRemovals = nextIntents
  }

  async function requestRemoveSpace(id: string): Promise<Response> {
    const removedIndex = spaces.findIndex((space) => space.id === id)
    pendingRemovals = {
      ...pendingRemovals,
      [id]: {
        id,
        fallbackId:
          removedIndex < 0
            ? null
            : (spaces[removedIndex + 1]?.id ?? spaces[removedIndex - 1]?.id ?? null),
        succeeded: false,
      },
    }

    try {
      const response = await removeSpace(id)
      if (!response.ok) clearRemovalIntent(id)
      return response
    } catch (error) {
      clearRemovalIntent(id)
      throw error
    }
  }

  function removedSpace(removedId: string): void {
    const removalIntent = pendingRemovals[removedId]
    if (removalIntent === undefined) return

    pendingRemovals = {
      ...pendingRemovals,
      [removedId]: { ...removalIntent, succeeded: true },
    }
    if (route.space === removedId) route.go(removalIntent.fallbackId)
    if (control.model?.spaces.every((space) => space.id !== removedId)) {
      clearRemovalIntent(removedId)
    }
  }

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

<main class="app-shell">
  <div class="sidebar-region">
    <Sidebar
      {spaces}
      activeSpaceId={route.space}
      connectionStatus={control.status}
      select={(spaceId) => route.go(spaceId)}
      added={addedSpace}
      removed={removedSpace}
      removeRequest={requestRemoveSpace}
    />
  </div>

  <section
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
        <p class="empty-map">No spaces yet. Add a local path or GitHub repository to begin.</p>
      {/if}
    </section>

    {#if activeStar}
      <section class="detail-region">
        <DetailPane star={activeStar} close={() => route.go(activeSpace?.id ?? null)} />
      </section>
    {/if}
  </section>
</main>

<style>
  .app-shell {
    display: grid;
    grid-template-columns: clamp(14rem, 24vw, 20rem) minmax(0, 1fr);
    width: 100%;
    height: 100dvh;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
  }

  .sidebar-region {
    display: flex;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
  }

  .sidebar-region :global(aside) {
    flex: 1 1 auto;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
  }

  .workspace {
    display: grid;
    grid-template-areas: "map";
    grid-template-columns: minmax(0, 1fr);
    grid-template-rows: minmax(0, 1fr);
    width: 100%;
    min-height: 0;
    overflow: hidden;
  }

  .workspace.detail-right {
    grid-template-areas: "map detail";
    grid-template-columns: minmax(0, 1fr) clamp(18rem, 32vw, 24rem);
  }

  .workspace.detail-bottom {
    grid-template-areas:
      "map"
      "detail";
    grid-template-rows: minmax(0, 1fr) clamp(14rem, 42dvh, 22rem);
  }

  .map-region,
  .detail-region {
    min-width: 0;
    min-height: 0;
    overflow: hidden;
  }

  .map-region {
    grid-area: map;
  }

  .detail-region {
    grid-area: detail;
  }

  .empty-map {
    display: flex;
    box-sizing: border-box;
    align-items: center;
    justify-content: center;
    width: 100%;
    height: 100%;
    margin: 0;
    padding: 2rem;
    color: var(--muted-foreground);
    text-align: center;
  }
</style>

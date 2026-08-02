<script lang="ts">
  import { onMount } from 'svelte'
  import DetailPane from './lib/DetailPane.svelte'
  import Sidebar from './lib/Sidebar.svelte'
  import StarMap from './lib/StarMap.svelte'
  import { Control, pageIssue, takePageToken } from './lib/control.svelte'
  import { Route } from './lib/route.svelte'
  import { decideDock, type Dock } from './lib/starmap/dock'

  const currentIssue = pageIssue()
  const sessionToken = takePageToken()
  const control = new Control(sessionToken)
  const route = new Route()
  const spaces = $derived(control.model?.spaces ?? [])
  let pendingAddedSpace = $state<string | null>(null)
  let pendingRemovedSpaces = $state<string[]>([])
  const availableSpaces = $derived(
    spaces.filter((space) => !pendingRemovedSpaces.includes(space.id)),
  )
  const activeSpace = $derived(
    (route.space === null
      ? null
      : availableSpaces.find((space) => space.id === route.space)) ??
      availableSpaces[0] ??
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

    if (pendingAddedSpace !== null && spaces.some((space) => space.id === pendingAddedSpace)) {
      pendingAddedSpace = null
    }

    const pendingStillPresent = pendingRemovedSpaces.filter((id) =>
      spaces.some((space) => space.id === id),
    )
    if (pendingStillPresent.length !== pendingRemovedSpaces.length) {
      pendingRemovedSpaces = pendingStillPresent
    }

    if (
      pendingAddedSpace !== null &&
      route.space === pendingAddedSpace &&
      !spaces.some((space) => space.id === pendingAddedSpace)
    ) {
      return
    }

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

  function selectSpace(id: string): void {
    pendingAddedSpace = null
    route.go(id)
  }

  function addedSpace(id: string): void {
    pendingAddedSpace = id
    route.go(id)
  }

  function removedSpace(id: string): void {
    const orderedIds = availableSpaces.map((space) => space.id)
    const removedIndex = orderedIds.indexOf(id)
    if (!pendingRemovedSpaces.includes(id)) {
      pendingRemovedSpaces = [...pendingRemovedSpaces, id]
    }

    if (route.space !== id) return

    const remainingIds = orderedIds.filter((spaceId) => spaceId !== id)
    const nextId =
      remainingIds[removedIndex] ?? remainingIds[removedIndex - 1] ?? remainingIds[0] ?? null
    route.go(nextId)
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
      route.destroy()
    }
  })
</script>

<main class="app-shell">
  <Sidebar
    {spaces}
    activeSpaceId={route.space ?? activeSpace?.id ?? null}
    connectionStatus={control.status}
    select={selectSpace}
    added={addedSpace}
    removed={removedSpace}
  />

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
          select={(issueNumber) => route.go(activeSpace.id, issueNumber)}
        />
      {:else if control.model === null}
        <p class="empty-state">Connecting to stellr…</p>
      {:else}
        <p class="empty-state">Add a space to begin</p>
      {/if}
    </section>

    {#if activeStar}
      <section class="detail-region">
        <DetailPane star={activeStar} close={() => route.go(activeSpace?.id ?? null)} />
      </section>
    {/if}
  </section>
</main>

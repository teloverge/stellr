<script lang="ts">
  import { onMount, untrack } from 'svelte'
  import DetailPane from './lib/DetailPane.svelte'
  import SignInPanel from './lib/SignInPanel.svelte'
  import Sidebar from './lib/Sidebar.svelte'
  import StatePanel from './lib/StatePanel.svelte'
  import StarMap from './lib/StarMap.svelte'
  import TemporalTimeline from './lib/TemporalTimeline.svelte'
  import { addSpace, removeSpace } from './lib/api'
  import { Control, pageIssue, takePageToken } from './lib/control.svelte'
  import type { Model } from './lib/model'
  import { fetchHistory } from './lib/history-api'
  import { projectTemporalSpace, type HistoryEvent } from './lib/history'
  import { Route } from './lib/route.svelte'
  import { ThemeController } from './lib/theme.svelte'
  import { decideDock, type Dock } from './lib/starmap/dock'
  import {
    beginDeviceAuthorization,
    cancelDeviceAuthorization,
    deviceAuthorizationStatus,
    hasNativeAuth,
    type DeviceFlowStatus,
  } from './lib/native-auth'
  import {
    applyNativeRouteEvent,
    hasNativeRoutePersistence,
    persistNativeRoute,
    takeNativeRouteEvent,
  } from './lib/native-route'

  interface RemovalIntent {
    id: string
    fallbackId: string | null
    succeeded: boolean
  }

  interface ResolvedRoute {
    space: Model['spaces'][number] | null
    star: Model['spaces'][number]['stars'][number] | null
  }

  function resolveRoute(
    modelSnapshot: Model | null,
    spaceId: string | null,
    issueNumber: number | null,
    allowSpaceFallback = true,
  ): ResolvedRoute {
    const spaces = modelSnapshot?.spaces ?? []
    const routedSpace =
      spaceId === null ? null : (spaces.find((space) => space.id === spaceId) ?? null)
    const space = routedSpace ?? (allowSpaceFallback ? spaces[0] ?? null : null)
    const star =
      space === null || issueNumber === null
        ? null
        : (space.stars.find((candidate) => candidate.number === issueNumber) ?? null)

    return { space, star }
  }

  const currentIssue = pageIssue()
  const sessionToken = takePageToken()
  const control = new Control(sessionToken)
  const route = new Route()
  const theme = new ThemeController()
  let pendingAddedId = $state<string | null>(null)
  const spaces = $derived(control.model?.spaces ?? [])
  const resolvedRoute = $derived(
    resolveRoute(
      control.model,
      route.space,
      route.issue,
      pendingAddedId === null || route.space !== pendingAddedId,
    ),
  )
  const activeSpace = $derived(resolvedRoute.space)
  const activeStar = $derived(resolvedRoute.star)
  let historyEvents = $state.raw<HistoryEvent[]>([])
  let historyPlayhead = $state<number | null>(null)
  let historySpaceId = $state<string | null>(null)
  let requestedHistoryKey: string | null = null
  const temporalSpace = $derived(
    activeSpace === null
      ? null
      : projectTemporalSpace(activeSpace, historyEvents, historyPlayhead),
  )
  let workspace: HTMLElement
  let dock = $state<Dock>('right')
  let pendingRemovals = $state.raw<Record<string, RemovalIntent>>({})
  let authStatus = $state<DeviceFlowStatus | null>(null)
  let nativeRouteNotice = $state<string | null>(null)
  let nativeRouteBusy = false
  let persistedRouteKey: string | null = null
  const nativeRoutePersistence = hasNativeRoutePersistence()
  const modelLoading = $derived(
    control.model === null ||
      (nativeRoutePersistence && control.revision === 1 && spaces.length === 0 && route.space !== null),
  )

  $effect(() => {
    const space = activeSpace
    if (historySpaceId !== space?.id) {
      historySpaceId = space?.id ?? null
      historyEvents = []
      historyPlayhead = null
      requestedHistoryKey = null
    }
    const summary = space?.history
    if (space === null || summary?.state !== 'complete') return
    const key = `${space.id}:${summary.revision}`
    if (requestedHistoryKey === key) return
    requestedHistoryKey = key
    void fetchHistory(space.id)
      .then((response) => {
        if (historySpaceId !== space.id) return
        historyEvents = response.events
      })
      .catch(() => {
        if (requestedHistoryKey === key) requestedHistoryKey = null
      })
  })

  async function pollNativeRoute(): Promise<void> {
    if (nativeRouteBusy) return
    nativeRouteBusy = true
    try {
      const event = await takeNativeRouteEvent()
      if (event === null) return
      const outcome = await applyNativeRouteEvent(
        event,
        spaces.map((space) => space.id),
        addSpace,
      )
      if (outcome.error !== null) {
        nativeRouteNotice = outcome.error
        return
      }
      if (outcome.route !== null) {
        nativeRouteNotice = null
        if (!spaces.some((space) => space.id === outcome.route?.space)) {
          pendingAddedId = outcome.route.space
        }
        route.go(outcome.route.space, outcome.route.issue)
      }
    } catch (error) {
      nativeRouteNotice = String(error)
    } finally {
      nativeRouteBusy = false
    }
  }

  async function refreshAuthorization(): Promise<void> {
    try {
      authStatus = await deviceAuthorizationStatus()
    } catch (error) {
      authStatus = { state: 'failed', message: String(error) }
    }
  }

  async function beginAuthorization(): Promise<void> {
    try {
      authStatus = await beginDeviceAuthorization()
    } catch (error) {
      authStatus = { state: 'failed', message: String(error) }
    }
  }

  async function cancelAuthorization(): Promise<void> {
    try {
      authStatus = await cancelDeviceAuthorization()
    } catch (error) {
      authStatus = { state: 'failed', message: String(error) }
    }
  }

  function reconcileModel(modelSnapshot: Model): void {
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

    const resolved = resolveRoute(modelSnapshot, route.space, route.issue)
    const fallbackSpace = resolved.space

    if (fallbackSpace === null) {
      if (route.space !== null || route.issue !== null) route.go(null)
      return
    }

    if (route.space !== fallbackSpace.id) {
      route.go(fallbackSpace.id)
      return
    }

    if (route.issue !== null && resolved.star === null) {
      route.go(fallbackSpace.id)
    }
  }

  $effect(() => {
    const modelSnapshot = control.model
    if (modelSnapshot === null) return
    if (
      nativeRoutePersistence &&
      control.revision === 1 &&
      modelSnapshot.spaces.length === 0 &&
      route.space !== null
    ) {
      return
    }

    untrack(() => reconcileModel(modelSnapshot))
  })

  $effect(() => {
    const modelSnapshot = control.model
    const spaceId = route.space
    const issueNumber = route.issue
    if (!nativeRoutePersistence || modelSnapshot === null) return
    if (control.revision === 1 && modelSnapshot.spaces.length === 0 && spaceId !== null) return
    if (spaceId === null && modelSnapshot.spaces.length > 0) return

    const validated = resolveRoute(modelSnapshot, spaceId, issueNumber, false)
    if (spaceId !== null && validated.space?.id !== spaceId) return
    if (issueNumber !== null && validated.star?.number !== issueNumber) return

    const key = JSON.stringify([spaceId, issueNumber])
    if (key === persistedRouteKey) return
    persistedRouteKey = key
    void persistNativeRoute(spaceId, issueNumber).catch((error) => {
      persistedRouteKey = null
      nativeRouteNotice = `Could not remember the current route: ${String(error)}`
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

  function rejectRemovalIntent(id: string): void {
    clearRemovalIntent(id)
    if (control.model !== null) reconcileModel(control.model)
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
      if (!response.ok) rejectRemovalIntent(id)
      return response
    } catch (error) {
      rejectRemovalIntent(id)
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
    if (control.model !== null) reconcileModel(control.model)
  }

  onMount(() => {
    void theme.start()
    control.connect()
    const nativeAuth = hasNativeAuth()
    if (nativeAuth) void refreshAuthorization()
    if (nativeAuth) void pollNativeRoute()
    const authPoller = nativeAuth ? window.setInterval(refreshAuthorization, 1_000) : undefined
    const routePoller = nativeAuth ? window.setInterval(pollNativeRoute, 250) : undefined
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
      if (authPoller !== undefined) window.clearInterval(authPoller)
      if (routePoller !== undefined) window.clearInterval(routePoller)
      observer.disconnect()
      control.destroy()
      route.destroy()
      theme.destroy()
    }
  })
</script>

<main class="app-shell">
  {#if nativeRouteNotice !== null}
    <div class="route-notice" role="alert">
      <span>{nativeRouteNotice}</span>
      <button aria-label="Dismiss routing error" onclick={() => (nativeRouteNotice = null)}>×</button>
    </div>
  {/if}
  <div class="sidebar-region">
    <Sidebar
      {spaces}
      activeSpaceId={route.space}
      connectionStatus={control.status}
      select={(spaceId) => route.go(spaceId)}
      added={addedSpace}
      removed={removedSpace}
      removeRequest={requestRemoveSpace}
      themePreference={theme.preference}
      selectTheme={(preference) => theme.setPreference(preference)}
    />
  </div>

  <section
    class="workspace"
    class:detail-right={activeStar !== null && dock === 'right'}
    class:detail-bottom={activeStar !== null && dock === 'bottom'}
    bind:this={workspace}
  >
    {#if authStatus !== null && (authStatus.state !== 'authorized' || authStatus.storage_warning !== null)}
      <SignInPanel
        status={authStatus}
        begin={beginAuthorization}
        cancel={cancelAuthorization}
      />
    {/if}
    <section class="map-region" aria-label="Issue map">
      {#if modelLoading}
        <StatePanel
          kind="loading"
          title="Opening observatory"
          description="Loading cached spaces and the latest GitHub issue state."
        />
      {:else if activeSpace && temporalSpace}
        <StarMap
          space={temporalSpace}
          {currentIssue}
          selectedIssue={activeStar?.number ?? null}
          select={(issueNumber) => route.go(activeSpace.id, issueNumber)}
          bottomInset={activeSpace.history === undefined ? 16 : 88}
        />
        {#if activeSpace.history !== undefined}
          <TemporalTimeline
            summary={activeSpace.history}
            events={historyEvents}
            playhead={historyPlayhead}
            change={(playhead) => (historyPlayhead = playhead)}
          />
        {/if}
      {:else}
        <StatePanel
          kind="empty"
          title="No spaces yet"
          description="Add a local path or GitHub repository to begin."
        />
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
    position: relative;
    display: grid;
    grid-template-columns: clamp(14rem, 24vw, 20rem) minmax(0, 1fr);
    width: 100%;
    height: 100dvh;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
  }

  .route-notice {
    position: absolute;
    top: 1rem;
    right: 1rem;
    z-index: 10;
    display: flex;
    max-width: min(28rem, calc(100% - 2rem));
    gap: 0.75rem;
    align-items: flex-start;
    padding: 0.8rem 1rem;
    border: 1px solid var(--destructive);
    border-radius: 0.7rem;
    color: var(--foreground);
    background: color-mix(in oklch, var(--surface-raised) 88%, var(--destructive));
    box-shadow: 0 0.75rem 2rem var(--shadow-color);
  }

  .route-notice button {
    padding: 0;
    border: 0;
    color: inherit;
    background: transparent;
    cursor: pointer;
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
    position: relative;
    display: grid;
    grid-template-areas: "map";
    grid-template-columns: minmax(0, 1fr);
    grid-template-rows: minmax(0, 1fr);
    width: 100%;
    min-height: 0;
    overflow: hidden;
    background: var(--map-background);
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
    position: relative;
    grid-area: map;
  }

  .detail-region {
    grid-area: detail;
  }

</style>

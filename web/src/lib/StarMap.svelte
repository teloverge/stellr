<script lang="ts">
  import { onMount, untrack } from 'svelte'
  import LayoutTransition from './LayoutTransition.svelte'
  import type { SpaceModel } from './model'
  import { toRendererModel } from './starmap/adapt'
  import { structureSignature, type LayoutNode } from './starmap/layout'
  import {
    browserLayoutLoader,
    type LayoutLoad,
    type LayoutRequester,
  } from './starmap/layout-loader'
  import { StarMap as Renderer } from './starmap/starmap'

  let {
    space,
    currentIssue = null,
    selectedIssue = null,
    select,
    layout = browserLayoutLoader,
    ready,
    cancelled,
    failed,
  }: {
    space: SpaceModel
    currentIssue?: number | null
    selectedIssue?: number | null
    select?: (issueNumber: number) => void
    layout?: LayoutRequester
    ready?: (spaceId: string) => void
    cancelled?: (spaceId: string) => void
    failed?: (spaceId: string, message: string) => void
  } = $props()
  let host: HTMLDivElement
  let renderer = $state.raw<Renderer | undefined>(undefined)
  let appliedSignature = $state<string | null>(null)
  let synchronizedRenderer: Renderer | undefined
  let synchronizedSpaceId: string | null = null
  let synchronizedIssue: number | null = null
  let synchronizedLayoutSignature: string | null = null
  let transition = $state<
    | { kind: 'loading'; spaceId: string; projectName: string }
    | { kind: 'cancelled'; spaceId: string; projectName: string }
    | { kind: 'error'; spaceId: string; projectName: string; message: string }
    | null
  >(null)
  let elapsedSeconds = $state(0)
  let activeLoad: Extract<LayoutLoad, { kind: 'pending' }> | null = null
  let activeSignature: string | null = null
  let activeSpaceId: string | null = null
  let activeModel: ReturnType<typeof toRendererModel> | null = null
  let activeGeneration = 0
  let timer: number | undefined
  let destroyed = false

  function nodesFor(model: ReturnType<typeof toRendererModel>): LayoutNode[] {
    return model.map(({ num, title, blockedBy, parentIssue }) => ({
      num,
      title,
      blockedBy,
      parentIssue,
    }))
  }

  function stopTimer(): void {
    if (timer === undefined) return
    window.clearInterval(timer)
    timer = undefined
  }

  function retireActiveLoad(): void {
    const previous = activeLoad
    activeLoad = null
    activeSignature = null
    activeSpaceId = null
    activeModel = null
    activeGeneration++
    stopTimer()
    previous?.cancel()
  }

  function applyModel(
    target: Renderer,
    model: ReturnType<typeof toRendererModel>,
    signature: string,
    points: Parameters<Renderer['setModel']>[3],
    spaceId: string,
  ): void {
    target.setModel(model, {}, currentIssue, points)
    appliedSignature = signature
    transition = null
    ready?.(spaceId)
  }

  function beginLayout(
    target: Renderer,
    model: ReturnType<typeof toRendererModel>,
    spaceId: string,
    projectName: string,
  ): void {
    retireActiveLoad()
    const generation = activeGeneration
    const load = layout.load(nodesFor(model))
    if (load.kind === 'cached') {
      applyModel(target, model, load.signature, load.points, spaceId)
      return
    }

    activeLoad = load
    activeSignature = load.signature
    activeSpaceId = spaceId
    activeModel = model
    transition = { kind: 'loading', spaceId, projectName }
    elapsedSeconds = 0
    timer = window.setInterval(() => {
      elapsedSeconds++
    }, 1_000)

    void load.result.then((outcome) => {
      if (destroyed || generation !== activeGeneration || activeLoad !== load) return
      const latestModel = activeModel
      activeLoad = null
      activeSignature = null
      activeSpaceId = null
      activeModel = null
      stopTimer()
      if (outcome.kind === 'ready') {
        if (latestModel === null) return
        applyModel(target, latestModel, load.signature, outcome.points, spaceId)
      } else if (outcome.kind === 'cancelled') {
        transition = { kind: 'cancelled', spaceId, projectName }
      } else {
        transition = { kind: 'error', spaceId, projectName, message: outcome.message }
        failed?.(spaceId, outcome.message)
      }
    })
  }

  function cancelLayout(): void {
    if (transition?.kind !== 'loading' || activeLoad === null) return
    const { spaceId, projectName } = transition
    const load = activeLoad
    activeLoad = null
    activeSignature = null
    activeSpaceId = null
    activeModel = null
    activeGeneration++
    stopTimer()
    load.cancel()
    transition = { kind: 'cancelled', spaceId, projectName }
    cancelled?.(spaceId)
  }

  function retryLayout(): void {
    if (renderer === undefined) return
    beginLayout(renderer, toRendererModel(space), space.id, space.name)
  }

  $effect(() => {
    const target = renderer
    const model = toRendererModel(space)
    const signature = structureSignature(model)
    const spaceId = space.id
    const projectName = space.name
    if (target === undefined) return

    if (signature === untrack(() => appliedSignature)) {
      target.setModel(model, {}, currentIssue)
      if (untrack(() => transition) !== null) transition = null
      return
    }
    if (
      activeLoad !== null &&
      activeSignature === signature &&
      activeSpaceId === spaceId
    ) {
      activeModel = model
      return
    }
    beginLayout(target, model, spaceId, projectName)
  })

  $effect(() => {
    if (renderer === undefined) return

    const rendererChanged = synchronizedRenderer !== renderer
    const spaceChanged = synchronizedSpaceId !== space.id
    const issueChanged = synchronizedIssue !== selectedIssue
    const layoutChanged = synchronizedLayoutSignature !== appliedSignature
    if (!rendererChanged && !spaceChanged && !issueChanged && !layoutChanged) return

    synchronizedRenderer = renderer
    synchronizedSpaceId = space.id
    synchronizedIssue = selectedIssue
    synchronizedLayoutSignature = appliedSignature
    if (spaceChanged || layoutChanged || selectedIssue === null) renderer.select(null)
    if (selectedIssue !== null) renderer.select(selectedIssue)
  })

  onMount(() => {
    renderer = new Renderer()
    const background = getComputedStyle(host).getPropertyValue('--map-background').trim()
    renderer.setBackground(background)
    renderer.mount(host)
    renderer.onSelect((issueNumber) => {
      if (issueNumber !== null && issueNumber !== selectedIssue) select?.(issueNumber)
    })

    return () => {
      destroyed = true
      retireActiveLoad()
      renderer?.destroy()
      renderer = undefined
    }
  })
</script>

<div
  class="star-map"
  class:transitioning={transition !== null}
  aria-hidden={transition !== null ? 'true' : undefined}
  bind:this={host}
></div>
{#if transition !== null}
  <div class="transition-layer">
    <LayoutTransition
      kind={transition.kind}
      projectName={transition.projectName}
      {elapsedSeconds}
      message={transition.kind === 'error' ? transition.message : undefined}
      cancel={transition.kind === 'loading' ? cancelLayout : undefined}
      retry={transition.kind !== 'loading' ? retryLayout : undefined}
    />
  </div>
{/if}

<style>
  .star-map {
    display: block;
    width: 100%;
    height: 100%;
    min-height: 0;
    background: var(--map-background);
  }

  .star-map.transitioning {
    visibility: hidden;
  }

  .transition-layer {
    position: absolute;
    inset: 0;
    z-index: 2;
    display: grid;
    background: var(--map-background);
  }
</style>

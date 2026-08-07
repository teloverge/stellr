<script lang="ts">
  import { onMount } from 'svelte'
  import type { SpaceModel } from './model'
  import { toRendererModel } from './starmap/adapt'
  import { StarMap as Renderer } from './starmap/starmap'

  let {
    space,
    currentIssue = null,
    selectedIssue = null,
    select,
  }: {
    space: SpaceModel
    currentIssue?: number | null
    selectedIssue?: number | null
    select?: (issueNumber: number) => void
  } = $props()
  let host: HTMLDivElement
  let renderer = $state.raw<Renderer | undefined>(undefined)
  let synchronizedRenderer: Renderer | undefined
  let synchronizedSpaceId: string | null = null
  let synchronizedIssue: number | null = null

  $effect(() => {
    const model = toRendererModel(space)
    renderer?.setModel(model, {}, currentIssue)
  })

  $effect(() => {
    if (renderer === undefined) return

    const rendererChanged = synchronizedRenderer !== renderer
    const spaceChanged = synchronizedSpaceId !== space.id
    const issueChanged = synchronizedIssue !== selectedIssue
    if (!rendererChanged && !spaceChanged && !issueChanged) return

    synchronizedRenderer = renderer
    synchronizedSpaceId = space.id
    synchronizedIssue = selectedIssue
    if (spaceChanged || selectedIssue === null) renderer.select(null)
    if (selectedIssue !== null) renderer.select(selectedIssue)
  })

  onMount(() => {
    renderer = new Renderer()
    const motionQuery = typeof matchMedia === 'function'
      ? matchMedia('(prefers-reduced-motion: reduce)')
      : null
    const updateReducedMotion = (event: MediaQueryListEvent) => {
      renderer?.setReducedMotion(event.matches)
    }
    renderer.setReducedMotion(motionQuery?.matches ?? false)
    motionQuery?.addEventListener('change', updateReducedMotion)
    const background = getComputedStyle(host).getPropertyValue('--map-background').trim()
    renderer.setBackground(background)
    renderer.mount(host)
    renderer.onSelect((issueNumber) => {
      if (issueNumber !== null && issueNumber !== selectedIssue) select?.(issueNumber)
    })

    return () => {
      motionQuery?.removeEventListener('change', updateReducedMotion)
      renderer?.destroy()
      renderer = undefined
    }
  })
</script>

<div class="star-map" bind:this={host}></div>

<style>
  .star-map {
    display: block;
    width: 100%;
    height: 100%;
    min-height: 0;
    background: var(--map-background);
  }
</style>

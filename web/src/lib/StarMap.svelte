<script lang="ts">
  import { onMount } from 'svelte'
  import type { SpaceModel } from './model'
  import { toRendererModel } from './starmap/adapt'
  import { StarMap as Renderer } from './starmap/starmap'

  let {
    space,
    currentIssue = null,
    select,
  }: {
    space: SpaceModel
    currentIssue?: number | null
    select?: (issueNumber: number) => void
  } = $props()
  let host: HTMLDivElement
  let renderer = $state.raw<Renderer | undefined>(undefined)

  $effect(() => {
    const model = toRendererModel(space)
    renderer?.setModel(model, {}, currentIssue)
  })

  onMount(() => {
    renderer = new Renderer()
    const background = getComputedStyle(host).getPropertyValue('--background').trim()
    renderer.setBackground(background)
    renderer.mount(host)
    renderer.onSelect((issueNumber) => {
      if (issueNumber !== null) select?.(issueNumber)
    })

    return () => {
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
  }
</style>

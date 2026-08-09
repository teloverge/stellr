<script lang="ts">
  import StarMap from './StarMap.svelte'
  import type { SpaceModel } from './model'
  import type { LayoutRequester } from './starmap/layout-loader'

  let {
    initialSpace: space,
    initialSelectedIssue: selectedIssue = null,
    layout,
    ready,
    cancelled,
    failed,
  }: {
    initialSpace: SpaceModel
    initialSelectedIssue?: number | null
    layout: LayoutRequester
    ready?: (spaceId: string) => void
    cancelled?: (spaceId: string) => void
    failed?: (spaceId: string, message: string) => void
  } = $props()

  export function updateSpace(next: SpaceModel): void {
    space = next
  }
</script>

<StarMap {space} {selectedIssue} {layout} {ready} {cancelled} {failed} />

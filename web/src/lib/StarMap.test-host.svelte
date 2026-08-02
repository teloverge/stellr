<script lang="ts">
  import StarMap from './StarMap.svelte'
  import type { SpaceModel } from './model'

  let {
    initialSpace: space,
    initialSelectedIssue: selectedIssue = null,
  }: {
    initialSpace: SpaceModel
    initialSelectedIssue?: number | null
  } = $props()

  export function updateSpace(next: SpaceModel): void {
    space = next
  }
</script>

<StarMap {space} {selectedIssue} />

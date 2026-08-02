<script lang="ts">
  import { onMount } from 'svelte'
  import StarMap from './lib/StarMap.svelte'
  import { Control, pageIssue, takePageToken } from './lib/control.svelte'

  const currentIssue = pageIssue()
  const sessionToken = takePageToken()
  const control = new Control(sessionToken)
  const space = $derived(control.model?.spaces[0] ?? null)

  onMount(() => control.connect())
</script>

<main>
  {#if space}
    <StarMap {space} {currentIssue} />
  {:else}
    <p>stellr</p>
  {/if}
</main>

<style>
  main {
    width: 100%;
    height: 100dvh;
    min-height: 0;
  }
</style>

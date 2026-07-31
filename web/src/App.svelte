<script lang="ts">
  import { onMount } from 'svelte'
  import StarMap from './lib/StarMap.svelte'
  import { Control } from './lib/control.svelte'

  const control = new Control()
  const space = $derived(control.model?.spaces[0] ?? null)

  onMount(() => control.connect())
</script>

<main>
  {#if space}
    <StarMap {space} />
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

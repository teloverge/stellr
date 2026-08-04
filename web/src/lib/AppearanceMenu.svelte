<script lang="ts">
  import DesktopIcon from 'phosphor-svelte/lib/DesktopIcon'
  import GearSixIcon from 'phosphor-svelte/lib/GearSixIcon'
  import MoonIcon from 'phosphor-svelte/lib/MoonIcon'
  import SunIcon from 'phosphor-svelte/lib/SunIcon'
  import type { ThemePreference } from './native-shell'

  let {
    preference,
    select,
  }: {
    preference: ThemePreference
    select: (preference: ThemePreference) => Promise<void>
  } = $props()

  let open = $state(false)
  let busy = $state(false)
  let error = $state<string | null>(null)

  const choices = [
    { value: 'system', label: 'System', icon: DesktopIcon },
    { value: 'light', label: 'Light', icon: SunIcon },
    { value: 'dark', label: 'Dark', icon: MoonIcon },
  ] as const

  async function choose(value: ThemePreference): Promise<void> {
    if (busy) return
    busy = true
    error = null
    try {
      await select(value)
      open = false
    } catch (failure) {
      error = String(failure)
    } finally {
      busy = false
    }
  }
</script>

<svelte:window onkeydown={(event) => event.key === 'Escape' && (open = false)} />

<div class="appearance-menu">
  <button
    class="trigger"
    type="button"
    aria-label="Appearance"
    aria-haspopup="true"
    aria-expanded={open}
    onclick={() => (open = !open)}
  >
    <GearSixIcon size={18} aria-hidden="true" />
  </button>
  {#if open}
    <div class="menu" role="radiogroup" aria-label="Appearance mode">
      {#each choices as choice (choice.value)}
        <button
          type="button"
          role="radio"
          aria-checked={preference === choice.value}
          disabled={busy}
          onclick={() => choose(choice.value)}
        >
          <choice.icon size={17} aria-hidden="true" />
          <span>{choice.label}</span>
        </button>
      {/each}
      {#if error}<p role="alert">{error}</p>{/if}
    </div>
  {/if}
</div>

<style>
  .appearance-menu {
    position: relative;
  }

  button {
    border: 1px solid var(--border);
    color: var(--foreground);
    background: var(--muted);
    font: inherit;
    cursor: pointer;
  }

  button:focus-visible {
    outline: 2px solid var(--primary);
    outline-offset: 2px;
  }

  .trigger {
    display: grid;
    width: 2rem;
    height: 2rem;
    padding: 0;
    place-items: center;
    border-radius: 0.5rem;
  }

  .menu {
    position: absolute;
    top: calc(100% + 0.4rem);
    right: 0;
    z-index: 30;
    display: grid;
    width: 9.5rem;
    padding: 0.35rem;
    border: 1px solid var(--border);
    border-radius: 0.65rem;
    background: var(--surface-raised);
    box-shadow: 0 0.8rem 2rem var(--shadow-color);
  }

  .menu button {
    display: flex;
    gap: 0.55rem;
    align-items: center;
    padding: 0.55rem 0.65rem;
    border-color: transparent;
    border-radius: 0.4rem;
    background: transparent;
    text-align: left;
  }

  .menu button[aria-checked='true'] {
    color: var(--primary);
    background: var(--muted);
  }

  p {
    margin: 0.35rem;
    color: var(--destructive);
    font-size: 0.75rem;
  }
</style>

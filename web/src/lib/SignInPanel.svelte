<script lang="ts">
  import type { DeviceFlowStatus } from './native-auth'

  let {
    status,
    begin,
    cancel,
  }: {
    status: DeviceFlowStatus
    begin: () => Promise<void>
    cancel: () => Promise<void>
  } = $props()

  let busy = $state(false)

  async function run(action: () => Promise<void>): Promise<void> {
    busy = true
    try {
      await action()
    } finally {
      busy = false
    }
  }

  function minutes(seconds: number): string {
    const count = Math.max(1, Math.ceil(seconds / 60))
    return `${count} ${count === 1 ? 'minute' : 'minutes'}`
  }
</script>

<section class="sign-in-panel" aria-labelledby="github-sign-in-title">
  <div class="constellation" aria-hidden="true">✦</div>
  <div class="content">
    <p class="eyebrow">Native authorization</p>
    <h1 id="github-sign-in-title">Connect GitHub</h1>

    {#if status.state === 'idle'}
      <p>Sign in to sync private repositories and issue metadata. Stellr never asks you to paste a token.</p>
      <button disabled={busy} onclick={() => run(begin)}>Continue with GitHub</button>
    {:else if status.state === 'pending' || status.state === 'slow_down'}
      <p>Open GitHub, then enter this one-time code:</p>
      <strong class="user-code">{status.user_code}</strong>
      <a href={status.verification_uri} target="_blank" rel="noreferrer">Open GitHub device sign-in</a>
      <p class="expiry">Code expires in {minutes(status.expires_in_seconds)}.</p>
      {#if status.state === 'slow_down'}
        <p class="notice">GitHub asked Stellr to check less often. Your code is still valid.</p>
      {/if}
      <button class="secondary" disabled={busy} onclick={() => run(cancel)}>Cancel</button>
    {:else if status.state === 'denied'}
      <p>GitHub declined this request.</p>
      <button disabled={busy} onclick={() => run(begin)}>Try again</button>
    {:else if status.state === 'expired'}
      <p>This sign-in code expired.</p>
      <button disabled={busy} onclick={() => run(begin)}>Try again</button>
    {:else if status.state === 'cancelled'}
      <p>Sign-in was cancelled.</p>
      <button disabled={busy} onclick={() => run(begin)}>Try again</button>
    {:else if status.state === 'failed'}
      <p role="alert">{status.message}</p>
      <button disabled={busy} onclick={() => run(begin)}>Try again</button>
    {:else}
      <p>GitHub is connected.</p>
    {/if}
  </div>
</section>

<style>
  .sign-in-panel {
    position: absolute;
    inset: 0;
    z-index: 5;
    display: grid;
    place-items: center;
    overflow: auto;
    padding: 2rem;
    background:
      radial-gradient(circle at 68% 24%, color-mix(in oklch, var(--primary) 16%, transparent), transparent 34%),
      color-mix(in oklch, var(--background) 92%, transparent);
    backdrop-filter: blur(16px);
  }

  .constellation {
    position: absolute;
    top: 12%;
    left: 14%;
    color: var(--primary);
    font-size: clamp(3rem, 8vw, 7rem);
    opacity: 0.35;
  }

  .content {
    box-sizing: border-box;
    width: min(100%, 32rem);
    padding: clamp(1.5rem, 4vw, 3rem);
    border: 1px solid var(--border);
    border-radius: 1.25rem;
    background: color-mix(in oklch, var(--background) 94%, var(--muted));
    box-shadow: 0 1.5rem 5rem rgb(0 0 0 / 45%);
    text-align: center;
  }

  .eyebrow {
    margin: 0 0 0.5rem;
    color: var(--primary);
    font-size: 0.75rem;
    font-weight: 700;
    letter-spacing: 0.14em;
    text-transform: uppercase;
  }

  h1 {
    margin: 0;
    font-size: clamp(1.8rem, 5vw, 2.6rem);
  }

  p {
    color: var(--muted-foreground);
    line-height: 1.55;
  }

  .user-code {
    display: block;
    margin: 1.25rem 0;
    color: var(--foreground);
    font-family: ui-monospace, SFMono-Regular, Consolas, monospace;
    font-size: clamp(1.6rem, 6vw, 2.4rem);
    letter-spacing: 0.08em;
  }

  a,
  button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-height: 2.75rem;
    padding: 0 1.25rem;
    border: 1px solid var(--primary);
    border-radius: 0.65rem;
    color: var(--background);
    background: var(--primary);
    font-weight: 700;
    text-decoration: none;
    cursor: pointer;
  }

  button:disabled {
    cursor: progress;
    opacity: 0.6;
  }

  .secondary {
    color: var(--foreground);
    background: transparent;
  }

  .expiry {
    margin-bottom: 0.75rem;
    font-size: 0.85rem;
  }

  .notice {
    color: var(--foreground);
  }
</style>

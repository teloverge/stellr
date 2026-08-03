<script lang="ts">
  import ArrowSquareOutIcon from 'phosphor-svelte/lib/ArrowSquareOutIcon'
  import XIcon from 'phosphor-svelte/lib/XIcon'
  import { safeGithubIssueUrl } from './github-url'
  import { renderIssueMarkdown } from './markdown'
  import type { Star } from './model'
  import { openExternalUrl } from './native-shell'

  let {
    star,
    close,
    openExternal = openExternalUrl,
  }: {
    star: Star
    close: () => void
    openExternal?: typeof openExternalUrl
  } = $props()
  const githubUrl = $derived(safeGithubIssueUrl(star.url, star.number))
  let externalError = $state<string | null>(null)
  let opening = $state(false)

  async function openIssue(): Promise<void> {
    if (githubUrl === null || opening) return
    opening = true
    externalError = null
    try {
      await openExternal(githubUrl)
    } catch (error) {
      externalError = String(error)
    } finally {
      opening = false
    }
  }
</script>

<aside class="detail-pane" aria-label="Issue details">
  <header>
    <div class="title-block">
      <span class="issue-number">#{star.number}</span>
      <h2>{star.title}</h2>
    </div>
    <button class="close" type="button" aria-label="Close issue details" onclick={close}>
      <XIcon size={18} aria-hidden="true" />
    </button>
  </header>

  <div class="status" data-status={star.status}>{star.status.replaceAll('_', ' ')}</div>

  {#if star.milestone}
    <section>
      <h3>Milestone</h3>
      <p>{star.milestone}</p>
    </section>
  {/if}

  {#if star.labels.length > 0}
    <section>
      <h3>Labels</h3>
      <ul class="chips" aria-label="Labels">
        {#each star.labels as label (label)}
          <li>{label}</li>
        {/each}
      </ul>
    </section>
  {/if}

  {#if star.assignees.length > 0}
    <section>
      <h3>Assignees</h3>
      <ul class="chips" aria-label="Assignees">
        {#each star.assignees as assignee (assignee)}
          <li>@{assignee}</li>
        {/each}
      </ul>
    </section>
  {/if}

  {#if star.body}
    <div class="body">{@html renderIssueMarkdown(star.body)}</div>
  {/if}

  {#if githubUrl}
    <button
      class="external-link"
      type="button"
      data-external-url={githubUrl}
      disabled={opening}
      onclick={openIssue}
    >
      Open on GitHub
      <ArrowSquareOutIcon size={17} aria-hidden="true" />
    </button>
    {#if externalError}<p class="external-error" role="alert">{externalError}</p>{/if}
  {/if}
</aside>

<style>
  .detail-pane {
    box-sizing: border-box;
    height: 100%;
    overflow: auto;
    padding: 1.25rem;
    border: 1px solid var(--border);
    background: var(--surface);
    color: var(--foreground);
  }

  header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 1rem;
  }

  .title-block {
    min-width: 0;
  }

  .issue-number,
  h3 {
    color: var(--muted-foreground);
    font-size: 0.75rem;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  h2 {
    margin: 0.25rem 0 0;
    font-size: 1.2rem;
    line-height: 1.25;
  }

  h3,
  p {
    margin: 0;
  }

  section,
  .body {
    margin-top: 1.25rem;
  }

  .close {
    flex: 0 0 auto;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: var(--muted);
    color: var(--foreground);
    cursor: pointer;
    font: inherit;
    font-size: 1.25rem;
    line-height: 1;
    width: 2rem;
    height: 2rem;
  }

  .status {
    display: inline-block;
    margin-top: 0.75rem;
    padding: 0.25rem 0.5rem;
    border: 1px solid var(--border);
    border-radius: 999px;
    color: var(--primary);
    font-size: 0.75rem;
    font-weight: 700;
    text-transform: uppercase;
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
    list-style: none;
    margin: 0.4rem 0 0;
    padding: 0;
  }

  .chips li {
    padding: 0.2rem 0.45rem;
    border-radius: 999px;
    background: var(--muted);
    color: var(--muted-foreground);
    font-size: 0.75rem;
  }

  .body {
    overflow-wrap: anywhere;
    line-height: 1.55;
  }

  .body :global(pre) {
    overflow: auto;
    padding: 0.75rem;
    background: var(--muted);
  }

  .external-link {
    display: inline-flex;
    gap: 0.4rem;
    align-items: center;
    margin-top: 1.25rem;
    padding: 0;
    border: 0;
    color: var(--primary);
    background: transparent;
    font: inherit;
    cursor: pointer;
  }

  .external-error {
    color: var(--destructive);
  }
</style>

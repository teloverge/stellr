<script lang="ts">
  import { addSpace, refreshSpace, removeSpace } from './api'
  import type { SpaceModel } from './model'

  type Status = 'connecting' | 'open' | 'closed'
  type RowAction = 'refresh' | 'remove'

  interface RowMutation {
    request: (id: string) => Promise<Response>
    failureLabel: string
    complete: (id: string) => void
  }

  interface SidebarProps {
    spaces: SpaceModel[]
    activeSpaceId: string | null
    connectionStatus: Status
    select: (id: string) => void
    added: (id: string) => void
    removed: (id: string) => void
    addRequest?: typeof addSpace
    removeRequest?: typeof removeSpace
    refreshRequest?: typeof refreshSpace
  }

  let {
    spaces,
    activeSpaceId,
    connectionStatus,
    select,
    added,
    removed,
    addRequest = addSpace,
    removeRequest = removeSpace,
    refreshRequest = refreshSpace,
  }: SidebarProps = $props()

  let path = $state('')
  let repo = $state('')
  let adding = $state(false)
  let addError = $state<string | null>(null)
  let pendingRows = $state<Record<string, Partial<Record<RowAction, boolean>>>>({})
  let rowErrors = $state<Record<string, string | null>>({})
  const canAdd = $derived(Boolean(path.trim()) !== Boolean(repo.trim()))

  function syncAge(syncedAt: number | null): string {
    if (syncedAt === null) return 'Never synced'

    const elapsedSeconds = Math.max(0, Math.floor(Date.now() / 1000 - syncedAt))
    if (elapsedSeconds < 60) return 'Synced just now'

    const minutes = Math.floor(elapsedSeconds / 60)
    if (minutes < 60) return `Synced ${minutes} ${minutes === 1 ? 'minute' : 'minutes'} ago`

    const hours = Math.floor(minutes / 60)
    if (hours < 24) return `Synced ${hours} ${hours === 1 ? 'hour' : 'hours'} ago`

    const days = Math.floor(hours / 24)
    return `Synced ${days} ${days === 1 ? 'day' : 'days'} ago`
  }

  function connectionLabel(status: Status): string {
    return status[0].toUpperCase() + status.slice(1)
  }

  async function submitAdd(event: SubmitEvent): Promise<void> {
    event.preventDefault()
    if (!canAdd || adding) return

    adding = true
    addError = null
    try {
      const body = path.trim() ? { path: path.trim() } : { repo: repo.trim() }
      const response = await addRequest(body)
      if (!response.ok) {
        addError = (await response.text()) || `Add failed (${response.status})`
        return
      }

      const result = (await response.json()) as { id: string }
      path = ''
      repo = ''
      added(result.id)
    } catch (error) {
      addError = error instanceof Error ? error.message : String(error)
    } finally {
      adding = false
    }
  }

  function rowMutation(action: RowAction): RowMutation {
    if (action === 'refresh') {
      return { request: refreshRequest, failureLabel: 'Refresh', complete: () => undefined }
    }

    return { request: removeRequest, failureLabel: 'Remove', complete: removed }
  }

  async function mutateSpace(space: SpaceModel, action: RowAction): Promise<void> {
    if (pendingRows[space.id]?.[action]) return

    const mutation = rowMutation(action)
    pendingRows[space.id] = { ...pendingRows[space.id], [action]: true }
    rowErrors[space.id] = null
    try {
      const response = await mutation.request(space.id)
      if (!response.ok) {
        rowErrors[space.id] =
          (await response.text()) || `${mutation.failureLabel} failed (${response.status})`
        return
      }

      mutation.complete(space.id)
    } catch (error) {
      rowErrors[space.id] = error instanceof Error ? error.message : String(error)
    } finally {
      pendingRows[space.id] = { ...pendingRows[space.id], [action]: false }
    }
  }
</script>

<aside aria-label="Spaces">
  <header>
    <h1>Spaces</h1>
    <p class="connection-status">{connectionLabel(connectionStatus)}</p>
  </header>

  <form onsubmit={submitAdd}>
    <label>
      Local path
      <input name="path" bind:value={path} disabled={adding} />
    </label>
    <span class="or">or</span>
    <label>
      GitHub repository
      <input name="repo" bind:value={repo} disabled={adding} />
    </label>
    <button type="submit" disabled={!canAdd || adding}>{adding ? 'Adding…' : 'Add'}</button>
    {#if addError}<p class="form-error" aria-live="polite">{addError}</p>{/if}
  </form>

  <nav aria-label="Space list">
    <ul>
      {#each spaces as space (space.id)}
        <li class:active={space.id === activeSpaceId} data-space-row={space.id}>
          <button
            type="button"
            class="space-select"
            data-space-id={space.id}
            aria-current={space.id === activeSpaceId ? 'true' : undefined}
            onclick={() => select(space.id)}
          >
            <span class="space-name">{space.name}</span>
            <span class="space-repo">{space.repo}</span>
            <span class="space-age">{syncAge(space.synced_at)}</span>
            {#if space.stale}<span class="stale">Stale</span>{/if}
            {#if space.error}<span class="provider-error">{space.error}</span>{/if}
          </button>
          <div class="row-actions">
            <button
              type="button"
              aria-label={`Refresh ${space.name}`}
              disabled={pendingRows[space.id]?.refresh ?? false}
              onclick={() => mutateSpace(space, 'refresh')}
            >Refresh</button>
            <button
              type="button"
              aria-label={`Remove ${space.name}`}
              disabled={pendingRows[space.id]?.remove ?? false}
              onclick={() => mutateSpace(space, 'remove')}
            >Remove</button>
          </div>
          {#if rowErrors[space.id]}
            <p class="row-error" aria-live="polite">{rowErrors[space.id]}</p>
          {/if}
        </li>
      {/each}
    </ul>
  </nav>
</aside>

<style>
  aside {
    display: flex;
    min-height: 0;
    flex-direction: column;
    border-right: 1px solid var(--border);
    background: var(--background);
    color: var(--foreground);
  }

  header {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 1rem;
    padding: 1rem;
    border-bottom: 1px solid var(--border);
  }

  h1,
  p {
    margin: 0;
  }

  h1 {
    font-size: 1rem;
  }

  form {
    display: grid;
    gap: 0.5rem;
    padding: 1rem;
    border-bottom: 1px solid var(--border);
  }

  label {
    display: grid;
    gap: 0.25rem;
    color: var(--muted-foreground);
    font-size: 0.75rem;
  }

  input {
    min-width: 0;
    padding: 0.5rem;
    border: 1px solid var(--border);
    color: var(--foreground);
    background: var(--muted);
    font: inherit;
  }

  input:focus-visible,
  form button:focus-visible {
    outline: 2px solid var(--primary);
    outline-offset: 2px;
  }

  form button {
    padding: 0.5rem 0.75rem;
    border: 1px solid var(--border);
    color: var(--foreground);
    background: var(--muted);
    font: inherit;
    cursor: pointer;
  }

  form button:disabled {
    color: var(--muted-foreground);
    cursor: not-allowed;
  }

  .or {
    color: var(--muted-foreground);
    font-size: 0.75rem;
    text-align: center;
  }

  .form-error {
    color: var(--destructive);
    font-size: 0.75rem;
  }

  .connection-status,
  .space-repo,
  .space-age {
    color: var(--muted-foreground);
    font-size: 0.75rem;
  }

  nav {
    min-height: 0;
    overflow-y: auto;
  }

  ul {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  li {
    border-bottom: 1px solid var(--border);
  }

  li.active {
    background: var(--muted);
  }

  .space-select {
    display: grid;
    width: 100%;
    gap: 0.25rem;
    padding: 0.75rem 1rem;
    border: 0;
    color: inherit;
    background: transparent;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  .space-select:focus-visible {
    outline: 2px solid var(--primary);
    outline-offset: -2px;
  }

  .space-name {
    font-weight: 600;
  }

  .row-actions {
    display: flex;
    gap: 0.5rem;
    padding: 0 1rem 0.75rem;
  }

  .row-actions button {
    padding: 0.25rem 0.5rem;
    border: 1px solid var(--border);
    color: var(--muted-foreground);
    background: var(--background);
    font: inherit;
    font-size: 0.75rem;
    cursor: pointer;
  }

  .row-actions button:focus-visible {
    outline: 2px solid var(--primary);
    outline-offset: 2px;
  }

  .row-actions button:disabled {
    cursor: not-allowed;
  }

  .stale,
  .provider-error,
  .row-error {
    color: var(--destructive);
    font-size: 0.75rem;
  }

  .row-error {
    padding: 0 1rem 0.75rem;
  }
</style>

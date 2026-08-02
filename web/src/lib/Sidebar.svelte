<script lang="ts">
  import {
    addSpace,
    refreshSpace,
    removeSpace,
    type AddSpaceRequest,
  } from './api'
  import type { SpaceModel } from './model'

  type ConnectionStatus = 'connecting' | 'open' | 'closed'

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
  }: {
    spaces: SpaceModel[]
    activeSpaceId: string | null
    connectionStatus: ConnectionStatus
    select: (id: string) => void
    added: (id: string) => void
    removed: (id: string) => void
    addRequest?: typeof addSpace
    removeRequest?: typeof removeSpace
    refreshRequest?: typeof refreshSpace
  } = $props()
  let path = $state('')
  let repo = $state('')
  let adding = $state(false)
  let addError = $state<string | null>(null)
  let pendingRows = $state<Record<string, boolean>>({})
  let rowErrors = $state<Record<string, string | undefined>>({})
  let canAdd = $derived(
    !adding && (path.trim().length > 0) !== (repo.trim().length > 0),
  )

  function syncLabel(timestamp: number | null): string {
    if (timestamp === null) return 'Never synced'
    const iso = new Date(timestamp * 1000).toISOString()
    return `Synced ${iso.slice(0, 16).replace('T', ' ')} UTC`
  }

  function message(error: unknown): string {
    return error instanceof Error ? error.message : String(error)
  }

  async function submit(event: SubmitEvent): Promise<void> {
    event.preventDefault()
    if (!canAdd) return

    const trimmedPath = path.trim()
    const trimmedRepo = repo.trim()
    const body: AddSpaceRequest = trimmedPath ? { path: trimmedPath } : { repo: trimmedRepo }

    adding = true
    addError = null
    try {
      const response = await addRequest(body)
      if (!response.ok) {
        addError = (await response.text()) || `Add failed (${response.status})`
        return
      }

      const payload = (await response.json()) as { id?: unknown }
      if (typeof payload.id !== 'string' || payload.id.length === 0) {
        addError = 'Add response did not include a space ID'
        return
      }

      path = ''
      repo = ''
      added(payload.id)
    } catch (error) {
      addError = message(error)
    } finally {
      adding = false
    }
  }

  function setRowPending(id: string, pending: boolean): void {
    pendingRows = { ...pendingRows, [id]: pending }
  }

  function setRowError(id: string, error?: string): void {
    rowErrors = { ...rowErrors, [id]: error }
  }

  async function refresh(id: string): Promise<void> {
    if (pendingRows[id]) return
    setRowPending(id, true)
    setRowError(id)
    try {
      const response = await refreshRequest(id)
      if (!response.ok) {
        setRowError(id, (await response.text()) || `Refresh failed (${response.status})`)
      }
    } catch (error) {
      setRowError(id, message(error))
    } finally {
      setRowPending(id, false)
    }
  }

  async function remove(id: string): Promise<void> {
    if (pendingRows[id]) return
    setRowPending(id, true)
    setRowError(id)
    try {
      const response = await removeRequest(id)
      if (!response.ok) {
        setRowError(id, (await response.text()) || `Remove failed (${response.status})`)
        return
      }
      removed(id)
    } catch (error) {
      setRowError(id, message(error))
    } finally {
      setRowPending(id, false)
    }
  }
</script>

<aside class="sidebar">
  <header>
    <div>
      <span class="eyebrow">stellr</span>
      <h1>Spaces</h1>
    </div>
    <span class="connection" data-status={connectionStatus}>
      {connectionStatus === 'open'
        ? 'Live'
        : connectionStatus === 'connecting'
          ? 'Connecting'
          : 'Offline'}
    </span>
  </header>

  <nav aria-label="Spaces">
    <ul>
      {#each spaces as space (space.id)}
        <li data-space-row={space.id}>
          <button
            class="space-select"
            type="button"
            data-space-id={space.id}
            aria-current={space.id === activeSpaceId ? 'true' : undefined}
            onclick={() => select(space.id)}
          >
            <span class="space-heading">
              <strong>{space.name}</strong>
              {#if space.stale}<span class="stale">Stale</span>{/if}
            </span>
            <span class="repo">{space.repo}</span>
            <time datetime={space.synced_at === null ? undefined : new Date(space.synced_at * 1000).toISOString()}>
              {syncLabel(space.synced_at)}
            </time>
            {#if space.error}<span class="provider-error">{space.error}</span>{/if}
          </button>
          <div class="row-actions">
            <button
              type="button"
              data-action="refresh"
              disabled={pendingRows[space.id] === true}
              onclick={() => refresh(space.id)}
            >Refresh</button>
            <button
              type="button"
              data-action="remove"
              disabled={pendingRows[space.id] === true}
              onclick={() => remove(space.id)}
            >Remove</button>
          </div>
          {#if rowErrors[space.id]}
            <p class="mutation-error row-error" data-row-error>{rowErrors[space.id]}</p>
          {/if}
        </li>
      {/each}
    </ul>
  </nav>

  <form onsubmit={submit}>
    <h2>Add space</h2>
    <label>
      Local path
      <input name="path" bind:value={path} placeholder="D:\dev\project" />
    </label>
    <span class="or">or</span>
    <label>
      GitHub repository
      <input name="repo" bind:value={repo} placeholder="owner/repository" />
    </label>
    <button class="add" type="submit" disabled={!canAdd}>{adding ? 'Adding…' : 'Add'}</button>
    {#if addError}<p class="mutation-error" data-add-error>{addError}</p>{/if}
  </form>
</aside>

<style>
  .sidebar {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    border-right: 1px solid var(--border);
    background: var(--background);
    color: var(--foreground);
  }

  header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 0.75rem;
    padding: 1rem;
    border-bottom: 1px solid var(--border);
  }

  .eyebrow,
  .connection,
  .stale {
    color: var(--muted-foreground);
    font-size: 0.7rem;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  h1 {
    margin: 0.15rem 0 0;
    font-size: 1.1rem;
  }

  .connection[data-status='open'] {
    color: var(--primary);
  }

  nav {
    flex: 1 1 auto;
    min-height: 0;
    overflow-y: auto;
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 0.5rem;
  }

  li + li {
    margin-top: 0.35rem;
  }

  .space-select {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
    width: 100%;
    padding: 0.7rem;
    border: 1px solid var(--border);
    border-radius: 0.45rem;
    background: var(--background);
    color: var(--foreground);
    cursor: pointer;
    font: inherit;
    text-align: left;
  }

  .space-select[aria-current='true'] {
    border-color: var(--primary);
    background: var(--muted);
  }

  .space-heading {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
    width: 100%;
  }

  .repo,
  time,
  .provider-error {
    color: var(--muted-foreground);
    font-size: 0.75rem;
  }

  .stale,
  .provider-error {
    color: var(--destructive);
  }

  form {
    display: grid;
    gap: 0.5rem;
    padding: 1rem;
    border-top: 1px solid var(--border);
  }

  form h2 {
    margin: 0;
    font-size: 0.9rem;
  }

  label {
    display: grid;
    gap: 0.25rem;
    color: var(--muted-foreground);
    font-size: 0.75rem;
  }

  input {
    box-sizing: border-box;
    width: 100%;
    padding: 0.5rem;
    border: 1px solid var(--border);
    border-radius: 0.35rem;
    background: var(--background);
    color: var(--foreground);
    font: inherit;
  }

  .or {
    color: var(--muted-foreground);
    font-size: 0.7rem;
    text-align: center;
    text-transform: uppercase;
  }

  .add {
    align-items: center;
    background: var(--primary);
    color: var(--background);
    text-align: center;
  }

  .add:disabled {
    background: var(--muted);
    color: var(--muted-foreground);
    cursor: not-allowed;
  }

  .mutation-error {
    margin: 0;
    color: var(--destructive);
    font-size: 0.75rem;
  }

  .row-actions {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0.35rem;
    margin-top: 0.35rem;
  }

  .row-actions button {
    padding: 0.35rem 0.5rem;
    border: 1px solid var(--border);
    border-radius: 0.35rem;
    background: var(--background);
    color: var(--muted-foreground);
    cursor: pointer;
    font: inherit;
    font-size: 0.7rem;
  }

  .row-actions button:disabled {
    background: var(--muted);
    cursor: not-allowed;
  }

  .row-error {
    margin-top: 0.35rem;
  }
</style>

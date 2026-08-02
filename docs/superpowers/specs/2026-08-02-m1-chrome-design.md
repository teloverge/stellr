# M1 Chrome Design

**Issue:** #16 — M1 chrome: sidebar, star-map, detail pane, deep links

**Status:** Approved for implementation on 2026-08-02

## Goal

Complete the browser-hosted M1 experience around the existing live model and
star-map renderer: users can manage spaces, understand stale/offline state,
select an issue, inspect safe rich detail, and restore the same space and issue
from a pasted URL.

## Scope

This slice adds only the M1 browser chrome described by Issue #16:

- a left sidebar for space selection, add, remove, and refresh;
- the existing star map as the center stage;
- a responsive issue-detail pane;
- hash routing for the selected space and issue;
- visible sync age, stale state, provider error, and mutation errors;
- sanitized Markdown for issue bodies.

Tauri windows, `stellr://` protocol handling, issue editing, authentication
redesign, and additional providers remain out of scope.

## Chosen Approach

Use thin UI components around the seams that already exist. `Control` remains
authoritative for server snapshots, `StarMap` remains authoritative for canvas
rendering, and a small `Route` object owns URL state. This avoids introducing a
second application store for M1.

Two alternatives were rejected:

- A central application store would unify routing, mutations, selection, and
  model state, but duplicates the existing `Control` and adds architecture that
  M1 does not need.
- URL-only control would make the hash responsible for transient mutation and
  error state, which produces awkward navigation and poor error ownership.

## Components and Responsibilities

### `route.svelte.ts`

`Route` owns `#s=<spaceId>&i=<issueNumber>`.

- Parse the current hash at construction and on `hashchange`.
- Ignore an invalid or non-positive issue number.
- Preserve a valid space ID even when the corresponding model has not arrived.
- `go(space, issue?)` writes a canonical encoded hash.
- `destroy()` removes the listener.

Routing defaults are explicit:

- When the hash has no space, `App` selects the first available space.
- Adding a space routes to the new space with no selected issue.
- Removing the selected space routes to the next available space and clears the
  selected issue.
- If a routed issue does not exist in the routed space, the space remains active
  and the issue selection is cleared.

### `api.ts`

Expose thin cookie-authenticated wrappers around the existing server routes:

```ts
addSpace(body: { path?: string; repo?: string }): Promise<Response>
removeSpace(id: string): Promise<Response>
refreshSpace(id: string): Promise<Response>
```

The wrappers encode path segments and JSON bodies but do not own UI state.
Components read unsuccessful response text and present it beside the action that
failed.

### `Sidebar.svelte`

The sidebar receives spaces, the active space ID, navigation callbacks, and API
functions.

- Each space row shows name/repository, relative sync age, stale state, and the
  current provider error when present.
- Selecting a row changes only route state.
- Refresh and remove are explicit buttons on each row.
- The add form accepts either a local Windows path or an `owner/repo` slug,
  never both. The submit button is disabled until exactly one input is nonempty.
- Only the mutation being performed is disabled; errors stay local to the form
  or row.

### `DetailPane.svelte`

The detail pane receives one routed `Star` and renders:

- issue number, title, and status chip;
- milestone, labels, and assignees when present;
- a sanitized Markdown body using `marked` followed by `DOMPurify.sanitize`;
- an “Open on GitHub” link with `target="_blank"` and `rel="noreferrer"`;
- a close action that clears only the issue portion of the hash.

The sanitized HTML is the only intentional HTML-rendering seam. Raw issue body
HTML must never be inserted directly.

### `App.svelte`

`App` composes `Control`, `Route`, `Sidebar`, `StarMap`, and `DetailPane`.

- Derive the active space and routed star from the current model plus route.
- Apply the approved first-space fallback only after a model snapshot exists.
- A star click calls `route.go(activeSpace.id, issueNumber)`.
- Keep the map mounted when detail opens so its deterministic layout and camera
  continuity are preserved.
- Use the existing `decideDock('hybrid', ...)` seam with a `ResizeObserver`:
  wide/landscape cards place detail to the right; narrow/portrait cards place it
  below. The helper's hysteresis prevents resize flicker.

## Layout and Visual Rules

- The page fills `100dvh` and never gives the browser body its own scrollbars.
- The sidebar has a bounded width; its list scrolls independently.
- The map receives all remaining space.
- The detail pane has a comfortable bounded reading size and scrolls its own
  content.
- All chrome colors use the existing CSS tokens. No raw component colors are
  introduced.
- Stale and error signals use text as well as color so their meaning is not
  color-dependent.
- Existing CURRENT/READY map focus, black background, star grammar, and canvas
  interactions remain unchanged.

## Data Flow

1. Page load captures and removes the session token as today.
2. `Control` connects and receives the current model.
3. `Route` parses the hash independently of model arrival.
4. `App` resolves the routed space and issue, applying only the documented
   fallbacks.
5. Sidebar mutations call the HTTP API; the poller/control socket publishes the
   authoritative resulting model.
6. Star selection writes the hash; route changes derive the detail pane without
   copying issue state.

## Error and Offline Behavior

- Network or server mutation failures remain visible beside the initiating
  control and do not change the route optimistically.
- A successful add routes to the response's new space ID; the map appears when
  the authoritative control snapshot contains it.
- A stale cached space remains fully navigable and displays both a stale badge
  and its error text.
- An empty model renders the sidebar/add form and a clear empty-map message.
- Reconnection status may be shown in the sidebar footer but does not block
  cached navigation.

## Testing Strategy

Implementation follows red-green-refactor through public seams:

- Route tests cover empty, space-only, and space-plus-issue round trips;
  invalid issue values; hashchange; canonical encoding; and listener cleanup.
- API tests cover methods, encoded paths, JSON bodies, and credential behavior.
- Sidebar tests cover selection, exact-one-input validation, add/remove/refresh,
  local errors, stale badges, and sync metadata.
- Detail tests prove metadata rendering, safe outbound link attributes, Markdown
  rendering, and removal of script/event-handler payloads.
- App tests cover first-space fallback, added-space routing, selected-space
  removal, star-to-hash selection, pasted deep-link restoration, invalid routed
  issues, and right/bottom docking.
- Existing renderer and control suites remain unchanged and green.
- Final verification includes frontend test/check/build, native Rust
  fmt/test/clippy/build, and a headed Windows browser flow against the embedded
  server. The browser flow adds/selects a space, opens detail, confirms hash
  restoration, and observes stale/error presentation from cached data.

## Acceptance Mapping

- **Add a path or repository, render correct statuses and edges:** Sidebar/API
  integration plus the existing server poller and renderer adapter.
- **Open detail and restore a pasted URL:** Route, StarMap selection, and
  DetailPane composition.
- **Restart offline with visible stale data:** Existing cache/poller model plus
  explicit Sidebar stale/error presentation.


# Project Layout Cache and Transition Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make project selection immediate and responsive by computing uncached deterministic layouts in a cancellable worker, showing a timed first-load state, caching successful coordinates for the session, and restoring the last successful project after cancellation or critical failure.

**Architecture:** Add a deep `LayoutLoader` module whose small interface returns either cached coordinates or a cancellable pending result. A dedicated Vite module worker preserves the existing pure layout geometry off the main thread. The Svelte star-map wrapper owns loading/error presentation and request lifecycle, while `App.svelte` owns requested-versus-committed routing and rollback.

**Tech Stack:** Svelte 5 runes, TypeScript 6, Vite module workers, Vitest/jsdom, existing canvas `StarMap`, native Windows PowerShell, Vite+ (`vp`), Rust/Cargo workspace verification.

## Global Constraints

- Work only in `D:\tmp\stellr-issues-91-96` on `codex/issues-91-96-work-priority`; preserve the dirty primary checkout.
- Preserve the existing deterministic `computeLayout` output exactly; do not optimize or retune cluster geometry in this slice.
- Cache only successful coordinate snapshots for the current browser session, keyed by the exact existing `structureSignature`.
- An uncached request must run off the browser main thread and remain immediately cancellable.
- Route/sidebar selection changes optimistically; only success changes the committed project.
- Cancel and critical failure restore the last successful project. With no successful project, Cancel shows Canceled plus Retry and critical failure shows Error plus Retry.
- Ignore every canceled or superseded result, even when worker completion races termination.
- Maintain the append-only, newest-first `CHANGELOG.md` structure under `Unreleased`.
- Use native Windows commands and executables only; do not use WSL or Linux toolchains.
- Preserve the six existing uncommitted Rust review-fix files and stage only files belonging to each task.

---

## File Map

- Create `web/src/lib/starmap/layout-loader.ts`: cache, worker adapter interface, result validation, cancellation, and browser worker factory.
- Create `web/src/lib/starmap/layout-loader.test.ts`: real interface tests using a controlled worker adapter.
- Create `web/src/lib/starmap/layout.worker.ts`: worker entry point invoking the existing pure `computeLayout`.
- Modify `web/src/lib/starmap/starmap.ts`: accept already-computed positions when applying a new structure.
- Modify `web/src/lib/starmap/starmap.test.ts`: preserve synchronous fallback tests and prove supplied coordinates bypass computation.
- Create `web/src/lib/LayoutTransition.svelte`: accessible loading/stopwatch/Cancel and canceled/error/Retry presentation.
- Create `web/src/lib/LayoutTransition.test.ts`: copy, accessibility, actions, and stopwatch rendering.
- Modify `web/src/lib/StarMap.svelte`: coordinate async layout requests, timers, stale-result suppression, renderer application, and callbacks.
- Modify `web/src/lib/StarMap.test-host.svelte`: expose controlled prop transitions required by wrapper tests.
- Modify `web/src/lib/StarMap.test.ts`: loading, cache hit, timer, cancellation, retry, failure, and cleanup tests.
- Modify `web/src/App.svelte`: committed-project tracking and rollback policy.
- Modify `web/src/App.test.ts`: optimistic selection and application-level rollback tests with a controlled loader.
- Modify `CHANGELOG.md`: add the pending behavior to `Unreleased`.

---

### Task 1: Cancellable worker-backed session layout cache

**Files:**
- Create: `web/src/lib/starmap/layout-loader.ts`
- Create: `web/src/lib/starmap/layout-loader.test.ts`
- Create: `web/src/lib/starmap/layout.worker.ts`

**Interfaces:**
- Consumes: `LayoutNode`, `Point`, `computeLayout`, and `structureSignature` from `web/src/lib/starmap/layout.ts`.
- Produces:

```ts
export type LayoutPoints = Record<number, Point>

export type LayoutOutcome =
  | { kind: 'ready'; points: LayoutPoints }
  | { kind: 'cancelled' }
  | { kind: 'failed'; message: string }

export type LayoutLoad =
  | { kind: 'cached'; signature: string; points: LayoutPoints }
  | {
      kind: 'pending'
      signature: string
      result: Promise<LayoutOutcome>
      cancel(): void
    }

export interface LayoutRequester {
  load(nodes: LayoutNode[]): LayoutLoad
}

export interface LayoutWorkerPort {
  onmessage: ((event: MessageEvent<unknown>) => void) | null
  onerror: ((event: ErrorEvent) => void) | null
  postMessage(message: { nodes: LayoutNode[] }): void
  terminate(): void
}

export class LayoutLoader implements LayoutRequester {
  constructor(workerFactory: () => LayoutWorkerPort)
  load(nodes: LayoutNode[]): LayoutLoad
}

export const browserLayoutLoader: LayoutRequester
```

- [ ] **Step 1: Write failing cache and lifecycle tests**

Create a controlled `LayoutWorkerPort` that records `postMessage`/`terminate` and can emit arbitrary messages or errors. Add separate tests proving:

```ts
it('returns a defensive cached result after one successful worker layout')
it('does not invalidate coordinates for status-only data outside LayoutNode')
it('uses a new worker when structure or an orbit title changes')
it('terminates and resolves cancelled without caching')
it('turns worker errors and malformed coordinates into failed outcomes')
it('ignores a ready message that races after cancellation')
```

Use finite coordinates for every requested node as the validity rule. Prove defensive copying by mutating the first returned point and asserting the later cache hit retains the original value.

- [ ] **Step 2: Run the focused test and verify RED**

Run:

```powershell
vp exec vitest run src/lib/starmap/layout-loader.test.ts --reporter=verbose
```

Expected: FAIL because `layout-loader.ts` and its interface do not exist.

- [ ] **Step 3: Implement the minimal loader and worker**

Implement `LayoutLoader.load` as follows:

1. Compute `structureSignature(nodes)`.
2. Return cloned cached points synchronously when present.
3. Create one dedicated worker for a miss.
4. Return a pending result whose `cancel()` terminates once and resolves `{ kind: 'cancelled' }`.
5. On a valid success message, clone into the cache, terminate, and resolve ready with another clone.
6. On construction error, worker error, explicit failure message, missing node, or non-finite coordinate, terminate and resolve failed.
7. Guard every terminal path with one settled flag so racing messages do nothing.

The worker receives `{ nodes }`, calls `computeLayout(nodes)`, and posts either:

```ts
{ kind: 'ready', points }
{ kind: 'failed', message: String(error) }
```

Create the browser adapter with Vite's statically analyzable worker URL:

```ts
new Worker(new URL('./layout.worker.ts', import.meta.url), { type: 'module' })
```

- [ ] **Step 4: Run focused tests and frontend typecheck**

Run:

```powershell
vp exec vitest run src/lib/starmap/layout-loader.test.ts --reporter=verbose
vp run check
```

Expected: all focused tests PASS and Svelte/TypeScript report zero errors.

- [ ] **Step 5: Commit Task 1**

```powershell
git add -- web/src/lib/starmap/layout-loader.ts web/src/lib/starmap/layout-loader.test.ts web/src/lib/starmap/layout.worker.ts
git commit -m "feat(web): cache cancellable project layouts"
```

---

### Task 2: Let the renderer apply prepared deterministic coordinates

**Files:**
- Modify: `web/src/lib/starmap/starmap.ts:350-430`
- Modify: `web/src/lib/starmap/starmap.test.ts`

**Interfaces:**
- Consumes: `LayoutPoints` from Task 1.
- Produces this compatible renderer seam:

```ts
setModel(
  tickets: Ticket[],
  sessions?: Record<number, SessionState>,
  currentIssue?: number | null,
  preparedLayout?: LayoutPoints,
): void
```

The fourth argument applies only when the structure signature changes. Existing direct renderer consumers may omit it and retain the synchronous deterministic fallback.

- [ ] **Step 1: Write the failing renderer test**

Add a test that spies on `computeLayout`, supplies distinctive finite points through the fourth argument, and asserts:

```ts
expect(computeLayout).not.toHaveBeenCalled()
expect(sm.positions()).toEqual(preparedPoints)
```

Also repush status-only tickets without a fourth argument and prove the prepared positions remain unchanged.

- [ ] **Step 2: Run the focused renderer test and verify RED**

Run:

```powershell
vp exec vitest run src/lib/starmap/starmap.test.ts -t "prepared deterministic coordinates" --reporter=verbose
```

Expected: FAIL because `setModel` ignores or does not accept prepared positions.

- [ ] **Step 3: Implement the minimal compatible renderer change**

At the existing structure-change branch, replace the unconditional layout call with:

```ts
const pts = preparedLayout ?? computeLayout(layoutNodes)
```

Keep signature calculation, node construction, edge refresh, selection clearing, and camera refit unchanged. Do not move cache or worker knowledge into the renderer.

- [ ] **Step 4: Run the complete star-map renderer suite**

Run:

```powershell
vp exec vitest run src/lib/starmap/starmap.test.ts src/lib/starmap/layout.test.ts src/lib/starmap/cluster-layout.test.ts --reporter=verbose
```

Expected: all tests PASS with identical existing geometry assertions.

- [ ] **Step 5: Commit Task 2**

```powershell
git add -- web/src/lib/starmap/starmap.ts web/src/lib/starmap/starmap.test.ts
git commit -m "feat(web): apply prepared constellation layouts"
```

---

### Task 3: Present responsive timed loading, Cancel, and Retry

**Files:**
- Create: `web/src/lib/LayoutTransition.svelte`
- Create: `web/src/lib/LayoutTransition.test.ts`
- Modify: `web/src/lib/StarMap.svelte`
- Modify: `web/src/lib/StarMap.test-host.svelte`
- Modify: `web/src/lib/StarMap.test.ts`

**Interfaces:**
- Consumes: `LayoutRequester`, `LayoutLoad`, and `LayoutOutcome` from Task 1; prepared renderer coordinates from Task 2.
- `StarMap.svelte` adds optional injected `layout` with default `browserLayoutLoader` and callbacks:

```ts
layout?: LayoutRequester
ready?: (spaceId: string) => void
cancelled?: (spaceId: string) => void
failed?: (spaceId: string, message: string) => void
```

- `LayoutTransition.svelte` accepts:

```ts
kind: 'loading' | 'cancelled' | 'error'
projectName: string
elapsedSeconds?: number
message?: string
cancel?: () => void
retry?: () => void
```

- [ ] **Step 1: Write failing presentation tests**

Use fake timers and a controlled `LayoutRequester`. Add separate tests proving:

```ts
it('shows Charting, the first-load message, 0 seconds, and accessible Cancel on a miss')
it('increments visible elapsed seconds once per second without a live timer announcement')
it('renders a cached layout without showing the transition')
it('cancels the active request and reports the project id')
it('suppresses a superseded request result')
it('applies ready coordinates and reports the project id')
it('shows Error and Retry after an unhandled initial failure')
it('shows Canceled and Retry when cancellation does not navigate away')
it('clears its interval and cancels work when destroyed')
```

The controlled requester must return cached points or expose a pending outcome resolver without mocking the Svelte component itself.

- [ ] **Step 2: Run wrapper and transition tests and verify RED**

Run:

```powershell
vp exec vitest run src/lib/LayoutTransition.test.ts src/lib/StarMap.test.ts --reporter=verbose
```

Expected: FAIL because the transition module and asynchronous wrapper behavior are absent.

- [ ] **Step 3: Implement `LayoutTransition.svelte`**

Render the exact loading copy:

```text
Charting {projectName}...
First load may take a moment. {elapsedSeconds} seconds elapsed.
Cancel
```

Use `role="status"` and `aria-live="polite"` for the stable title/message. Put the ticking value in a separate element with `aria-live="off"`. Use real `<button type="button">` controls for Cancel and Retry. Preserve reduced-motion behavior by reusing the existing state-panel visual vocabulary without requiring an animated indicator.

- [ ] **Step 4: Implement the wrapper request state machine**

In `StarMap.svelte`:

1. Adapt `space` to tickets and compute layout nodes/signature.
2. Track the signature already applied to the renderer.
3. For a status-only update, call the existing three-argument `setModel` immediately.
4. For a new signature, cancel the prior request and call `layout.load(nodes)`.
5. Apply cached points synchronously and emit `ready(space.id)` without a loading flash.
6. For pending work, hide the old canvas from view and accessibility, show `LayoutTransition`, set elapsed seconds to zero, and start a one-second interval.
7. Apply ready points only when both request generation and current `space.id` still match, then stop the timer and emit `ready`.
8. On cancellation, stop the timer, enter canceled state, and emit `cancelled`.
9. On failure, stop the timer, enter error state, and emit `failed`.
10. Retry repeats the current uncached request with a new generation.
11. Component cleanup cancels the request, clears the interval, destroys the renderer, and prevents callbacks.

Keep the prior canvas mounted behind the transition so application rollback can restore it from cache without reconstructing the island.

- [ ] **Step 5: Run focused tests and typecheck**

Run:

```powershell
vp exec vitest run src/lib/LayoutTransition.test.ts src/lib/StarMap.test.ts --reporter=verbose
vp run check
```

Expected: all focused tests PASS and Svelte reports zero errors/warnings.

- [ ] **Step 6: Commit Task 3**

```powershell
git add -- web/src/lib/LayoutTransition.svelte web/src/lib/LayoutTransition.test.ts web/src/lib/StarMap.svelte web/src/lib/StarMap.test-host.svelte web/src/lib/StarMap.test.ts
git commit -m "feat(web): show cancellable project layout progress"
```

---

### Task 4: Roll back navigation to the last successful project

**Files:**
- Modify: `web/src/App.svelte:55-220,310-365`
- Modify: `web/src/App.test.ts`

**Interfaces:**
- Consumes the `ready`, `cancelled`, and `failed` callbacks from Task 3.
- Produces application transition handlers:

```ts
function committedLayout(spaceId: string): void
function cancelledLayout(spaceId: string): void
function failedLayout(spaceId: string, message: string): void
```

- [ ] **Step 1: Add a controlled loader to App test setup**

Give `App.svelte` one optional `layout?: LayoutRequester` prop defaulting to `browserLayoutLoader`, pass it to `StarMap`, and update `mountApp` to inject an immediate cached test requester by default. This preserves the existing synchronous routing tests while allowing new tests to control pending work.

- [ ] **Step 2: Write failing optimistic-transition and rollback tests**

Add separate tests proving:

```ts
it('selects the requested project immediately while its layout is pending')
it('records only a successfully rendered project as the rollback target')
it('Cancel restores the previous successful project')
it('a critical failure restores the previous project and shows a dismissible notice')
it('a superseded project cannot become committed')
it('initial cancellation stays selected with Canceled and Retry')
it('initial critical failure stays selected with Error and Retry')
```

For the primary sequence, resolve First successfully, click Second, assert `#s=second` and the Second sidebar row immediately, then cancel or fail and assert `#s=first` plus First active.

- [ ] **Step 3: Run the focused App tests and verify RED**

Run:

```powershell
vp exec vitest run src/App.test.ts -t "layout|Cancel|critical failure|requested project" --reporter=verbose
```

Expected: FAIL because App does not track a committed layout or respond to wrapper outcomes.

- [ ] **Step 4: Implement committed-selection policy**

Add `committedSpaceId` and a dismissible `layoutFailureNotice` to `App.svelte`.

- `committedLayout` records only the currently requested project and clears a matching failure notice.
- `cancelledLayout` ignores stale callbacks; when a committed project exists and differs from the failed request, route to it. Otherwise leave the current route so the wrapper's Canceled/Retry state remains visible.
- `failedLayout` follows the same rollback rule and records `Could not chart {project}: {message}`.
- Pass callbacks and the injected requester to `StarMap`.
- Render the notice through the existing `.route-notice` grammar and dismissal control.

Do not change `reconcileModel`, server authority, space persistence, issue routing, or ordinary detail-pane selection.

- [ ] **Step 5: Run all App, Sidebar, and wrapper tests**

Run:

```powershell
vp exec vitest run src/App.test.ts src/lib/Sidebar.test.ts src/lib/StarMap.test.ts src/lib/LayoutTransition.test.ts --reporter=verbose
vp run check
```

Expected: all tests PASS with zero Svelte errors/warnings.

- [ ] **Step 6: Commit Task 4**

```powershell
git add -- web/src/App.svelte web/src/App.test.ts
git commit -m "feat(web): restore the last charted project"
```

---

### Task 5: Release note, complete verification, and headed acceptance

**Files:**
- Modify: `CHANGELOG.md:3-10`

**Interfaces:**
- Consumes all prior task behavior.
- Produces final regression and real-browser evidence; no new runtime interface.

- [ ] **Step 1: Add the newest pending release note**

Insert this bullet at the top of `Unreleased`:

```markdown
- Made project changes immediate and responsive: first-time constellation layouts
  now show cancellable elapsed-time progress off the UI thread, while completed
  layouts are cached for instant return visits and failures restore the last
  successfully charted project.
```

- [ ] **Step 2: Run the complete frontend gates**

Run from `web`:

```powershell
vp run test
vp run check
vp run build
```

Expected: all Vitest files pass, Svelte reports zero errors/warnings, and Vite produces `web/dist`.

- [ ] **Step 3: Run native Rust workspace gates**

Run from the repository root after `web/dist` exists:

```powershell
cargo.exe fmt --all -- --check
cargo.exe clippy --workspace --all-targets --locked -j 1 -- -D warnings
cargo.exe test --workspace --locked -j 1
```

Expected: formatting, Clippy, and every locked workspace test pass. These commands also verify that the existing uncommitted Rust publication fix still compiles with the frontend bundle.

- [ ] **Step 4: Run a production-worker smoke**

Start the debug serve mode on an available native Windows address, authenticate in a headed browser, and verify:

1. Load Encrydle successfully.
2. Select Evolve and confirm its route/sidebar selection changes before layout completes.
3. Confirm `0 seconds` appears and increments while Chrome remains responsive.
4. Click Cancel and confirm Encrydle is restored promptly.
5. Select Evolve again and let it finish.
6. Return to Encrydle and then Evolve; confirm neither completed layout shows the first-load transition again.
7. Confirm browser console has no errors and project selection, pan, zoom, issue selection, priority visuals, and edge motion remain intact.

- [ ] **Step 5: Commit Task 5**

```powershell
git add -- CHANGELOG.md
git commit -m "docs: note responsive project transitions"
```

- [ ] **Step 6: Review final branch scope**

Run:

```powershell
git status --short
git diff 4264b29..HEAD --check
git log --oneline 4264b29..HEAD
```

Expected: only the six pre-existing Rust review-fix modifications remain uncommitted; all layout-transition files are committed, the diff is whitespace-clean, and commits are task-scoped.

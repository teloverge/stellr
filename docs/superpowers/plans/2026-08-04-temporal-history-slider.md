# Temporal Issue-History Slider Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a bottom-hovering, locally replayable issue-history timeline for creation, close/reopen, and milestone activity while keeping the constellation spatially stable and minimizing GitHub retrieval.

**Architecture:** Keep the current snapshot and the historical event ledger separate. Add provider-neutral temporal types to `stellr-core`, a transactional SQLite ledger in a new `stellr-history` crate, targeted GitHub timeline retrieval in `stellr-github`, authenticated summary/delta delivery in `stellr-server`, and a fully local Svelte projection/playback layer that feeds the existing renderer a structurally stable constellation plus temporal visual overlays.

**Tech Stack:** Rust 2024, Axum, Tokio, rusqlite (bundled SQLite), GitHub GraphQL, Svelte 5, TypeScript, Vitest, WireMock, native Windows PowerShell.

## Global Constraints

- Work only in native Windows tooling from `D:\tmp\stellr-issues-78-84`; do not use WSL or Linux paths.
- Implement and commit the dependency chain in order: #78, #79, #81, #82, #83, #84.
- Keep all six tickets on `codex/issues-78-84-temporal-history`; preserve the dirty primary checkout.
- Treat `docs/superpowers/specs/2026-08-04-temporal-history-slider-design.md` as the product contract.
- Use public behavior seams: domain projection, ledger methods, provider HTTP requests/results, authenticated HTTP responses, accessible DOM interactions, and renderer positions/camera/selection.
- Do not test private SQL tables or private canvas fields. Use a controllable clock for playback and provider timestamps for history.
- Run the smallest relevant Rust test target or Vitest file after each red/green cycle. Run full frontend and Rust suites once after #84.
- Keep `CHANGELOG.md` newest-first and append each pending change under `Unreleased`; do not edit shipped release sections.
- Scrubbing and playback must never call GitHub. Initial history is complete before controls enable. Completed history is never broadly fetched again.
- Never move the map camera, selection, detail pane, or star world coordinates as a result of temporal playback.

---

## Task 1: Issue #78 — Durable creation ledger and creation-time scrubbing

**Files:**

- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Create: `crates/history/Cargo.toml`
- Create: `crates/history/src/lib.rs`
- Create: `crates/history/src/store.rs`
- Create: `crates/history/tests/store_test.rs`
- Create: `crates/core/src/history.rs`
- Modify: `crates/core/src/lib.rs`
- Modify: `crates/core/src/model.rs`
- Modify: `crates/core/src/provider.rs`
- Modify: `crates/core/tests/model_contract_test.rs`
- Modify: `crates/github/src/sync.rs`
- Modify: `crates/github/tests/sync_test.rs`
- Modify: `crates/server/Cargo.toml`
- Modify: `crates/server/src/state.rs`
- Modify: `crates/server/src/poll.rs`
- Modify: `crates/server/src/routes.rs`
- Modify: `crates/server/tests/api_test.rs`
- Modify: `crates/app/Cargo.toml`
- Modify: `crates/app/src/runtime.rs`
- Modify: `web/src/lib/model.ts`
- Create: `web/src/lib/history.ts`
- Create: `web/src/lib/history.test.ts`
- Create: `web/src/lib/history-api.ts`
- Create: `web/src/lib/TemporalTimeline.svelte`
- Create: `web/src/lib/TemporalTimeline.test.ts`
- Modify: `web/src/App.svelte`
- Modify: `web/src/App.test.ts`
- Modify: `CHANGELOG.md`

### 1.1 Define provider-neutral history contracts

- [ ] Add a failing serialization/projection contract test for `HistoryEvent`, `HistoryEventKind::IssueCreated`, `HistorySummary`, `HistoryImportState`, `IssueSyncMetadata`, `ProviderSnapshot`, `HistoryPageRequest`, and `HistoryPage`.
- [ ] Add `crates/core/src/history.rs` with stable repository/issue/event IDs, UTC epoch-second timestamps, deterministic `(occurred_at, provider_event_id)` ordering, minimal milestone payloads, progress/diagnostic fields, and ledger sequence numbers.
- [ ] Extend `Provider` with a default `fetch_snapshot` that wraps existing `fetch`, plus a default unsupported targeted-history method so existing provider stubs remain source-compatible.
- [ ] Add `history: HistorySummary` to `SpaceModel` and use a default unavailable summary for old/current-only models.
- [ ] Run `cargo.exe test -p stellr-core --locked`.

### 1.2 Add the transactional SQLite ledger

- [ ] Add failing `store_test` cases through `HistoryStore` for fixed transactional migration, bound repository lookup, deterministic event ordering, unique repository/event identity, repeated-page idempotence, atomic event/cursor/progress commits, summary calculation, and repository deletion.
- [ ] Create `stellr-history` using `rusqlite` with bundled SQLite. Store repository/issue identities, events, per-issue cursors and `updatedAt`, import state/progress, verified-through, retry evidence, and a monotonic ledger sequence; never store tokens, bodies, comments, headers, or raw responses.
- [ ] Keep each page checkpoint in one transaction and expose `events_after(repository, sequence)` rather than SQL details.
- [ ] Run `cargo.exe test -p stellr-history --locked --test store_test`.

### 1.3 Piggyback stable IDs and timestamps on the current issue request

- [ ] Add failing WireMock assertions that the existing paginated issue-list GraphQL query requests repository ID and issue `id`, `createdAt`, and `updatedAt` without adding a second repository-wide request.
- [ ] Update `GithubProvider` to return a `ProviderSnapshot` with current `RawIssue` values and the minimal history index metadata. Preserve `fetch` behavior for callers that only need current state.
- [ ] Normalize a deterministic creation event ID from the provider issue ID plus `issue_created`.
- [ ] Run `cargo.exe test -p stellr-github --locked --test sync_test`.

### 1.4 Import creations after the first successful current sync

- [ ] Add failing server polling tests for creating the history database after a successful snapshot, recording all issue-created events, publishing determinate import progress, marking the repository complete only after the frozen issue set is checkpointed, and leaving the current map usable throughout.
- [ ] Construct `HistoryStore` beside the existing JSON cache in runtime state. Start serialized low-priority history work only after successful current sync; use the issue metadata already returned by that sync.
- [ ] Make failed/offline creation import retain checkpoints and expose an incomplete diagnostic without making the current model stale solely because history is delayed.
- [ ] Run the focused server polling test target.

### 1.5 Deliver authenticated history and scrub creation visibility locally

- [ ] Add failing API tests for `GET /api/spaces/{id}/history?after=<sequence>`: known-space validation, existing token/cookie/bearer protection, complete ordered events, delta sequence, compact summary, no path/raw-provider leakage, and `404` for an unknown space.
- [ ] Add the protected route and include only `HistorySummary` in whole-model snapshots.
- [ ] Add failing frontend tests for fetching once after completion, retaining events in memory, projecting issues whose creation is at or before the selected instant, defaulting to Now, and making repeated slider input without any network call.
- [ ] Implement `projectTemporalSpace(current, events, playhead)` as a pure adapter. Keep every current star in a structural renderer input and mark future stars hidden through a temporal overlay so creation-time scrubbing does not change layout signatures.
- [ ] Add the initial bottom-floating date/range control, disabled progress text while incomplete, `No issue history` for an empty ledger, Return to Now behavior, and reserved map inset.
- [ ] Assert with the renderer’s public `positions()`, `camera()`, and selection callbacks that scrubbing creation events changes visibility only.
- [ ] Run `npm.exe --prefix web test -- history.test.ts TemporalTimeline.test.ts App.test.ts StarMap.test.ts starmap/starmap.test.ts`.
- [ ] Update `CHANGELOG.md`, review `git diff 08bbaa8...HEAD`, and commit `feat(history): scrub issue creation from local ledger` with `Closes #78`.

---

## Task 2: Issue #79 — Close and reopen history

**Files:**

- Modify: `crates/core/src/history.rs`
- Modify: `crates/core/tests/model_contract_test.rs`
- Create: `crates/github/src/history.rs`
- Modify: `crates/github/src/lib.rs`
- Modify: `crates/github/src/sync.rs`
- Create: `crates/github/tests/history_test.rs`
- Modify: `crates/history/src/store.rs`
- Modify: `crates/history/tests/store_test.rs`
- Modify: `crates/server/src/poll.rs`
- Modify: `crates/server/tests/api_test.rs`
- Modify: `web/src/lib/history.ts`
- Modify: `web/src/lib/history.test.ts`
- Modify: `web/src/lib/starmap/model.ts`
- Modify: `web/src/lib/starmap/adapt.ts`
- Modify: `web/src/lib/starmap/theme.ts`
- Modify: `web/src/lib/starmap/starmap.test.ts`
- Modify: `CHANGELOG.md`

### 2.1 Retrieve only lifecycle history per issue

- [ ] Add failing WireMock tests for a bounded, cursor-paginated single-issue timeline query that selects `ClosedEvent` and `ReopenedEvent`, retains provider event IDs/timestamps, ignores unrelated items, and surfaces malformed/ambiguous pages with issue/cursor/stage context.
- [ ] Implement `TemporalHistorySource` behavior on `GithubProvider` using the existing GraphQL client and typed error path. Return rate-limit/retry evidence without unbounded retrying.
- [ ] Run `cargo.exe test -p stellr-github --locked --test history_test`.

### 2.2 Checkpoint and replay lifecycle transitions

- [ ] Add failing ledger/import tests for same-page and repeated-page idempotence, atomic cursor advancement, restart resume, stable same-timestamp ordering, and no storage of untracked events.
- [ ] Extend the initial importer to retrieve one issue page stream at a time after recording creation, checkpoint each page, yield between requests, and complete only when all frozen issues share a verified boundary.
- [ ] Add failing pure frontend projection cases for exact close/reopen boundaries, repeated cycles, same-timestamp provider-ID ordering, and historical neutral-open/closed styles.
- [ ] Implement historical `open` versus `resolved` rendering without leaking current blocked/frontier/claimed/assignee status. Restore the full live status only at Now.
- [ ] Assert positions, camera, selected issue, and open detail remain unchanged across lifecycle frames.
- [ ] Run focused history, server, and frontend tests.
- [ ] Update `CHANGELOG.md`, self-review the issue delta, and commit `feat(history): replay issue close and reopen activity` with `Closes #79` and `Refs #78`.

---

## Task 3: Issue #81 — Milestone transition history without movement

**Files:**

- Modify: `crates/core/src/history.rs`
- Modify: `crates/github/src/history.rs`
- Modify: `crates/github/tests/history_test.rs`
- Modify: `crates/history/src/store.rs`
- Modify: `crates/history/tests/store_test.rs`
- Modify: `web/src/lib/history.ts`
- Modify: `web/src/lib/history.test.ts`
- Modify: `web/src/lib/starmap/model.ts`
- Modify: `web/src/lib/starmap/starmap.ts`
- Modify: `web/src/lib/starmap/starmap.test.ts`
- Modify: `web/src/lib/StarMap.svelte`
- Modify: `web/src/lib/StarMap.test.ts`
- Modify: `CHANGELOG.md`

### 3.1 Normalize milestone assignments, moves, and removals

- [ ] Add failing provider tests for `MilestonedEvent` and `DemilestonedEvent`, stable milestone identity/title payloads, move ordering, removal to none, and filtering milestone rename noise.
- [ ] Extend domain and ledger payloads with previous/result milestone identities and display titles while preserving only minimal values.
- [ ] Run focused core, provider, and ledger tests.

### 3.2 Render temporal milestone membership as an overlay

- [ ] Add failing pure projection tests for creation-with-milestone, assignment, move, removal, and exact timestamp boundaries.
- [ ] Extend renderer tickets with current temporal milestone membership and visibility metadata, but keep `structureSignature` and `computeLayout` dependent only on the full present-day issue/edge/parent topology.
- [ ] Add a renderer overlay for milestone membership/hulls that can reshape around fixed star coordinates. Treat absent/no-milestone consistently and escape all titles as text.
- [ ] Add wrapper tests proving milestone changes update the renderer model without selection echo, camera changes, or world-coordinate changes.
- [ ] Run focused frontend tests.
- [ ] Update `CHANGELOG.md`, self-review the issue delta, and commit `feat(history): replay milestone membership without moving stars` with `Closes #81` and `Refs #79`.

---

## Task 4: Issue #82 — Full-history playback, ticks, and speed control

**Files:**

- Modify: `web/src/lib/history.ts`
- Modify: `web/src/lib/history.test.ts`
- Create: `web/src/lib/playback.ts`
- Create: `web/src/lib/playback.test.ts`
- Modify: `web/src/lib/TemporalTimeline.svelte`
- Modify: `web/src/lib/TemporalTimeline.test.ts`
- Modify: `web/src/lib/StarMap.svelte`
- Modify: `web/src/lib/StarMap.test.ts`
- Modify: `web/src/App.svelte`
- Modify: `web/src/App.test.ts`
- Modify: `CHANGELOG.md`

### 4.1 Implement a deterministic playback clock

- [ ] Add failing fake-clock tests mapping the complete verified history to exactly 30 seconds at 1x, continuing from the current playhead, restarting at the first event when Play is pressed at Now, pausing at Now, and applying every crossed event in stable order after a slow frame.
- [ ] Implement a pure playback controller for 0.5x, 1x, 2x, and 4x. Keep the playhead in absolute provider time and never query during animation.
- [ ] Run `npm.exe --prefix web test -- playback.test.ts`.

### 4.2 Add accessible event ticks and controls

- [ ] Add failing component tests for control order (`date | slider/ticks | Play/Pause | speed`), proportional event positions, same-timestamp clustering, dense pixel clustering, tooltip event ordering, click/focus navigation, Left/Right distinct-event navigation, Home/End, play/pause, and speed cycling.
- [ ] Implement Play/Pause to the right of the slider and a visible/focusable speed button. At narrow widths move the date above the slider and retain a usable touch target.
- [ ] Add reduced-motion-aware event pulses and bounded captions for affected stars; group bursts without panning, changing selection, or opening detail.
- [ ] Assert playback does not change renderer coordinates/camera/selection and that current dependency edges appear only between temporally visible endpoints with subdued contextual styling.
- [ ] Run focused timeline, App, wrapper, and renderer tests.
- [ ] Update `CHANGELOG.md`, self-review the issue delta, and commit `feat(history): play complete history with event ticks` with `Closes #82` and `Refs #81`.

---

## Task 5: Issue #83 — Delta-only background synchronization

**Files:**

- Modify: `crates/core/src/history.rs`
- Modify: `crates/github/src/history.rs`
- Modify: `crates/github/tests/history_test.rs`
- Modify: `crates/history/src/store.rs`
- Modify: `crates/history/tests/store_test.rs`
- Modify: `crates/server/src/poll.rs`
- Modify: `crates/server/tests/api_test.rs`
- Modify: `web/src/lib/history-api.ts`
- Modify: `web/src/lib/history.ts`
- Modify: `web/src/lib/history.test.ts`
- Modify: `web/src/lib/TemporalTimeline.svelte`
- Modify: `web/src/lib/TemporalTimeline.test.ts`
- Modify: `web/src/App.svelte`
- Modify: `web/src/App.test.ts`
- Modify: `CHANGELOG.md`

### 5.1 Detect only relevant changed issues

- [ ] Add failing importer/provider tests proving unchanged `updatedAt` values cause zero timeline requests, new issues get one targeted import, changed issues resume after their saved cursor, and a close/reopen cycle between ordinary polls is discovered.
- [ ] Implement post-completion synchronization from the metadata already piggybacked on ordinary current snapshots. Serialize history requests to one page stream, respect provider reset/retry evidence, and checkpoint conservative resume times.
- [ ] Add catch-up verification after an initial frozen import before declaring the ledger complete.
- [ ] Prove completed pages/issues/repositories are not broadly refetched after restart, reconnect, playback, or ordinary unchanged polls.
- [ ] Run focused provider, ledger, and server tests.

### 5.2 Deliver and merge local deltas

- [ ] Add failing API/frontend tests for `after=<last_sequence>` returning only later events plus the new summary, client-side de-duplication by sequence/event ID, and no loss on reconnect.
- [ ] At Now, merge a verified delta and advance the live projection automatically. In the past, keep the playhead pinned and show an accessible `New activity` action that returns to Now.
- [ ] Make ordinary current-refresh errors preserve local history and never prune it.
- [ ] Run focused API and frontend tests.
- [ ] Update `CHANGELOG.md`, self-review the issue delta, and commit `feat(history): follow activity with delta-only synchronization` with `Closes #83` and `Refs #82`.

---

## Task 6: Issue #84 — Offline, delayed, accessible native hardening

**Files:**

- Modify: `crates/history/src/store.rs`
- Modify: `crates/history/tests/store_test.rs`
- Modify: `crates/server/src/poll.rs`
- Modify: `crates/server/src/routes.rs`
- Modify: `crates/server/tests/api_test.rs`
- Modify: `crates/app/src/runtime.rs`
- Modify: `crates/app/tests/runtime_test.rs`
- Modify: `web/src/lib/TemporalTimeline.svelte`
- Modify: `web/src/lib/TemporalTimeline.test.ts`
- Modify: `web/src/App.svelte`
- Modify: `web/src/App.test.ts`
- Modify: `CHANGELOG.md`

### 6.1 Harden storage, failure, and offline semantics

- [ ] Add failing tests for migration rollback preserving the prior usable ledger, offline resume from the last page checkpoint, complete-ledger playback with an unreachable provider, delayed incremental history capped at `History through DATE`, explicit retry, rate-limit reset display, space-removal history deletion, and refresh errors preserving history.
- [ ] Implement diagnostic state transitions without inventing missing intervals. Return to Now must always restore the current snapshot even when verified history is delayed.
- [ ] Validate requested repository IDs against known spaces and keep all SQLite statements bound.
- [ ] Run focused Rust tests.

### 6.2 Complete accessibility and responsive behavior

- [ ] Add failing component tests for readable date/value text, persistent accessible names, visible focus affordances, focusable tick tooltips, keyboard navigation, status distinctions not conveyed by color alone, reduced-motion behavior, narrow layout, and sanitized event/milestone strings.
- [ ] Implement the remaining accessibility/responsive states and ensure the overlay reserves bottom inset without changing graph world coordinates.
- [ ] Run focused frontend tests.

### 6.3 Native Windows acceptance and final verification

- [ ] Build `web/dist` before Rust embedding: `npm.exe --prefix web run build`.
- [ ] Run `npm.exe --prefix web run check`.
- [ ] Run `npm.exe --prefix web test` once for the final frontend suite.
- [ ] Run `cargo.exe fmt --all -- --check`.
- [ ] Run `cargo.exe clippy --workspace --all-targets --locked -- -D warnings`.
- [ ] Run `cargo.exe test --workspace --locked` once for the final Rust suite.
- [ ] Run `cargo.exe build --workspace --locked`.
- [ ] Exercise native runtime acceptance against a temporary history root: complete import, scrub all four event kinds, play at all speeds, restart offline, resume an interrupted import, and verify request counts show no playback calls or completed repository-wide refetch.
- [ ] Verify the detail pane, selected issue, camera pose, and renderer `positions()` remain stable throughout a complete replay.
- [ ] Update `CHANGELOG.md` under `Unreleased` only.
- [ ] Run the required two-axis code review against fixed point `08bbaa8`: repository standards and the six issue/spec contracts. Fix every valid finding and rerun affected gates.
- [ ] Commit `feat(history): harden native temporal playback` with `Closes #84` and `Refs #83`.
- [ ] Confirm `git status --short --branch` is clean and report the branch/commit chain without pushing, opening a PR, or closing tracker items unless separately authorized.

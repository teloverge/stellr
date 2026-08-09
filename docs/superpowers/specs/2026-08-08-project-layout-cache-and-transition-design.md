# Project Layout Cache and Transition Design

**Date:** 2026-08-08
**Status:** Approved

## Problem

Changing the selected project synchronously recomputes its deterministic star-map
layout on the browser main thread. Real-data measurements put this work at about
2.5 to 3.7 seconds for Idle Mind, 21 to 22 seconds for Encrydle, and 85 to 94
seconds for Evolve. The ordinary force layout takes under 60 milliseconds; the
subissue-cluster candidate search consumes almost all remaining time. During that
search the browser cannot paint, update controls, or accept another selection.

The layout is deterministic for a structure signature, but the renderer remembers
only the currently displayed signature. Returning to a previously viewed project
therefore repeats the same expensive calculation.

## Product Decisions

- Selecting a project updates the route and selected sidebar item immediately.
- An uncached project displays a textual loading state before layout begins.
- Layout computation runs outside the browser main thread.
- A completed layout is cached for the app session by its exact structure
  signature.
- The loading state includes a whole-seconds stopwatch and a Cancel action.
- Cancel restores the last successfully displayed project.
- A critical layout failure also restores the last successfully displayed
  project.
- A stale result from a canceled or superseded request can never replace the
  current project.

## Transition Model

The application distinguishes three project identities:

- **requested project:** the project named by the current route and highlighted
  in the sidebar;
- **loading project:** the uncached project whose layout request is active;
- **committed project:** the most recent project whose constellation rendered
  successfully.

Clicking a project immediately changes the requested project. A cache hit commits
and displays the project without showing a loading transition. A cache miss keeps
the new route and selection visible while the map region displays its loading
state and starts a background layout request.

Only the active request may commit. Each request has an opaque generation. Success,
failure, and cancellation handlers compare their generation with the active one
before changing UI state. Selecting a third project while another project loads
terminates the obsolete worker, clears its stopwatch, and starts or restores the
new request without briefly displaying the obsolete result.

On successful layout, the application displays the constellation, records the
requested project as committed, and removes the loading state. On Cancel or a
critical worker/layout failure, it terminates the active work and routes back to
the committed project. A critical failure also presents a dismissible error notice;
user cancellation is not reported as an error.

If initial startup has no committed project to restore, a critical failure leaves
the application shell and requested selection available with an error state and a
Retry action. Cancel remains available during an initial layout, stops the work,
and leaves the requested selection in a canceled state with a Retry action because
there is no successful project to restore.

## Loading State

The map region displays this content for an uncached layout:

> Charting {project name}…
>
> First load may take a moment. {N} seconds elapsed.
>
> Cancel

The stopwatch starts at `0 seconds`, increments once per elapsed whole second, and
stops on success, cancellation, supersession, failure, or component destruction.
The project name and primary loading message use a polite live region. The changing
stopwatch text is visible but does not announce every tick to screen readers. Cancel
is a keyboard-accessible button and receives a descriptive accessible name.

The loading state must be painted before expensive work begins. Worker execution
keeps the message, stopwatch, and Cancel action responsive throughout the first
layout.

## Layout Worker and Cache

A small layout module owns one deep interface: request deterministic coordinates
for a set of layout nodes. Its implementation hides worker lifecycle, request
generation, session caching, defensive coordinate copies, cancellation, and typed
failure results from the Svelte wrapper and canvas renderer.

The module computes the existing `structureSignature` before dispatch. Its cache is
keyed by that exact signature, which already includes node numbers, workflow edges,
valid parent topology, and orbit-label titles. Status, assignment, work priority,
ordinary selection, and session animation do not invalidate coordinates. A topology
or orbit-title change produces a different signature and triggers a new layout.

The cache lives only for the current application session. It does not write to disk
and does not change server models. Entries hold immutable coordinate snapshots; a
caller receives a defensive copy so renderer state cannot corrupt a later cache hit.

Each cache miss uses a dedicated module worker created through Vite's worker URL
support. The worker imports the existing pure deterministic layout implementation,
computes coordinates, and posts either a typed success or a serializable failure.
Cancel and supersession terminate that worker, giving immediate cancellation even
while the collision search is CPU-bound.

## Renderer and Application Responsibilities

The layout module owns coordinate calculation and reuse. The star-map renderer
continues to own canvas nodes, edges, camera fit, selection, animation, and paint.
It receives precomputed coordinates when a structure changes instead of invoking
the expensive layout implementation itself. Status-only pushes continue using the
existing no-movement fast path.

The Svelte star-map wrapper owns presentation of loading and initial error states,
the stopwatch lifecycle, and the active layout request. It reports `ready`,
`cancel`, and `critical failure` transition outcomes through a small interface.

The application owns requested-versus-committed project routing. It records the
last successful selection and performs rollback after cancellation or critical
failure. This keeps navigation policy out of the renderer and worker.

## Error Handling

- Worker construction, execution, message decoding, or layout exceptions become a
  typed critical failure.
- A critical failure with a committed project restores that project and shows a
  dismissible notice naming the project that could not be charted.
- A critical failure without a committed project keeps the shell usable and offers
  Retry.
- Cancel and supersession terminate work and cannot populate the cache.
- A result whose generation is no longer active is ignored even if termination
  raced with worker completion.
- A malformed coordinate result never enters the cache or renderer.
- Cache hits are validated for the requested signature before use.
- Worker and timer resources are released on every terminal path and component
  destruction.

## Testing

Use test-driven development at the public seams:

1. Layout-module tests prove a first request invokes the worker and a second request
   for the same signature returns defensive cached coordinates without invoking it.
2. Cache tests prove status-only changes reuse coordinates, structural and orbit-title
   changes miss, and failures or cancellations are not cached.
3. Request-lifecycle tests prove cancellation terminates work, superseded results are
   ignored, malformed results fail, and only the active generation can commit.
4. Star-map wrapper tests prove optimistic loading copy, `0 seconds`, whole-second
   stopwatch progression, accessible Cancel, cleanup, ready display, and initial
   Retry behavior.
5. Application tests prove immediate route/sidebar selection, committed-selection
   tracking, Cancel rollback, critical-failure rollback and notice, and startup
   failure without a rollback target.
6. Renderer tests prove precomputed positions produce the existing deterministic
   coordinates and preserve status-only no-movement, selection, camera, and edge
   behavior.
7. Run the complete frontend test, Svelte check, production web build, Rust format,
   Clippy, and locked workspace test gates on native Windows.
8. Verify in a headed browser that a first Evolve visit shows a responsive stopwatch
   and Cancel, a canceled visit restores the previous project, and a completed return
   visit uses the cache without another loading transition.

## Release Notes

Add an `Unreleased` changelog entry stating that project changes now select
immediately, show cancellable timed layout progress on first load, and reuse cached
constellation coordinates on later visits.

## Scope

This slice changes browser-side project transitions and deterministic layout
execution. It does not change GitHub synchronization, server model publication,
project persistence, graph topology, layout geometry, work-priority semantics,
installed application files, or cross-session disk caching. It does not optimize the
cluster scoring algorithm itself; the worker preserves its exact output while session
caching prevents repeated computation.

## Acceptance Criteria

- Clicking a configured project immediately updates the selected route and sidebar.
- Every uncached layout displays the approved project-specific loading message,
  visible whole-seconds stopwatch, and Cancel button before computation begins.
- The UI, stopwatch, and Cancel action remain responsive while layout runs.
- Cancel terminates the pending calculation and restores the last successfully
  displayed project. Without a successful project, it stops at a canceled state
  with Retry.
- Critical failure restores the last successfully displayed project and presents a
  useful error notice.
- Initial failure without a previous successful project presents Error and Retry
  without breaking the shell.
- Returning to an unchanged, successfully laid-out project does not recompute layout
  or display the first-load transition.
- Structural or orbit-title changes invalidate the relevant cached coordinates.
- Stale, canceled, failed, or malformed results never reach the canvas or cache.
- Existing deterministic geometry and complete native-Windows verification gates
  remain green.

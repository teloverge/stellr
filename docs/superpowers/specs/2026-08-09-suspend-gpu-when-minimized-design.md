# Suspend GPU Rendering While Minimized Design

**Status:** Approved by the project owner on 2026-08-09

## Goal

Stop Stellr's continuous canvas rendering while the native application window
is minimized, while preserving a very-low-rate data refresh and restoring the
same visual state immediately when the window returns.

## Behavior Contract

Minimizing the native Stellr window cancels the star-map renderer's outstanding
animation frame and schedules no replacement frame. Camera position, camera
goal, selection, model data, layout, flare state, and animation state remain in
memory; minimization does not destroy or remount the renderer.

Restoring the window resets the renderer's frame-time baseline and schedules
exactly one animation loop. The first restored frame paints the current model
without advancing animations by the full minimized duration. Repeated minimize
or restore notifications are idempotent and cannot create duplicate loops.

Browser-hosted Stellr applies the equivalent behavior when the document becomes
hidden or visible. This gives browser tabs the same power-saving behavior
without changing the browser transport or native-shell contract.

## Renderer Boundary

Add explicit `suspend()` and `resume()` lifecycle methods to the imperative
star-map renderer:

- `suspend()` records the suspended state, cancels a scheduled animation frame,
  and clears the frame handle;
- `resume()` does nothing unless suspended, clears the suspended state, resets
  the last-frame timestamp, and schedules one frame when a canvas context is
  mounted;
- the render callback schedules its successor only while active;
- the animation clock advances by the bounded per-frame delta rather than raw
  wall time, so it remains frozen for the entire suspension;
- `destroy()` remains terminal and cleans up a scheduled frame in either state.

The Svelte wrapper suspends a new renderer before mounting it, so mounting
cannot schedule a frame until initial lifecycle state is known. The observer's
initial notification either keeps it suspended or resumes it. Model, camera,
selection, resize, and input methods remain valid while suspended; their state
is reflected on the first frame after resume.

## Window Lifecycle Boundary

Keep native lifecycle detection in the frontend's existing native-shell seam.
When running under Tauri, observe the current window's resize and focus-change
notifications and query `isMinimized()` for the authoritative state. Focus loss
alone does not suspend rendering. Perform an initial query before allowing the
mounted renderer to start, and discard stale asynchronous query results so a
rapid minimize/restore sequence cannot apply lifecycle state out of order.

When not running under Tauri, observe `document.visibilitychange` and derive the
suspended state from `document.hidden`. The observer exposes one boolean
callback and returns an unsubscribe function. `StarMap.svelte` uses that seam
to call `renderer.suspend()` or `renderer.resume()` and removes the observer on
unmount.

An observer setup or state-query failure must not leave a visible application
permanently frozen. The failure path retains or returns to active rendering;
ordinary rendering errors remain governed by the existing renderer behavior.

## Polling Lifecycle

The existing focus-aware server polling policy remains unchanged:

- a focused native window polls approximately every 30 seconds;
- a minimized or otherwise unfocused native window polls approximately every
  five minutes;
- manual refresh remains immediate in either state;
- browser-hosted `serve` mode retains its established polling behavior.

Minimization therefore stops GPU-driven canvas frames but does not stop the
application runtime, GitHub provider, control WebSocket, or cached model
updates. Data received while minimized is retained and appears on the first
restored frame.

## Testing Strategy

Use test-driven development at the renderer and lifecycle seams.

Renderer tests prove that:

- suspension cancels the outstanding frame and no callback schedules another;
- repeated suspension is harmless;
- resume schedules exactly one loop and repeated resume does not duplicate it;
- resume resets elapsed time so animations do not jump by the minimized
  duration;
- model, camera, and selection changes made while suspended appear after
  resume;
- destroy cleans up correctly from active and suspended states.

Lifecycle tests prove that:

- Tauri mode maps initial and subsequent `isMinimized()` results to the boolean
  callback and unregisters its native listener;
- browser mode maps `document.hidden` changes and unregisters its document
  listener;
- a native state-query failure fails open to active rendering;
- the Svelte wrapper connects lifecycle notifications to renderer suspension
  and removes both lifecycle and renderer resources on unmount.

The completion gate is the targeted frontend tests in red and green states,
the full frontend test suite, Svelte check, frontend production build, Rust
formatting, warnings-denied Clippy, and locked native Windows workspace tests.

## Release Notes

Record the power-saving behavior once under `Unreleased` in `CHANGELOG.md`.
Do not edit or repeat the change in an already shipped version section.

## Scope

This change does not destroy the webview, hide the window to the tray, pause
WebSocket delivery, change manual refresh, alter the five-minute background
polling interval, suspend merely because another window has focus, reset visual
state, or introduce a user-facing power setting.

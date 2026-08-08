# Immediate Space Publication Design

**Date:** 2026-08-08
**Status:** Approved in conversation; pending written-spec review

## Problem

Adding a GitHub repository persists its space, but the new project does not
appear in the sidebar until another action wakes synchronization. Removing a
space, restarting Stellr, manually refreshing, or waiting for the scheduled
poll makes the previously added project visible.

The frontend intentionally renders authoritative models received through the
control WebSocket. The add endpoint currently saves the new `SpaceEntry` but,
unlike remove and refresh, does not notify the poller to derive and publish a
new model.

## Product Decision

A successful repository add immediately requests synchronization through the
existing poller notification seam. Stellr continues to render only
authoritative server models; the frontend does not invent an optimistic space
or duplicate synchronization state.

## Design

After the add endpoint has validated the request, updated the `SpaceStore`,
successfully persisted it, and released the store lock, it calls the existing
refresh notification. The poller reads the updated store, synchronizes every
space, and replaces the model in the watch hub. Connected control WebSockets
then deliver that model, causing the keyed sidebar list and selected route to
update through their existing behavior.

The HTTP response remains the existing `{ "id": "..." }` payload and need not
wait for GitHub synchronization to finish. Duplicate and persistence failures
return before notification, so failed additions cannot trigger a misleading
model publication.

Provider failures retain the current behavior: the poller publishes the added
space with cached data when available and marks it stale with the provider
error. This keeps the new project visible even when GitHub cannot be reached.

## Alternatives Rejected

- An optimistic frontend placeholder would create a second source of truth and
  require reconciliation for provider, persistence, duplicate, and routing
  failures.
- Synchronizing inside the add request would make POST latency depend on GitHub
  and duplicate the poller's existing cache and publication path.

## Testing

The server integration test adds a repository and waits for the derived model
without issuing a separate refresh. It must fail before the fix because no
model is published, then pass after the add endpoint notifies the poller.

Existing add validation, persistence, remove, manual refresh, polling, control
WebSocket, and frontend authoritative-snapshot tests remain green. Native Rust
formatting, linting, and affected workspace tests provide broader validation.

## Release Notes

The `Unreleased` changelog records that newly added repositories now appear in
the sidebar without restarting Stellr or performing another space action.

## Acceptance Criteria

- A successfully added GitHub repository appears in the sidebar after the
  resulting authoritative model is published, without a restart, removal, or
  manual refresh.
- Failed validation or persistence does not trigger synchronization.
- Provider failure still publishes the added space with existing stale/error
  semantics.
- The add response shape and frontend authoritative-model architecture remain
  unchanged.
- Focused and broader native-Windows validation passes.

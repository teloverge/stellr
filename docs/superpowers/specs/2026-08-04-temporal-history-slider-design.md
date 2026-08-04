# Temporal Issue-History Slider Design

**Date:** 2026-08-04
**Status:** Approved in design review
**Scope:** The interactive Stellr issue map in desktop and `serve` modes

## Purpose

Add a temporal control to the bottom of the Stellr map so a user can inspect
and replay the repository's issue history. The control uses real GitHub issue
dates, marks recorded events, and projects the map as of the selected time.

The feature must minimize GitHub retrieval. Historical data is imported once,
stored locally, resumed rather than restarted after interruption, and updated
with targeted delta requests. Scrubbing and playback never query GitHub.

## User Experience Contract

The timeline is a floating control attached to the bottom of the map region.
It does not span the sidebar or cover the issue-detail pane. When issue detail
is bottom-docked, the map becomes shorter and the timeline remains attached to
the map's new bottom edge.

The control is ordered left to right as:

```text
selected date | chronological slider with event ticks | Play/Pause | speed
```

The selected range begins at the repository's first issue creation and ends at
the latest fully verified history checkpoint. When that checkpoint is current,
the right endpoint is labelled **Now**.

The default view is Now. While the playhead remains at Now, a newly verified
event extends the range and updates the live map automatically. If the user is
viewing the past, new activity does not move the playhead. The control instead
shows **New activity · Return to Now**.

## Tracked Activity

Only these event kinds participate in the temporal projection and produce
ticks:

- issue created;
- issue closed;
- issue reopened;
- milestone changed, including assignment to a milestone, movement between
  milestones, and removal from a milestone.

Comments, title and body edits, label changes, assignments, dependency edits,
and sub-issue edits are not timeline events in this feature.

The importer may use an issue's provider `updatedAt` value as a cheap signal
that its timeline needs examination. Events outside the tracked set are
discarded during normalization and never enter the local event ledger.

## Historical Projection

The renderer uses one deterministic coordinate set for the entire history.
Coordinates are derived from the complete current issue set and current graph
topology, not from the selected time. Moving the playhead never recomputes star
positions or moves the camera.

At a selected instant:

- an issue whose creation is later than the playhead is absent;
- an issue that exists and is open uses a distinct neutral
  **open-at-this-time** treatment;
- a closed issue uses the existing resolved treatment;
- a reopened issue returns to the neutral historical-open treatment;
- an issue belongs to the milestone recorded for it at that instant;
- milestone hulls may change membership and shape, but stars do not move.

Historical mode deliberately does not display blocked, frontier, or claimed
status. Those states require assignment and dependency history, which is
outside the selected event scope. At Now, Stellr restores its complete live
status derivation.

Dependency edges are current-topology context rather than reconstructed
history. In historical mode they remain spatially fixed, are limited to edges
whose two endpoint stars exist at the selected instant, and use subdued styling
with a **Current dependencies** legend. This makes their temporal limitation
explicit instead of implying that Stellr reconstructed past dependency edits.

Temporal playback changes only the map projection. It never changes the
canonical route, selected issue, open detail pane, or camera. A selected star
may be absent before its creation time; the selection remains available when
the playhead returns to a time at which the star exists.

## Playback

Playback covers the complete chronological range in a fixed presentation time,
not in real elapsed repository time:

| Speed | Full-history duration |
| --- | ---: |
| 0.5x | 60 seconds |
| 1x | 30 seconds |
| 2x | 15 seconds |
| 4x | 7.5 seconds |

Calendar time remains linearly proportional to slider position. Long quiet
periods are therefore represented honestly even when no visible event occurs.

Play continues from the current playhead. Pressing Play while already at Now
restarts at the first event. Reaching Now pauses. Dragging the slider pauses
playback and updates the map continuously.

Playback uses a monotonic animation clock. It converts elapsed presentation
time into an absolute event time, then applies all newly crossed events in
stable order. A slow frame cannot skip state. Events sharing a timestamp are
applied as one atomic group.

Each reached event briefly pulses its affected star and displays a short
caption such as `#42 created`, `#18 reopened`, or `#7 moved to M2`. A group of
events produces one bounded summary. Event feedback never pans the camera,
opens detail, or changes selection. Under reduced motion, the state and caption
still update but the pulse is omitted.

## Tick Interaction

Tick position is proportional to the event's actual timestamp. Events with the
same timestamp form one logical tick. Ticks that overlap at the current rendered
width are grouped into a visual cluster without changing the underlying time or
event order.

Hovering or keyboard-focusing a tick or cluster shows the exact local date and
the contained event summaries. Clicking moves the playhead to that instant. A
cluster tooltip lists its events in timestamp and provider-event-ID order.

The slider is a semantic range control with readable date `aria-valuetext`.
Keyboard behavior is:

- Left/Right: previous or next distinct event time;
- Home: first event;
- End: latest verified time;
- Space/Enter on Play: play or pause;
- Space/Enter on speed: cycle 0.5x, 1x, 2x, and 4x.

## Architecture

The current snapshot and temporal history remain separate responsibilities.
The existing `Provider` continues to supply the present issue graph. A new
`TemporalHistorySource` supplies normalized historical evidence. The live app
may reuse GitHub GraphQL transport, error mapping, and event normalization
patterns from `crates/showcase`, but it does not depend on release windows,
story beats, asset generation, or showcase manifests.

```text
existing GitHub issue snapshot
            |
            +------> current Model and live renderer
            |
            +------> relevant-change detector
                            |
GitHub historical import --> TemporalHistorySource
                            |
                            v
                     local SQLite ledger
                            |
                 history status + delta API
                            |
                            v
              frontend temporal projection
                            |
                            v
                existing canvas renderer
```

### Domain events

The normalized domain event contains:

- stable provider repository identity;
- stable provider issue identity and display number;
- stable provider event identity;
- exact UTC occurrence timestamp;
- one of `issue_created`, `issue_closed`, `issue_reopened`, or
  `milestone_changed`;
- the minimum payload needed to apply that transition.

An issue-creation event records its milestone at creation, if any. A milestone
transition records stable milestone identity plus the prior and resulting
display titles when the provider exposes them. Renaming a milestone is not a
tracked activity and does not create a tick.

Issue creation does not necessarily have a GitHub timeline-item ID. In that
case its provider event ID is derived deterministically from the provider issue
node ID and the `created` event kind. Provider-supplied timeline IDs are used
unchanged for all other events.

Events are ordered by `(occurred_at, provider_event_id)`. This order is stable
across pagination, retries, process restarts, and input ordering.

### Local ledger

Stellr adds a SQLite history database alongside, not inside, the existing JSON
current-snapshot cache. The ledger stores:

- provider repositories and stable provider repository IDs;
- provider issues and stable issue IDs/numbers;
- normalized events with a uniqueness constraint on repository and provider
  event ID;
- a monotonically increasing local ledger sequence for delta delivery;
- per-issue pagination cursors and last-seen provider `updatedAt` values;
- repository import state, counts, retry information, and verified-through
  watermark;
- a schema version managed by transactional migrations.

Event insertion, cursor advancement, and import-progress advancement occur in
one transaction. A crash can repeat the last provider page, but uniqueness and
the transaction boundary make the retry idempotent.

Tokens, API headers, comments, issue bodies, unrelated timeline items, and UI
playhead state are not stored in the history database.

### Initial background import

The first successful current sync starts an automatic, low-priority history
import. The importer freezes a provider cutoff, enumerates the issues known at
that cutoff, and retrieves only the selected timeline item kinds for each
issue. It processes one bounded page at a time, checkpoints every accepted
page, and yields between requests.

New activity may occur during the import. After all enumerated issue histories
reach the frozen cutoff, the importer performs a targeted catch-up against a
fresh current snapshot. The ledger becomes complete only when every initial
issue and every relevant catch-up issue is verified through the same successful
sync boundary.

The current map remains fully usable during import. The timeline stays visible
but disabled and reports determinate progress such as
`Building history · 312/840 issues`. No partial ticks are exposed.

### Incremental synchronization

The existing issue-list request gains only the stable provider IDs,
`createdAt`, and `updatedAt` fields needed by history synchronization. This
piggybacks on ordinary snapshot pagination rather than adding another
repository-wide query.

After initial completion:

1. a new provider issue creates a targeted import for that issue;
2. an unchanged issue `updatedAt` causes no history request;
3. a changed `updatedAt` queues only that issue's timeline after its saved
   cursor;
4. normalization discards non-tracked events;
5. accepted events advance the verified history and ledger revision.

Using `updatedAt` rather than only comparing current state is intentional. It
allows Stellr to discover a close/reopen cycle that begins and ends between two
ordinary polls. It can cause a small targeted timeline request after an
untracked edit or comment, but it avoids both a full-history refetch and a
false claim of complete history.

## Retrieval and Rate-Limit Policy

History work is always lower priority than the live snapshot. It has bounded
concurrency of one request/page stream per application process and never uses
unbounded retries.

The importer reads GitHub rate-limit cost, remaining budget, and reset evidence
available through the existing typed provider error path. It pauses before
exhaustion, records the resume time, and resumes after that time or an explicit
manual retry. Abuse-detection or secondary-rate-limit responses use the
provider's retry evidence when present and conservative backoff otherwise.

Completed pages, issues, and repositories are never fetched again unless the
provider reports a new `updatedAt` value or the user explicitly clears local
history. Playback, scrubbing, tick tooltips, application restarts, and offline
use make no GitHub requests.

## Server and Frontend Data Flow

The whole-model control WebSocket remains the current-state authority. Each
space gains only a compact history summary:

- import state;
- completed and total issue counts;
- earliest event time;
- verified-through time;
- local ledger revision;
- delayed, rate-limited, or failed diagnostic when applicable.

A dedicated authenticated loopback endpoint returns the complete normalized
history after initial completion. Subsequent requests accept the last local
ledger sequence and return only later events plus the new history summary. The
frontend retains the event array in memory and derives historical projections
without server round trips while the user scrubs or plays.

The history endpoint follows the existing per-run bearer-token protection in
desktop mode and the existing authenticated server policy in `serve` mode. It
never exposes the SQLite path or raw provider responses.

## Delayed, Offline, and Failure States

- **Initial import incomplete:** timeline disabled; current map unaffected.
- **Offline during initial import:** checkpoint retained; import resumes when
  connectivity returns.
- **Complete ledger offline:** verified history remains playable.
- **Incremental history delayed:** playback ends at the last verified
  checkpoint and displays `History through DATE`; Return to Now exits temporal
  mode and shows the current snapshot without inventing the missing interval.
- **Rate limited:** import status shows the reset/resume time when known.
- **Malformed or ambiguous provider history:** the affected repository import
  fails with issue number, event identity, cursor, and stage where available.
- **Migration failure:** the migration transaction rolls back and the previous
  usable ledger remains intact.
- **Space removal:** the explicit existing space-removal workflow removes that
  space's local history; ordinary refreshes and errors never prune it.
- **No issues:** the control displays `No issue history` and remains disabled.

Stellr never fills an unverified interval from observation time, local wall
clock guesses, current blocker state, or synthetic lifecycle events other than
the deterministic issue-creation identity described above.

## Accessibility and Responsive Behavior

- The floating control meets the existing token-based contrast policy and does
  not use status color as its only event distinction.
- The playhead exposes a readable date, not only a numeric epoch.
- Play/Pause and speed have persistent accessible names and visible focus.
- Tick tooltips are available through focus as well as hover.
- Reduced motion removes pulses and animated emphasis while preserving state,
  captions, and manual navigation.
- At narrow map widths, the date moves above the slider and controls remain in
  a compact second row; the slider never collapses below a usable touch target.
- The overlay reserves enough bottom inset that stars and labels can be panned
  clear of it without changing the graph's world coordinates.

## Privacy and Security

- Historical acquisition remains read-only.
- No new GitHub permission scope is required.
- The history database excludes tokens, comments, bodies, response headers,
  and unrelated timeline events.
- All event strings crossing into the UI are rendered as text.
- Database and endpoint repository identifiers are validated against known
  spaces; callers cannot supply an arbitrary filesystem database path.
- SQLite statements use bound parameters, and migrations are fixed application
  resources rather than runtime input.

## Testing Strategy

### Domain projection

- Created, closed, reopened, and every milestone transition at exact boundary
  timestamps.
- Same-timestamp ordering by provider event ID.
- Issues absent before creation and neutral-open after creation or reopening.
- Now restoring the live blocked/frontier/claimed derivation.
- Current dependency edges filtered to visible endpoints and identified as
  current topology.
- Fixed star coordinates through every projection and milestone hull change.

### Ledger and importer

- Unique event IDs make repeated pages and catch-up retries idempotent.
- Cursor, events, progress, and watermark commit atomically.
- Restart resumes at the saved issue and cursor rather than restarting.
- Initial completeness requires frozen-cutoff import plus catch-up.
- Changed `updatedAt` queues one issue; unchanged metadata queues none.
- A close/reopen cycle between polls is retained even when final state matches
  the prior snapshot.
- Non-tracked events are filtered and not stored.
- Transactional schema migration success and rollback.
- Explicit space removal deletes only the intended repository history.

### Provider and retrieval policy

- GraphQL pagination requests only the selected timeline kinds.
- Existing list pagination carries IDs and timestamps without a second list
  request.
- Rate-limit exhaustion pauses until reset and does not busy-loop.
- Secondary-limit responses use bounded conservative backoff.
- Completed history is not fetched again on playback, reconnect, or restart.
- Offline and malformed-response diagnostics name the failing stage.

### Server and frontend

- History summary rides current snapshots without embedding the event ledger.
- Initial history response and ledger-sequence deltas are consistent.
- Authentication protects the history endpoint in desktop and serve modes.
- Thirty-second 1x playback and 0.5x, 2x, and 4x variants under a controllable
  monotonic clock.
- Slow animation frames apply every crossed event in order.
- Dragging pauses; Play at Now restarts; reaching the end pauses.
- Default Now following, past-view pinning, new-activity notice, and Return to
  Now.
- Dense tick clustering, exact tick navigation, captions, and grouped events.
- Playback preserves route, issue detail, camera, and selection.
- Keyboard, screen-reader, reduced-motion, and narrow-layout behavior.

### Native acceptance

- Native Windows Rust tests, frontend tests, type checks, formatting, linting,
  and packaged desktop smoke tests.
- A repository with a complete ledger plays while GitHub is unreachable.
- A deliberately interrupted large import resumes without re-fetching accepted
  pages.
- Provider request logs demonstrate no history request during repeated
  playback and no repository-wide history refetch after completion.

## Non-Goals

- Historical comments, titles, bodies, labels, assignments, blockers, or
  sub-issue relationships.
- Historically exact blocked/frontier/claimed status.
- Historical dependency-edge reconstruction.
- Timeline zoom, arbitrary loop ranges, bookmarks, or exported recordings.
- Cross-device history synchronization or a hosted history service.
- Replacing the current snapshot cache or the release-showcase story format.
- Automatically opening issue detail or moving the map camera during playback.

## Acceptance Criteria

- A fixed-bottom map control exposes date-proportional ticks, scrubbing,
  Play/Pause, and 0.5x, 1x, 2x, and 4x playback.
- At 1x, the full verified history plays in 30 seconds.
- The map is spatially stable throughout playback.
- Temporal state covers only issue creation, close/reopen, and milestone
  changes and does not imply unavailable historical status information.
- Event feedback highlights affected stars without changing route, selection,
  detail, or camera.
- A complete local ledger is required before playback becomes available.
- Initial import is automatic, low priority, resumable, rate-limit aware, and
  never blocks the live map.
- Completed history is never broadly refetched; incremental synchronization
  targets only new or provider-updated issues.
- Scrubbing and playback are entirely local and work offline after completion.
- Unverified gaps remain explicit and are never filled with invented history.
- Storage, transport, projection, timing, accessibility, and native Windows
  acceptance tests enforce the design.

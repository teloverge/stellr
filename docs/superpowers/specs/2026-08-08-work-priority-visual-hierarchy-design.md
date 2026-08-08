# Work-Priority Visual Hierarchy Design

## Goal

Make Stellr's live issue graph answer four questions in order:

1. What needs my attention now?
2. What am I doing now?
3. What should I do next or later?
4. What team and historical context should remain visible?

The map must keep the complete issue graph visible without letting completed
paths or other people's assignments compete with the operator's active work.
Active nodes must retain a readable screen-space size when the camera is
zoomed out.

## Status Model

Keep `claimed` compatible with its current meaning: an open issue assigned to
one or more people. Do not redefine it as "assigned to me."

Add orthogonal facts instead of expanding the mutually exclusive status enum:

- `viewer_login`: the authenticated GitHub login, when known;
- `blocked`: the issue is open and at least one known blocker is open;
- `ready_for_agent`: the issue is open, is not blocked, and carries the
  case-insensitive `ready-for-agent` label;
- `assignees`: the existing list of GitHub logins;
- session liveness: the existing local implementing, blocked, or dead overlay;
- closure reason: the existing resolved versus out-of-scope distinction.

`assigned_to_viewer` is true when `viewer_login` case-insensitively matches an
assignee. An issue assigned to the viewer can therefore also be ready or
blocked. The existing `Status` enum remains available to current consumers and
keeps its current precedence and serialized values.

## Viewer Identity and Offline Behavior

Add `viewer { login }` to the existing GitHub GraphQL issue request. Return the
login with the fetched issues from the provider boundary. Pagination must
retain one stable login while accumulating issue pages.

Store the optional viewer login in the per-repository cache snapshot and expose
it on `SpaceModel`. Older caches without the field remain readable through a
serde default. A failed live refresh uses both cached issues and cached viewer
identity.

When native device authorization replaces the active credential, cached issues
may remain visible but cached viewer identity must not be applied until the new
credential completes one successful live fetch. This prevents an account
change from temporarily presenting the previous account's assignments as My
work.

If no viewer identity is available, Stellr must not guess. Assigned issues are
rendered as team work and no issue is classified as assigned to the viewer.
Local session evidence can still identify Doing now or the attention override,
because it is authoritative evidence from this Stellr instance. The existing
launcher-supplied CURRENT issue is also authoritative Doing-now evidence; a
merely selected detail-pane issue is not.

## Priority Derivation

Derive one renderer priority for every issue in this order:

| Priority | Name | Condition |
| --- | --- | --- |
| Override | Needs attention | A local session on the issue is blocked or dead |
| P1 | Doing now | The launcher-supplied CURRENT issue, or a local session on the issue is implementing |
| P2 | My next work | Open, assigned to the viewer, and ready for agent |
| P3 | My future work | Any other open issue assigned to the viewer |
| P4 | Available next | Open, unassigned, and ready for agent |
| P5 | Team work | Open and assigned to someone other than the viewer |
| P6 | Planning or waiting | Any remaining open issue |
| P7 | Closed context | Resolved or out of scope |

P3 includes dependency-blocked work as its primary case, but also retains an
owned issue that is waiting for information, ready for a human, or otherwise
not agent-ready. Ownership must not be demoted below unassigned work merely
because an issue lacks an actionability label.

The attention override uses P1's core size and ownership treatment while the
existing session overlay continues to distinguish a waiting session from a
dead one. Issue blocking and session blocking remain separate concepts.

Ordinary click selection remains a navigation overlay and never promotes an
issue. CURRENT keeps its existing navigation marker while its
launcher-supplied issue identity also provides the P1 Doing-now input.

## Node Treatment

The renderer uses these exact dark-canvas core colors and shape rules:

| Priority | Core | Shape | Motion |
| --- | --- | --- | --- |
| Needs attention | `#ffd873` | solid | existing blocked/dead session grammar |
| Doing now | `#ffd873` | solid | core pulse plus existing orbiting session moon |
| My next work | `#8ad8ff` | solid | none |
| My future work | `#8ad8ff` | hollow | none |
| Available next | `#8ed7ac` | hollow | none |
| Team work | `#b9a7ee` | solid | none |
| Planning or waiting | `#aaa0bd` | hollow | none |
| Closed completed | existing `#b9d6c4` | solid | none |
| Closed not planned | existing `#948da4` | hollow | none |

Doing now pulses its core radius between `1.00` and `1.08` of the derived
radius using the renderer's existing beat. The existing session moon is the
approved orbiting proton; do not add a second orbiting body. Reduced-motion
rendering freezes the pulse and orbit while preserving their shapes.

Use the following base world-space radii and minimum screen-space radii:

| Priority | Base world radius | Minimum screen radius |
| --- | ---: | ---: |
| Needs attention / Doing now | 12 | 10 px |
| My next work | 11 | 8.5 px |
| My future work | 10 | 7 px |
| Available next | 10 | 7 px |
| Team work | 9 | 6 px |

The rendered world radius is `max(base_world_radius,
minimum_screen_radius / camera_scale)`. Lower priorities retain their existing
state radii without a screen-space floor. Glow, relationship rings, session
orbits, selection rings, label obstacles, and pointer hit areas use the derived
core radius so the visible and interactive geometry stays aligned. Layout
positions and camera fit do not change.

## Traversed Connections

An edge is traversed when its source issue is resolved and its directed
destination is still open. Resolved/completed workflow edges that do not meet
that condition remain historical edges, but they use the same quiet static
treatment.

Traversed and other resolved/completed connections are contextual history by
default:

- use a `1.6` world-pixel stroke in `rgba(150,178,160,0.36)` and an arrowhead
  in `rgba(190,218,198,0.52)`, making them thinner and lower contrast than
  active incomplete paths;
- paint no particles by default;
- preserve direction, arrowheads, curve geometry, and dependency versus
  parent/subissue semantics;
- never let a selected edge regain motion that its workflow state does not
  permit.

Animate particles only when a traversed edge's directed `to` endpoint is
currently Doing now, My next work, or Available next. The test is direct and
directional: an incident edge pointing away from an active node remains static,
as does every transitive edge beyond the active node.

An eligible edge paints exactly two `1.8` world-pixel particles with no halo.
Particle alpha follows `0.35 + 0.40 * sin(pi * u)`, retaining direction without
turning the historical line into a luminous path. Particle speed remains the
existing `0.1` curve-lengths per renderer second.

The selected-node contract remains direct-only. Selected incoming and outgoing
edges still paint last with their approved width and arrowhead multipliers, but
motion eligibility is calculated independently from selection.

## Renderer Boundaries

Introduce a small pure priority-derivation module between adapted tickets and
canvas paint. It owns priority precedence and exposes the visual priority to the
renderer. Keep palette and geometry constants in the star-map visual layer.

The adapter copies the new orthogonal fields and derives
`assignedToViewer` from `SpaceModel.viewer_login`. The pure priority seam also
receives CURRENT and session state. The canvas renderer consumes those derived
facts; it does not inspect GitHub labels, compare logins, or reconstruct blocker
state.

Do not change graph topology, deterministic layout positions, pan/zoom
behavior, route state, detail-pane content, or GitHub write behavior.

## Error Handling and Compatibility

- Older cache snapshots load with `viewer_login = None`.
- Older serialized models load new booleans as false.
- Credential replacement suppresses cached viewer identity until a successful
  fetch confirms the new account.
- Missing or malformed viewer data is a typed provider parse failure on a live
  response; stale cached data remains available through the existing fallback.
- Unknown viewer identity produces Team work, never a false My-work result.
- A session whose GitHub assignment has not caught up still renders as Doing
  now, because local live evidence has higher precedence.
- Existing showcase and historical status consumers keep the legacy `Status`
  contract.

## Verification

Use test-driven development across the affected seams:

1. Rust core tests prove `blocked` and `ready_for_agent` coexist with assignment
   while legacy `claimed` remains unchanged.
2. GitHub provider tests prove viewer parsing on every pagination shape and
   typed failure behavior.
3. Cache tests prove viewer identity round-trips and old snapshots default
   safely.
4. Server/API tests prove live and stale `SpaceModel` snapshots expose the
   correct viewer identity and orthogonal facts.
5. Adapter and pure priority tests cover every priority, unknown viewer,
   case-insensitive login matching, CURRENT, ordinary selection, and session
   overrides.
6. Canvas recording tests prove exact colors, solid/hollow shapes, pulse,
   screen-size floors, session overlay alignment, and reduced-motion behavior.
7. Edge tests prove subtle static traversed treatment, direct directional
   motion into only P1, P2, and P4, and selection-independent motion gating.
8. Run the complete native Windows frontend test, check, build, Rust format,
   Clippy, and locked workspace test gates.
9. Perform a headed visual check at normal and zoomed-out camera scales.

## Build and Running-App Handoff

Implementation and validation occur in an isolated worktree under `D:\tmp`
using native Windows tools. Build the web bundle before the Rust application so
the embedded assets are current.

Do not overwrite or uninstall `D:\Apps\Stellr`. Keep the currently installed
binary as the rollback target. After every automated gate passes:

1. capture the exact running `stellr-desktop.exe` PID and executable path;
2. build a release `stellr-desktop.exe` from the implementation worktree;
3. stop only the captured Stellr desktop process;
4. launch the freshly built executable without arguments so it restores the
   existing spaces and route from the shared Stellr app data;
5. verify a top-level Stellr window, WebView2 child, and the new graph visuals;
6. if the new process fails to establish a working window, stop it and relaunch
   `D:\Apps\Stellr\stellr-desktop.exe`.

The successful handoff leaves the development build running for user review.
It does not replace the installed application or produce a release installer.

## Scope

This slice changes live issue synchronization metadata and star-map visual
priority. It does not add GitHub writes, change assignment semantics, redesign
the detail pane, alter graph layout, add team-presence synchronization, modify
the installed application, or redesign historical showcase playback.

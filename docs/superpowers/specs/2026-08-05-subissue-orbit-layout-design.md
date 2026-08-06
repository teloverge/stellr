# Adaptive Subissue Orbit Layout Design

## Problem

Stellr currently places direct subissues on compact parent-local arcs with
fixed radii. Dense issue families therefore put stars, relationship curves,
and titles into the same small area. Labels overlap, relationship direction is
hard to follow, and neighboring stars are difficult to select reliably.

The layout must improve subissue readability and click precision without
discarding Stellr's circular constellation language or its deterministic,
spatially stable map.

## Goals

- Arrange each valid subissue family on adaptive concentric circular orbits
  around its immediate parent.
- Reserve enough space for visible labels as well as star centers.
- Place subissue labels outward from their parent-centered orbit.
- Make individual subissues easier to select without enlarging their visible
  stars.
- Preserve deterministic coordinates across reloads, snapshot input order,
  and status-only or temporal updates.
- Keep broad-layout parent anchors unchanged.

## Non-goals

- Replacing the broad constellation layout.
- Changing relationship semantics, focus traversal, status styling, or edge
  animation.
- Moving nodes in response to status, selection, hover, or replay time.
- Guaranteeing full untruncated titles at arbitrarily high graph density.
- Adding user-configurable layout controls in this slice.

## Layout Geometry

The broad deterministic layout remains responsible for the initial position of
every issue. Subissue placement then operates parent-first, using each immediate
parent's final point as the fixed center of a local orbit.

For each valid parent group:

1. Order siblings deterministically by dependency flow, with issue number as
   the stable fallback and cycle tie-breaker.
2. Estimate the occupied footprint of each child from the visible star and its
   truncated label bounds.
3. Allocate children to one or more concentric rings. Ring radii and capacities
   derive from the minimum occupied footprint rather than fixed child counts.
4. Distribute each ring's children around the full circle with deterministic
   angular spacing. Additional rings are introduced only when one ring cannot
   meet the required clearance.
5. Evaluate a bounded set of deterministic rotations for the complete cluster.
   Select a collision-free candidate when available; otherwise select the
   candidate with the lowest stable collision score.

Candidate scoring considers:

- child-to-child star and label clearance;
- clearance from unrelated nodes and labels;
- clearance from existing dependency lines and previously placed nested
  cluster curves;
- relationship-line crossings, which retain a stronger penalty than ordinary
  clearance differences.

Nested groups are placed in hierarchy-depth order. A child that is also a
parent receives its own orbit only after its position in the ancestor orbit is
final.

## Labels

Subissue labels extend away from the immediate parent:

- nodes on the right side use left-aligned labels;
- nodes on the left side use right-aligned labels;
- nodes near the top or bottom use centered labels;
- every label receives a consistent radial gap from the visible star.

The renderer exposes the same deterministic label bounds to the layout scorer
and drawing path so collision decisions match what appears on screen. Full
titles remain available in the detail pane. When a graph is too dense for all
labels, the fallback preserves star separation and pointer access first, then
uses the existing consistent title truncation.

Top-level issue labels retain their current behavior. This change is scoped to
nodes with a valid in-snapshot parent relationship.

## Pointer Interaction

Visible star sizes remain unchanged. Hit testing uses a larger invisible
screen-space target for subissues so a user can select the intended child even
at ordinary zoom levels. The target must not allow a more distant subissue to
win over a nearer one; when targets overlap, hit testing resolves to the closest
star center with issue number as the deterministic final tie-breaker.

Pan, zoom, deep-link selection, focus highlighting, and detail-pane behavior
remain unchanged.

## Invalid and Constrained Inputs

Missing parents, self-parent relationships, parent cycles, and non-finite broad
coordinates do not participate in orbit placement. Their nodes retain their
broad-layout positions.

If all candidate rotations are obstructed, placement uses the finite candidate
with the lowest deterministic collision score. It never introduces random
coordinates or movement tied to transient UI state.

## Implementation Boundaries

- `cluster-layout.ts` owns deterministic ring allocation, candidate generation,
  and collision scoring.
- A small shared label-geometry seam owns subissue label alignment and bounds so
  layout and canvas rendering use the same rules.
- `starmap.ts` consumes the selected coordinates and shared label geometry, and
  owns screen-space pointer hit testing.
- The broad `computeLayout` seam remains status-independent and continues to
  invoke subissue placement after broad relaxation.

## Validation

Unit tests will cover:

- small sibling groups using one complete circular orbit;
- dense sibling groups expanding to multiple concentric rings;
- minimum star and label clearance;
- outward label alignment on the left, right, top, and bottom of a parent;
- dependency-ordered siblings and deterministic cycle fallback;
- snapshot-order independence and status-only coordinate stability;
- nested parent-first placement;
- invalid hierarchy and fully obstructed deterministic fallbacks;
- enlarged subissue hit targets, nearest-node resolution, and unchanged
  top-level hit behavior.

Renderer tests will verify that drawing and collision scoring share the same
label geometry. A native Windows browser/app validation will exercise an
Encrydle-shaped dense graph at normal and reduced viewport sizes, checking that
titles are materially more readable and intended subissues can be selected
without neighboring stars intercepting the click.

## Acceptance Criteria

1. Valid direct subissues appear on deterministic full circular orbits around
   their immediate parent, adding concentric rings as density requires.
2. Subissue labels extend outward and participate in layout collision scoring.
3. Broad parent anchors and status-only/temporal spatial stability are
   preserved.
4. Visible stars remain unchanged while subissue pointer targets become easier
   to acquire and resolve to the nearest center.
5. Dense and nested groups have deterministic, finite fallback geometry.
6. Automated layout, rendering, and interaction tests pass, and the dense
   Windows validation demonstrates readable labels and reliable selection.

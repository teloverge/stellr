# Compact Subissue Cluster Layout Design

**Date:** 2026-08-02
**Status:** Approved design, pending written-spec review
**Repository:** Stellr

## Purpose

Subissue mini-graphs currently participate in the same broad physics relaxation as ordinary dependencies. Their entry, sequence, and return edges can therefore spread children too far from their parent and sweep across unrelated dependency paths.

Stellr will treat each native parent/subissue group as a compact, parent-local satellite cluster. The parent keeps its global dependency position. Its direct children are placed afterward in a deterministic nearby sector selected for clearance from unrelated nodes and dependency lines.

## Goals

- Keep direct subissues visibly close to their parent.
- Preserve sibling blocker order inside each mini-graph.
- Avoid obstructing unrelated dependency lines and unrelated nodes whenever a collision-free sector exists.
- Keep repeated layouts and status-only updates positionally stable.
- Support independent children, sibling sequences, nested subissues, and larger direct-child groups.
- Preserve the current directed arrows, resolved/incomplete grammar, CURRENT/READY precedence, and traversed-only motion.

## Non-goals

- Do not change readiness, status, or blocker-derived rank.
- Do not move parent nodes to accommodate children.
- Do not globally optimize or reroute every dependency edge.
- Do not add continuous layout physics or animation-time collision work.
- Do not change GitHub synchronization or hierarchy contracts.
- Do not guarantee a crossing-free drawing when every bounded candidate sector is obstructed.

## Decisions

### Parent-local override

A valid native child may override its normal radial-rank coordinate for display. This override changes presentation only. The child's status and dependency rank continue to come exclusively from `blocked_by`.

The global layout determines parent and non-cluster anchor positions. A second deterministic pass replaces direct-child coordinates with cluster coordinates.

### Global layout boundary

The broad relaxation uses ordinary dependency edges from `edgesOf(nodes)` for springs and radial rank. Entry and return edges do not pull global anchors. Sibling blocker edges remain ordinary dependencies for rank but the cluster pass owns their final local child positions.

The structure signature continues to include hierarchy topology so adding or removing a parent relationship recomputes layout. Status remains excluded.

### Compact geometry

Each parent group uses these initial constants:

| Constant | Value | Purpose |
| --- | ---: | --- |
| Candidate sectors | 16 | Deterministic directions at 22.5 degree intervals |
| First-arc radius | 92 layout pixels | Compact default parent-to-child distance |
| Second-arc radius | 126 layout pixels | Overflow and clearance fallback |
| First-arc capacity | 5 children | Keeps the compact arc legible |
| Arc step | 30 degrees | At radius 92, yields at least 47 pixels between adjacent centers |
| Minimum child-center clearance | 44 pixels | Prevents node and label-core overlap |
| Unrelated-node clearance | 42 pixels | Keeps the cluster away from other nodes |
| Dependency-line clearance | 18 pixels | Keeps child nodes and mini curves away from unrelated dependency strokes |
| Mini-edge bow cap | 28 pixels | Prevents entry/return curves from sweeping across the broad graph |

The first arc is centered on the chosen sector. Children are distributed symmetrically around its center direction. More than five children use a shallow second arc. The second arc remains centered on the same sector and uses the same minimum-clearance rule.

Constants live in one pure layout module and are covered by geometry tests rather than duplicated between layout and renderer.

### Child ordering

Direct children are ordered by the sibling-only dependency DAG:

1. roots with no sibling blockers;
2. their blocker-to-dependent sequence;
3. issue number as the deterministic tie-breaker for independent or otherwise equal nodes.

The ordering affects positions only. It does not introduce new dependency semantics.

If malformed sibling dependencies contain a cycle, the bounded topological walk emits all acyclic nodes first and appends remaining nodes by issue number. Rendering never fails.

### Sector selection

For each parent, the cluster pass evaluates all 16 sector directions. A candidate contains the proposed child centers and the exact mini-edge centerlines produced by the shared curve-geometry helper.

A candidate is collision-free when:

- every child center has at least 42 pixels of clearance from unrelated nodes;
- child centers are at least 44 pixels apart;
- sampled mini-edge curves remain at least 18 pixels from unrelated dependency segments;
- mini-edge curves do not pass through unrelated node-clearance circles.

Unrelated dependencies exclude edges whose endpoints are both the parent or its direct children. Sibling sequence edges are therefore evaluated as part of the mini-graph, not as external obstructions.

Candidate selection proceeds in this order:

1. collision-free first-arc candidate with the highest clearance score;
2. collision-free second-arc or mixed two-arc candidate with the highest clearance score;
3. deterministic lowest-obstruction candidate if no collision-free placement exists.

The score combines minimum node clearance, minimum dependency-segment clearance, and crossing count. Crossing count has highest penalty. Exact score weights are module constants and tests assert ordering outcomes rather than incidental floating-point totals.

Ties use a stable parent-number-derived starting sector, then sector index. Input order never affects the result.

### Nested groups

Groups are processed parent-first by hierarchy depth. If a child is also a parent, its own cluster is placed after that child receives its parent-local coordinate.

Nested clusters:

- use their immediate parent's final coordinate as the anchor;
- consider ancestor cluster nodes and curves as obstacles;
- may use the second radius when the compact arc is blocked;
- never move ancestors or unrelated anchors.

Hierarchy traversal is cycle-safe. Self-parent, missing-parent, and cyclic-parent references that cannot be ordered fall back to their global coordinates.

### Shared curve geometry

Mini-edge control-point calculation moves into a pure helper used by both cluster scoring and canvas rendering. This keeps obstruction scoring aligned with the curve that users see.

- Reverse entry/return edges retain opposite curve sides.
- Sequence edges bend toward the parent-side interior of the cluster.
- Bow magnitude is `min(28, edgeLength * 0.18)`.
- Arrowheads retain their current size, midpoint placement, tangent direction, colors, alpha, and motion behavior.

Ordinary dependency edges retain their existing geometry and styling.

## Module boundaries

### `layout.ts`

Owns the broad deterministic dependency layout:

1. sort nodes;
2. compute dependency-only rank;
3. run broad dependency springs and radial relaxation;
4. call the cluster-placement module;
5. return final points.

It does not contain sector scoring or curve sampling details.

### `cluster-layout.ts`

A new pure deep module that owns:

- direct-child grouping;
- hierarchy depth ordering;
- sibling topological ordering;
- first/second arc generation;
- obstacle extraction;
- candidate scoring and deterministic tie-breaking;
- child-coordinate overrides;
- bounded malformed-data fallbacks.

Its public interface accepts nodes, broad-layout points, and ordinary dependency edges and returns final points plus optional cluster metadata needed by curve geometry.

### `workflow-geometry.ts`

A pure geometry helper that owns:

- mini-edge control-point calculation;
- quadratic sampling;
- point-to-segment and curve-to-segment clearance;
- shared bow constants.

The renderer consumes it but does not own placement policy.

### Renderer

The renderer consumes final points and shared curve geometry. It retains:

- node/core paint;
- CURRENT/READY paint;
- edge state and alpha;
- arrows;
- traversed-only particles;
- camera and interaction behavior.

No candidate scoring or obstacle scan occurs per animation frame.

## Data flow

```text
Snapshot tickets
  -> dependency edges and blocker-derived rank
  -> broad deterministic anchor layout
  -> direct-child hierarchy groups
  -> candidate satellite sectors
  -> clearance/crossing score
  -> child coordinate overrides
  -> shared mini-curve geometry
  -> canvas paint
```

Status changes update paint and focus but do not enter the structure signature or cluster placement.

## Failure and fallback behavior

- Missing or out-of-snapshot parent: keep the broad-layout coordinate.
- Self-parent: ignore the relationship for placement.
- Parent cycle: place the orderable prefix; leave unorderable groups at broad coordinates.
- Sibling dependency cycle: append remaining siblings by issue number.
- No collision-free sector: use the deterministic lowest-obstruction candidate.
- Excess children: add the bounded second arc; do not expand without limit.
- Invalid numeric geometry: retain the node's broad coordinate and continue rendering.

One malformed group never prevents other groups or the map from rendering.

## Testing strategy

### Pure cluster tests

- #14-style independent children occupy one compact parent arc.
- #16-style sibling sequences preserve blocker order.
- A dependency segment through the default sector selects a clear alternative sector.
- A nearby unrelated node redirects the cluster.
- Five children fit the first arc without violating clearance.
- Six or more children use the second arc.
- Nested groups process parent-first and remain bounded.
- Self-parent, missing parent, sibling cycles, and parent cycles fail safely.
- Reversed input order produces identical points and metadata.
- Status-only changes produce identical points.
- Dependency ranks remain unchanged by clustering.
- When every sector is blocked, the same lowest-obstruction fallback wins repeatedly.

### Curve tests

- Mini bow never exceeds 28 pixels.
- Reverse entry/return curves occupy opposite sides.
- Sequence curvature points toward the parent-side interior.
- Candidate scoring samples the same curve geometry used by rendering.
- Arrow direction, tangent alignment, resolved/incomplete styling, alpha, and motion remain unchanged.

### Renderer and integration tests

- Same topology preserves coordinates across status snapshots.
- Adding/removing a parent relationship triggers one structural recomputation.
- Parent-local placement does not change CURRENT/READY rings or solid/hollow cores.
- Non-mini dependencies retain their existing paths and motion.
- The map remains interactive and programmatic selection does not recenter on same-route snapshot refresh.

### Live acceptance

Using the freshly built native Windows app:

- #14-style independent loops are compact and do not cover unrelated dependency lines.
- #16-style sequences remain close to #16 and readable in blocker order.
- Nested/large groups remain bounded.
- Wide and narrow M1 chrome remain functional.
- Browser console contains no errors or warnings.

## Performance

Cluster placement runs only on structural layout recomputation. It performs a bounded 16-sector search per parent group. Obstacle scoring may scan nodes and dependency segments, which is acceptable off the animation path for repositories with hundreds of issues.

No new per-frame search, collision detection, or allocation is introduced.

## Acceptance criteria

- Every collision-free direct-child group uses a sector with the specified node and dependency clearances.
- Up to five direct children use the compact first arc; larger groups use the bounded second arc.
- Parents and unrelated anchor nodes retain their broad-layout coordinates.
- Sibling sequence order matches blocker direction.
- Hierarchy does not change readiness or dependency rank.
- Repeated and status-only layouts are deterministic.
- Mini curves use the 28-pixel bow cap and current arrow/motion grammar.
- When no collision-free solution exists, the deterministic fallback renders without failure.

# Node Visual Hierarchy and 2.5D Rendering Design

## Goal

Make the map's work priority readable at a glance, reduce ring and arrowhead
clutter, and give nodes an Obsidian-inspired three-dimensional presence without
replacing Stellr's deterministic two-dimensional topology or camera.

The visual priority for active work is:

1. `in-progress`;
2. `ready-for-agent`;
3. blocked.

Closed and not-planned issues remain terminal states rather than active work.

## Status Resolution

Status derivation continues to resolve terminal issue state first. A closed
issue is completed and a closed-not-planned issue is out of scope even if stale
workflow labels remain on GitHub.

For an open issue, resolve the visible priority in this order:

1. a case-insensitive `in-progress` label;
2. the existing claimed fallback when the issue has one or more assignees;
3. the canonical case-insensitive `ready-for-agent` label when no blocker is
   open;
4. blocked when at least one blocker remains open;
5. the existing unblocked frontier treatment without a READY marker.

An open issue labeled `in-progress` therefore retains the strongest active
treatment even when it has an unresolved blocker. `ready-for-agent` remains
truthful only for unblocked work. Do not introduce a `ready` alias: the
repository's canonical tracker vocabulary remains `ready-for-agent`.

Add the missing `in-progress` tracker label with color `#fbca04` and description
`Work is actively being implemented`. Do not automatically add or remove it on
existing issues; label application remains an explicit tracker action.

## Reference Treatment

Obsidian's core Graph View establishes the useful interaction vocabulary:
circles are nodes, lines are relationships, and focusing a node highlights its
connections. Community 3D graph views add lit sphere meshes, perspective,
orbiting, and neighbor emphasis. Stellr adopts only the lit-sphere depth cue and
neighbor emphasis from that treatment. It deliberately retains its fixed 2D
map, camera, and direct-edge behavior.

References:

- <https://obsidian.md/help/plugins/graph>
- <https://community.obsidian.md/plugins/fast-graph>

## Node Rendering

Keep the current Canvas 2D renderer and deterministic node coordinates. Render
each node as a 2.5D sphere using layered radial gradients:

- a small, soft specular highlight above and left of center;
- the semantic status color across the middle of the sphere;
- a darker lower-right falloff that supplies volume;
- a restrained contact shadow below the sphere;
- one thin internal boundary stroke that is part of the body, not an outer
  status ring.

The depth treatment must preserve the existing status palette. It may derive
lighter and darker variants from the semantic color, but it must not introduce
a new color meaning. The highlight and shadow stay fixed in screen orientation
so nodes read as consistently lit while the map pans and zooms.

This is visual depth, not graph depth. Do not add Z coordinates, perspective
layout, orbit controls, auto-rotation, WebGL, Three.js, force simulation, or
status-dependent node movement. Historical replay must keep the same spatial
topology.

## Ring Grammar

An outer ring has exactly one meaning. Stable rendering may show at most one
status ring and one selection ring around a node.

- `in-progress`: one amber breathing status ring. This is the strongest active
  treatment. Replace the current pair of claimed rings with this single ring.
- `ready-for-agent`: one quieter cyan status ring with no breathing motion.
- blocked, completed, out-of-scope, and ordinary unblocked frontier: no outer
  status ring; render only the 2.5D node body.
- selected: one neutral white selection ring outside the status ring when one
  exists, or outside the node body otherwise.

Remove the node-level parent/subissue rim. Parent and subissue meaning already
travels through the workflow edges and must not consume another ring channel.
Remove the two CURRENT rings because CURRENT is the selected issue in this
renderer and the selection ring already communicates that state. Replace the
ring-shaped click flare with a brief increase in sphere glow so interaction
feedback cannot create a third outer ring.

The maximum stable count is therefore two outer rings for a selected
in-progress or ready issue, one for an unselected in-progress or ready issue,
one for a selected blocked or completed issue, and zero for an unselected
blocked or completed issue.

## Relative Emphasis and Labels

The base node sizing, glow strength, and label-slot priority must follow the
same active-work order. Use these body-radius and glow-radius pairs before the
existing issue-radius scale is applied:

- in-progress: `8.1` and `42`;
- ready-for-agent: `7.2` and `34`;
- ordinary unblocked frontier: `6.2` and `28`;
- blocked: `4.5` and `20`;
- completed: retain `5.4` and `24`;
- out-of-scope: retain `4.5` and `18`.

Selection remains the first label-placement priority because it reflects the
operator's immediate action. Among unselected nodes, an in-progress issue gets
a contested label slot before a ready issue, a ready issue gets it before an
ordinary frontier issue, and an ordinary frontier issue gets it before a
blocked issue. Completed and out-of-scope nodes retain their existing relative
label priority after active work.

Keep the existing selected label and detail-pane behavior. A selected ready
issue may still say `CURRENT / READY`; this design changes the surrounding
visual clutter, not the label vocabulary.

## Edges and Arrowheads

Preserve every edge's semantic color, dash pattern, direction, particle motion,
direct-only selection membership, and selected-last paint order from PR #87.

Reduce ordinary arrowhead length from `12` to `8` canvas units and half-width
from `6.5` to `4`. Selected edges use the same compact arrowhead dimensions;
selection continues to read through full opacity, painter order, and the
existing `1.7` stroke-width multiplier. Remove the selected arrow-size
multiplier so selection does not make arrowheads busy again.

Do not change edge paths, curve geometry, particle size, topology, or the rule
that only edges directly incident to the selected node receive selected
emphasis.

## Renderer and Model Boundaries

Keep sphere paint, ring paint, click glow, and arrowhead dimensions inside the
imperative canvas renderer. Isolate workflow-label precedence in a small,
purely testable derivation function rather than scattering label checks through
paint code.

The pushed model already carries labels and assignees, but the renderer adapter
currently retains only `readyForAgent`. Extend the narrow renderer ticket model
with the minimum derived workflow flags needed by the renderer. Do not pass the
entire label array into the canvas island or change layout signatures, camera
state, URLs, persistence, server endpoints, or historical event storage.

## Accessibility and Motion

Color is not the only signal:

- in-progress has a breathing ring and stronger size/glow;
- ready has one steady ring;
- blocked and terminal nodes have no outer status ring;
- selection has a neutral outer ring and direct-edge emphasis.

Under reduced motion, freeze the in-progress ring at its midpoint and keep the
click response as a short static glow change. The sphere highlight and shadow
remain visible without animation.

## Verification

Add or update tests that prove:

- terminal states override stale `in-progress` and `ready-for-agent` labels;
- open-state precedence is in-progress, then assigned fallback, then ready,
  then blocked/frontier;
- a blocked issue cannot become ready solely because it has the ready label;
- in-progress and ready nodes have exactly one status ring;
- blocked and completed nodes have no outer status ring;
- selection adds exactly one ring and never produces more than two stable outer
  rings;
- parent/subissue membership adds no node ring;
- click feedback changes glow without adding a ring;
- sphere painting records the highlight, semantic body, shade, shadow, and body
  boundary in the intended order;
- label priority orders selected first, then in-progress, ready, ordinary
  frontier, and blocked;
- ordinary and selected arrowheads both use length `8` and half-width `4`;
- selected edges retain `1.7` stroke scaling, full-opacity selected-last paint,
  semantic styling, and direct-only scope;
- reduced motion freezes rather than removes the in-progress status signal.

The completion gate is the focused derivation and canvas tests, complete
frontend tests, Svelte check, production frontend build, Rust formatting,
workspace Clippy with warnings denied, and locked native Windows workspace
tests. Visual verification must cover an in-progress node, ready node, blocked
node, completed node, selected ready node, and selected in-progress node.

## Non-Goals

This change does not implement a true three-dimensional graph, migrate to a
different rendering library, create or infer tracker assignments, relabel
existing issues, change dependency semantics, highlight transitive paths,
change timeline topology, or redesign the detail pane.

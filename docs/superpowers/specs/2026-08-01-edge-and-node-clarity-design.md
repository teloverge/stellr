# Edge and Node Clarity Design

## Goal

Make every dependency easier to trace on the pure-black star map while keeping
the current conversation path visually dominant. Add a completion channel that
does not depend on color: completed issues are solid and incomplete issues are
hollow.

## Edge Hierarchy

Preserve the existing blocker-to-dependent direction, curved geometry, and
resolved-versus-unresolved semantics. Strengthen the paint grammar as follows:

- unresolved edges use a 2.4 world-pixel line, a `[7, 7]` dash pattern, rounded
  dash caps, and `rgba(174, 192, 218, 0.62)`;
- resolved edges use a 3 world-pixel solid line, rounded caps, and
  `rgba(190, 225, 200, 0.82)`;
- edges on the CURRENT/READY dependency path retain full paint opacity;
- unrelated edges use a 0.45 context multiplier instead of the current 0.2.

The context multiplier applies to the complete edge treatment—line,
arrowhead, and motion—so unrelated dependencies become legible without
competing with the emphasized path.

## Direction Arrows

Increase the midpoint arrowhead from 7 by 3.8 world pixels to 12 by 6.5 world
pixels. Use high-contrast semantic colors:

- resolved: `#d9f3df`;
- unresolved: `#c8d5e8`.

Arrowheads continue to point from blocker to dependent and follow the same
focus/context opacity as their edge.

## Edge Motion

Motion keeps its current meaning: it appears only on resolved edges, showing
that the blocker has cleared and work can flow toward the dependent issue. Do
not add motion to unresolved edges, because that would imply progress through a
still-blocked dependency.

Strengthen the existing motion without changing its speed:

- increase from two to three particles per resolved edge;
- increase particle radius from 1.7 to 2.6 world pixels;
- set particle alpha to `0.45 + 0.5 × sin(πu)`, producing a 0.45–0.95
  envelope along the curve;
- place a soft 5 world-pixel halo behind each particle with alpha
  `0.14 + 0.18 × sin(πu)`, producing a 0.14–0.32 envelope.

The moving core and halo use the resolved-edge mint family and remain subject
to the context multiplier.

## Completion Shape Grammar

Keep the existing status colors and glow radii. Shape becomes an independent
completion channel:

- `resolved` issues retain the existing solid radial-gradient core;
- `frontier`, `blocked`, `claimed`, and `out_of_scope` issues use a black inner
  disk with a status-colored rim, producing a clearly hollow center;
- hollow nodes retain the same outer radius, glow, hit area, label clearance,
  flare, session orbit, and selection behavior as solid nodes.

The current issue remains visually special. Issue #14 keeps its existing bright
double CURRENT rings exactly; only its inner issue core becomes hollow while it
is incomplete. This preserves the treatment the user approved without making
solid mean both “completed” and “current.”

## Renderer Boundary

Keep these changes inside the imperative canvas renderer. Do not change the
GitHub model, focus analysis, deterministic layout, camera fit, pan/zoom,
selection, URL session focus, or server contract.

The renderer continues to derive completion from the existing visual state:
only `resolved` paints a solid core. No new persisted field or dependency is
needed.

## Verification

Canvas recording tests will prove:

- resolved and unresolved edges use the new line widths and dash grammar;
- arrowheads use the larger geometry and brighter colors;
- resolved edges paint three larger motion cores plus halos;
- unresolved edges do not paint motion particles;
- resolved nodes paint a solid core while every non-resolved state paints a
  hollow center and status-colored rim;
- the CURRENT double rings remain present around an incomplete current node.

The complete frontend test, Svelte check, production build, native Rust
workspace checks, embedded-asset probes, and a headed browser screenshot remain
the completion gate.

## Scope

This change does not alter node positions, add background decoration, animate
unresolved dependencies, change issue status derivation, or introduce a new
completion status. It extends the already-approved Issue #14 visual hierarchy.

# Selected Node Edge Emphasis Design

## Goal

Make a selected node's immediate graph relationships easy to trace without
changing the map's workflow semantics, layout, or camera behavior.

## Selection Contract

When an issue is selected, emphasize every rendered edge whose `from` or `to`
endpoint is that issue. This includes both incoming and outgoing edges and
applies to dependency edges as well as parent/subissue entry, sequence, and
return edges.

Selection is local. It does not emphasize edges beyond the selected issue's
direct neighbors or compute a transitive upstream or downstream path. A
selected issue with no incident edges changes no edge rendering. Deselecting
restores the ordinary graph treatment.

## Visual Treatment

Preserve each selected edge's existing semantic grammar:

- keep its resolved, unresolved, or subissue color;
- keep its solid or dashed line pattern;
- keep its arrow direction;
- keep its existing motion eligibility, particle count, speed, and size.

Selected incident edges render with canvas `globalAlpha` set to `1`, even when
the existing CURRENT/READY focus analysis would otherwise classify them as
context. Their semantic stroke and fill colors retain their built-in RGBA
alpha. Their stroke width is exactly `1.7` times the ordinary width, and their
arrowhead length and half-width are exactly `1.25` times the ordinary
dimensions.

Render non-selected edges first using the existing focus/context treatment.
Render selected incident edges once, afterward, using the selected treatment.
This keeps selected connections above crossings without double-painting them.
Unrelated edges retain their current opacity, width, arrowheads, dash pattern,
and motion.

The selected node keeps its existing selection ring and label priority. No new
accent color, glow, or workflow state is introduced.

## Renderer Boundary

Keep the change inside the imperative canvas renderer. Derive incident-edge
membership directly from the renderer's existing selected issue number and
rendered edge endpoints. Do not change the model, adapter, focus analysis,
workflow-edge derivation, layout, camera seating, detail pane, URL state, or
server contract.

Selection remains transient view state. Model refreshes continue to retain a
selection while its issue remains present and drop it when that issue leaves
the map.

## Interaction With Existing Emphasis

Selection is the strongest edge-level emphasis because it reflects the
operator's immediate action. CURRENT/READY path emphasis remains unchanged for
all non-selected edges. When a selected edge is also on the CURRENT/READY path,
it receives the selected width and arrow dimensions without an additional
color or opacity layer.

Historical playback and ordinary status updates may change an edge's semantic
paint, but selection does not move nodes, refit the camera, or alter topology.

## Verification

Extend the canvas recording tests to prove:

- selecting a node emboldens both an incoming and an outgoing direct edge;
- a non-incident edge keeps its ordinary treatment;
- selected edges render at full opacity when CURRENT/READY focus would have
  treated them as context;
- dependency and parent/subissue workflow edges both receive the treatment;
- selected strokes use the `1.7` multiplier and selected arrowheads use the
  `1.25` multiplier while preserving semantic color and dash patterns;
- deselection restores the ordinary edge rendering;
- selecting an isolated node does not change unrelated edges.

The implementation completion gate is the targeted renderer test in red and
green states, the complete frontend test suite, Svelte check, frontend
production build, Rust formatting, Clippy with warnings denied, and the locked
native Windows workspace tests.

## Scope

This change does not highlight transitive dependency chains, dim unrelated
edges beyond their existing focus treatment, emphasize neighboring nodes, add
hover behavior, change labels, add persistence, or modify graph geometry.

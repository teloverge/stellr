# Session-Focus Star Map Design

## Goal

Make the map answer two questions immediately:

1. Where am I working in this conversation?
2. Which unblocked `ready-for-agent` issue can move that work forward?

The graph remains a stable dependency map, but its visual hierarchy becomes an
execution navigator rather than a decorative constellation.

## Session Context Contract

The launcher supplies the conversation's main issue explicitly:

```text
stellr serve --issue 14
```

`--issue` is optional and accepts one positive GitHub issue number. When
present, the generated cockpit URL includes `issue=14` alongside the per-run
token. The existing token-capture logic removes only `token`, leaving `issue`
in the scrubbed browser URL so reconnects and reloads retain the conversation
focus.

The current issue is viewer-session context, not repository state. It therefore
does not become part of the shared GitHub `Model`. The web client reads the URL
parameter and passes it through the `StarMap` wrapper to the renderer.

If `--issue` is absent or the issue is not present in the displayed space,
Stellr shows no CURRENT marker and still prioritizes actionable ready issues.
The M1 cockpit displays its first configured space, so issue numbers resolve
within that space; multi-space session identity remains outside this change.

## Actionable Priority

An issue is actionable when both conditions hold:

- its derived status is `frontier`, meaning it has no unresolved blocker;
- its labels contain `ready-for-agent`, compared case-insensitively.

The renderer receives this as a `readyForAgent` boolean from the adapter rather
than learning GitHub label semantics itself.

Priority order is:

1. the current conversation issue;
2. actionable issues on an unresolved blocker path leading to the current issue;
3. other actionable issues;
4. nodes on the highlighted dependency path;
5. every remaining issue as subdued context.

Dependency edges point from blocker to dependent. To find work that can move a
blocked current issue forward, focus analysis walks recursively upstream through
`blockedBy`, locates actionable ancestors, and retains the edges on their paths
to the current issue. For the current Stellr model, this makes the path
`#8 -> #12 -> #14` explicit: #14 is CURRENT and #8 is READY.

If no actionable ancestor exists, all actionable issues remain READY and share
second priority. If the current issue is itself actionable, it carries both
meanings but renders one CURRENT marker with READY included in its label.

Focus analysis is a pure module. It consumes tickets and an optional current
issue number, then returns the current node, ordered ready nodes, path nodes,
and highlighted edges. Layout remains independent of status, labels, and focus.

## Visual Hierarchy

- Keep the pure-black canvas.
- Remove the decorative parallax starfield completely.
- Preserve the existing status colors and animations as the node's base state.
- Multiply every issue core radius by 1.25 at the renderer seam; use the scaled
  radius for drawing, hit testing, selection rings, sessions, and label
  obstacles. Keep glow radii unchanged so larger nodes do not create more haze.
- Increase the adaptive label scale from the current 8-13 px range to 10-16 px.
- Render the current issue with a high-contrast double ring and a persistent
  `CURRENT · #N title` label.
- Render actionable issues at full opacity with persistent
  `READY · #N title` labels. The current actionable issue uses
  `CURRENT / READY · #N title`.
- Render nodes on the actionable path at full opacity.
- Render unrelated nodes and labels at 30 percent opacity; they remain present,
  selectable, and available through pan and zoom.
- Render highlighted path edges at their existing opacity and multiply unrelated
  edge opacity by 0.2.

CURRENT and READY are encoded by words and ring shape, not color alone.

## Density and Framing

Do not alter the deterministic seeded positions or make layout depend on issue
status. Stars must not jump when GitHub state changes.

Initial fit uses the current issue, actionable ready issues, and highlighted
path as its focus set. It uses 150 world units of fit padding instead of the
existing 90, multiplies the resulting fit scale by 0.8, and caps that scale at
1.0. This keeps the focus cluster compact while leaving room for the larger
nodes and labels. When there is no focus set, Stellr falls back to fitting the
complete graph with its existing padding and scale behavior.

The complete graph is still drawn. Panning, zooming, selection, resize behavior,
and stable positions remain unchanged.

## Data Flow

```text
Codex or Claude conversation
    -> stellr serve --issue N
    -> cockpit URL ?token=...&issue=N
    -> token removed, issue retained
    -> App passes current issue to StarMap
    -> adapter marks frontier + ready-for-agent tickets actionable
    -> pure focus analysis finds current, ready, and dependency path
    -> renderer applies framing and visual emphasis without moving nodes
```

## Verification

- CLI tests cover default `None`, `--issue N`, invalid zero, and generated URLs
  with and without session authentication.
- Token URL tests prove `issue=N` survives token removal.
- Adapter tests prove only `frontier` plus `ready-for-agent` becomes actionable.
- Focus-analysis tests cover an actionable blocker chain, global fallback,
  current-is-ready, missing current, and cycles.
- Renderer tests prove a blank graph draws no decorative points, CURRENT and
  READY labels win crowded placement, unrelated context is subdued, and status
  pushes do not move nodes.
- Existing label-collision, selection, camera, session-overlay, API-auth, and
  embedded-asset tests remain green.
- A rebuilt native server is checked in VS Code Simple Browser at the real
  cockpit URL for legibility and the expected `#8 -> #12 -> #14` emphasis.

## Scope

This change does not infer issues from branch names, inspect Codex internals,
add terminal/session management, hide repository issues, change GitHub status
derivation, or implement multi-space focus identity. Those remain compatible
with the planned M3 session model.

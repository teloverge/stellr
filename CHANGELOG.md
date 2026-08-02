# Changelog

## Unreleased

- Added native GitHub subissue mini-graphs with directed parent-entry,
  sibling-sequence, and child-return arrows; violet dashed incomplete routes
  and rims; resolved completed grammar; and motion only on traversed
  completed-to-ready routes.
- Added the M1 application chrome: complete space add/select/refresh/remove
  controls, visible stale and provider-error state, and restorable hash-routed
  issue detail rendered through sanitized Markdown with responsive docking.
- Strengthened dependency lines, arrows, and resolved-edge motion, and made
  incomplete issue cores hollow while completed issues remain solid.
- Refocused the star map on the current conversation issue and actionable
  `ready-for-agent` paths, with denser issue markers and no decorative starfield.
- Made the embedded star map use a pure-black default background so its stars,
  labels, and dependency paths retain their intended contrast in browser panes.
- Added `stellr serve` with an embedded SPA, per-run session tokens, and an
  explicit `--no-token` mode.
- Kept embedded UI assets loadable in iframe-based browser panes while API and
  control WebSocket routes remain token-protected, and removed the token from
  browser history after the client captures it.

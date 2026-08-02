# Changelog

## Unreleased

- Added a space lifecycle sidebar with stale/offline status, plus routed,
  responsive, sanitized issue detail, canonical restorable deep links, and safe
  GitHub navigation from the star map.
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

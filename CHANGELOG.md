# Changelog

## Unreleased

- Added fixed-duration full-history playback with proportional clustered event
  ticks, accessible play/pause and speed controls, slow-frame-safe event
  feedback, and reduced-motion captions that preserve map position and selection.
- Added exact milestone assignment, movement, and removal replay with minimal
  GitHub event payloads and temporal milestone hulls that reshape around fixed
  star coordinates without moving the constellation or camera.
- Added resumable, one-issue-at-a-time close/reopen history import with
  transactional page checkpoints, stable lifecycle replay, and schema migration
  from creation-only ledgers without repository-wide history refetches.
- Added a durable local SQLite issue-creation ledger, authenticated history
  deltas, and a bottom timeline that scrubs historical issue visibility without
  moving the constellation or querying GitHub during scrubbing.
- Fixed installed desktop startup so a no-argument launch opens the existing
  empty repository-selection shell instead of treating the installation
  directory as a Git repository.
- Added the M2 native desktop shell with GitHub device authorization, OS-backed
  credential persistence, single-instance deep-link routing, focus-aware
  synchronization, restorable window and route state, native tray/settings
  actions, and gated Windows, universal macOS, and Linux packages.
- Published the first real M1 release constellation from the frozen live GitHub
  issue history, with immutable SVG, PNG, and story evidence linked from the
  README through animated, reduced-motion, and strict-Markdown paths.
- Added explicit digest-gated acceptance for reviewed release previews, with
  immutable versioned SVG, PNG, and story assets plus a README-last atomic
  publication step and exact unreferenced-asset failure reporting.
- Added a native fail-closed live release preview command that acquires complete
  GitHub evidence, proves byte determinism, validates all four review outputs,
  and atomically exposes them under the ignored `target/readme-showcase` tree.
- Added a truthful twelve-second release replay with fixed graph geometry,
  evidence-keyed synchronized status beats, CURRENT and READY focus, motion only
  on newly traversable resolved edges, a two-second final hold, soft reset, and
  a deterministic reduced-motion final state.
- Added deterministic final-scene release previews with a safe 1200-by-675 SVG,
  matching 1600-by-900 PNG, canonical manifest, self-contained review page,
  bounded accessible labels, fixed asset budgets, and a bundled raster font.
- Added a read-only live GitHub release-history source with complete milestone,
  release, issue, blocker, and lifecycle pagination; explicit first/later release
  boundaries; inherited typed provider failures; and manifest privacy checks.
- Added deterministic, auditable release-story manifests with explicit UTC
  boundaries, lifecycle reconstruction through Stellr's core status derivation,
  hidden blocker support, bounded beat grouping, and precise fail-closed
  evidence diagnostics.
- Added a script-free animated README constellation compatibility probe with a
  reduced-motion PNG and strict-Markdown fallback path.
- Compacted native subissue workflows into deterministic parent-local arcs
  that avoid unrelated nodes and dependency paths, with bounded overflow,
  nested-cluster placement, and safe malformed-data fallbacks.
- Added native GitHub subissue mini-graphs with directed parent-entry,
  sibling-sequence, and child-return arrows; violet dashed incomplete routes
  and rims; resolved completed grammar; and motion only on traversed
  completed-to-ready routes.
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

# Changelog

## Unreleased

- Kept the authenticated Tailnet browser session stable across launcher-driven
  rebuilds and restarts so existing browser tabs can reconnect, and replaced
  endless retries for invalid sessions with an explicit expiration message;
  rebuilt executable inodes are now recognized when replacing the old server.
- Added an npm-accessible Tailnet launcher that safely replaces the prior
  instance and binds the optimized web server to the current Tailscale IPv4
  address with session authentication enabled, while retaining the latest
  authenticated URL in an owner-only local file.
- Added Linux shell, Windows PowerShell, and Windows Command Prompt helpers for
  retrieving that authenticated URL from the `amd-halo` Tailnet host over SSH.
- Added a Debian/Ubuntu dependency installer for optional native desktop and
  package builds using Tauri, WebKitGTK, D-Bus, and app indicators.
- Updated the locked DOMPurify and Nano ID dependencies to versions that pass
  the npm security audit.
- Added `.nvmrc`, npm package metadata, and strict engine checks requiring
  Node.js 24 with npm 12.0.2 before installing the web workspace.
- Made the browser-hosted server the default Linux development build, with
  Tauri and OS credential storage isolated behind an explicit desktop feature.
- Suspended star-map GPU rendering while the app is minimized or its browser
  document is hidden, while retaining five-minute native background polling.
- Made project changes immediate and responsive: first-time constellation
  layouts now show cancellable elapsed-time progress off the UI thread, while
  completed layouts are cached for instant return visits and failures restore
  the last successfully charted project.
- Reworked the star map around viewer-aware work priorities: current and owned
  work now remains prominent when zoomed out, team and closure states stay
  distinct, account changes cannot reuse stale ownership, and completed paths
  remain quiet except for subtle directional motion into immediately actionable
  nodes.
- Kept ready subissue labels visible and clear of their emphasis rings while
  the star-map camera eases.
- Declared npm 12.0.2 as the web workspace's development package manager and
  activated it across CI, bundle, and release workflows.
- Made newly added repositories appear in the sidebar without restarting
  Stellr or performing another space action.
- Emphasized the incoming and outgoing edges directly connected to a selected
  node while preserving dependency direction, state styling, and motion.
- Replaced compact subissue arcs with adaptive, label-aware concentric orbits,
  outward titles, and larger nearest-node pointer targets for dense workflows.
- Restored dependency and parent relationship lines from cached Markdown issue
  bodies without making additional GitHub requests.
- Upgraded all bundle artifact uploads to `actions/upload-artifact@v7` and
  downloads to `actions/download-artifact@v8`, removing the obsolete
  action-runtime warnings without suppressing them.
- Hardened Windows application startup smokes with a measured 90-second cold
  start budget and captured native startup-stage diagnostics on failure.
- Split the Windows desktop and CLI entry points so installed shortcuts and
  protocol launches open without a terminal while `stellr serve` retains
  native PowerShell and Command Prompt behavior.
- Hardened temporal playback with provider-backed verification cutoffs,
  cutoff-safe cursors, native offline restarts, durable import resume, migration
  rollback, space-scoped deletion, bounded rate-limit retry, neutral historical
  workflow styling, accessible status text, focus, and narrow layouts.
- Added delta-only background history synchronization using ordinary snapshot
  metadata, resumable per-issue cursors, catch-up verification, rate-limit reset
  evidence, idempotent client merging, and a pinned-past New activity action.
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

# Reference assets from chartr

Files copied verbatim from [chartr](https://github.com/rengwu/chartr)
(MIT, Copyright (c) 2026 John Goh — see `../LICENSE-chartr`). They are staged
here until the port moves them into their final locations, per the design spec
(`../docs/specs/2026-07-29-stellr-port-design.md`).

- `starmap/` — the canvas star-map renderer and its headless test suite, from
  chartr `web/src/lib/starmap/`. The tests are the renderer's contract; port
  them first. Ports into stellr's `web/src/lib/starmap/`.
- `detect-manifests/` — per-agent TOML manifests (claude, codex, grok, kimi,
  opencode, pi) driving agent-state detection, from chartr
  `internal/terminal/detect/manifests/`. Port as data files, unchanged, into
  `crates/term`.
- `docs/` — chartr's ADRs 0001–0016 plus `design-system.md`,
  `getting-started.md`, and `skill-sync.md`. The stellr spec cites several by
  number (0008 write restraint, 0010 chrome/island split, 0012 design-system
  discipline, 0013 webview seam); read the cited ADR before re-deciding
  anything it covers. chartr's icon/branding assets were deliberately not
  copied — stellr gets its own identity.
- `chartr-ci/` — chartr's `ci.yml` and `release.yml` workflows. Mostly
  obsoleted by Tauri's bundler, but the release-gating ideas (smoke test as a
  release gate, checksummed artifacts) inform M2's pipeline.
- `plan-maps/` — chartr's own wayfinder planning maps for the features stellr
  ports in M3/M4: `agent-state-detection`, `agent-selection`,
  `session-notifications` (+ `-impl`; chartr never shipped it — stellr M4
  builds it natively via Tauri), `sidebar-order`, `terminal-customization`.
  Tickets' `## Answer` sections hold the design rationale; chartr-specific
  visual-design maps were not copied.

Files ported substantially intact keep a provenance header noting they derive
from chartr (MIT).

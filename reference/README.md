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

Files ported substantially intact keep a provenance header noting they derive
from chartr (MIT).

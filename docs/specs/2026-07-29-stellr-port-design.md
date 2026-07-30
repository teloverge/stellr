# stellr — design spec

**Date:** 2026-07-29
**Status:** Approved (design review with project owner)
**Origin:** Port of [chartr](https://github.com/rengwu/chartr) (Go + embedded Svelte SPA)
to Rust/Tauri, generalized from local wayfinder maps to GitHub Issues, with a
full UX redesign. This document is the spec for the new project; the Go repo at
`D:\dev\chartr` is the reference implementation.

## 1. Product definition

stellr is a desktop app (Tauri 2) that charts a GitHub repository's issues as an
interactive **star-map** — GitHub "blocked by" relationships as edges, milestones
as clusters, closed issues as resolved stars — and doubles as an **agent
multiplexer**: pick an unblocked issue off the frontier, pick an agent CLI
(claude, codex, opencode, kimi, grok, pi, or anything on `PATH`), and a terminal
session opens with the issue, its blockers' resolutions, and repo context already
composed into the agent's opening prompt.

The same binary runs headless (`stellr serve`) so the UI can be opened in VS
Code's Simple Browser, t3code's preview pane, or any browser — the cross-IDE
portability story, with no editor extension to maintain.

Constraints carried over from chartr's philosophy:

- **Windows 11 is a first-class, daily-driven platform** (chartr treats Windows
  as best-effort; stellr does not).
- No hosted service, no account, nothing phones home. GitHub's API is the only
  network dependency, and only for repos the user adds.
- The tool's writes to shared state are minimal and reversible (see §3).

Out of scope for v1: a VS Code extension, GitLab/Linear/Jira providers, the
local-markdown wayfinder provider, multi-user coordination beyond what GitHub
assignment already gives.

## 2. Architecture

Fresh repository, standard cargo workspace:

```
stellr/
├── Cargo.toml            # workspace root
├── crates/
│   ├── core/             # pure domain: issue graph, status + frontier
│   │                     #   derivation, Provider trait — no I/O
│   ├── github/           # Provider impl: auth, sync, dependency edges,
│   │                     #   claim/release writes, on-disk snapshot cache
│   ├── term/             # PTY (portable-pty), VT screen model (termwiz),
│   │                     #   agent launch adapters, agent-state detection
│   ├── server/           # axum: REST + JSON control websocket +
│   │                     #   binary per-terminal websocket, model hub
│   └── app/              # Tauri 2 shell + CLI entry (window & serve modes);
│                         #   tauri.conf.json lives here
├── web/                  # Svelte 5 + Vite SPA (ported from chartr, redesigned)
├── docs/adr/             # ADR habit carries over; ADR 0001 records this port
└── .github/workflows/    # CI + tauri-action release pipeline
```

### Process & transport model

- The Tauri app embeds an **axum** server bound to `127.0.0.1:0` (random port).
  The webview loads the SPA from it. Requests carry a per-run bearer token
  injected into the webview at startup; `serve` mode binds a user-chosen
  `--addr` and prints the URL (token included as a query parameter that the SPA
  exchanges for a session).
- **Two-socket transport**, identical in shape to chartr (its ADR 0010):
  - `/ws/control` — JSON, server-authoritative, whole-model snapshot on every
    change, resent on reconnect.
  - `/ws/terminal/{id}` — binary, one per attached terminal; raw PTY bytes
    down, keystrokes up; server-side scrollback replayed on attach; slow
    consumers are dropped rather than back-pressuring the PTY.
  Terminal bytes never ride Tauri's JSON event IPC (known performance trap);
  a flooding shell can never head-of-line-block map updates.
- **Tauri IPC is used only for native concerns:** window/tray, OS
  notifications, file dialogs, opening external URLs, single-instance focus,
  global shortcut. This replaces chartr's `__chartrOpenExternal` /
  `__chartrTitleBar` webview globals.
- `core` exposes a `Provider` trait (project reference in → issue graph +
  capability flags out; claim/release hooks). GitHub is the only shipped
  implementation in v1. The trait exists to keep the door open, not to promise
  other providers.

### CLI surface

```
stellr                    # open (or focus) the desktop app on the cwd's repo
stellr serve [--addr ..]  # headless server for browser / IDE-pane use
stellr open <path|url>    # open a specific space or stellr:// deep link
stellr --version
```

Single-instance behaviour: launching `stellr` while the app runs focuses the
existing window and switches it to the requested space (Tauri single-instance +
deep-link plugins). `stellr://space?...&issue=N` deep links resolve the same way.

## 3. GitHub data layer (`crates/github`)

### Auth

1. Reuse the `gh` CLI token when available (`gh auth token`) — zero setup for
   most developers.
2. Otherwise, GitHub **device flow** in-app.
3. Tokens are stored in the OS keychain (`keyring` crate) — never plaintext on
   disk. Required scopes: `repo` (read/write issues, read metadata).

### Sync

- GraphQL via `octocrab`: issues, milestones, labels, assignees, GitHub's
  native **issue dependencies** ("blocked by" / "blocking") and sub-issues.
- Textual fallback for repos not using native dependencies: `Blocked by #N` /
  `Blocks #N` lines and task-list references in issue bodies are parsed into
  edges (fenced code blocks excluded, as chartr's parser does for markdown).
- Polling with conditional requests: ~30 s while the window is focused, backed
  off (5 min) when idle or unfocused; manual refresh always available; rate-limit
  headers respected with visible degradation, never silent failure.
- Per-repo snapshots cached in the app data dir so the map renders instantly on
  launch and works offline (staleness indicated in the UI).

### Mapping (issue → star)

| GitHub state | Star status |
| --- | --- |
| open, a blocker still open | `blocked` (dim, small) |
| open, all blockers closed, unassigned | `frontier` (bright, large) |
| open, assigned | `claimed` |
| closed (completed) | `resolved` |
| closed as not-planned | `out_of_scope` |

- Edges: native dependencies first, textual fallback second; duplicates merged.
- Milestone → visual cluster; issues without a milestone form the unclustered
  field.
- Labels → session role hints (e.g. `research`, `prototype` labels pre-select
  the agent role, mirroring chartr's ticket `type:`).

### Writes (deliberately minimal, mirroring chartr ADR 0008)

- **Claim** = assign the authenticated user + a marker comment identifying the
  stellr session. **Release** = unassign (+ closing marker comment).
- The **agent** resolves an issue by closing it through its own tooling
  (`gh issue close`, MCP, etc.) — stellr itself never closes issues and never
  edits issue bodies or titles.

### Spaces

A **space** is a local repo directory; `owner/repo` is detected from the
`origin` remote (overridable). A space may also be a bare `owner/repo` with no
local checkout — such spaces are **map-only** (terminals need a working
directory). Spaces persist in stellr's config dir (TOML, like chartr's
`spaces.toml`).

## 4. Terminal subsystem (`crates/term`)

Feature-for-feature port of chartr's `internal/terminal`:

- **PTY:** `portable-pty` (wezterm's layer) — ConPTY on Windows,
  openpty on Unix. The Unix parent-side slave-fd release fix (chartr's
  `slave_unix.go`, the Linux exit-reaping bug) is replicated; on Windows the
  issue is moot but ConPTY resize/exit semantics get their own tests.
- **Screen model:** `termwiz` reconstructs the grid; an OSC scanner pulls
  OSC 0/2 titles and OSC 9 progress from the raw stream.
- **Agent-state detection:** data-driven priority rules — (title, progress,
  screen region) → `working` / `blocked` / `idle` — loaded from per-agent TOML
  manifests. chartr's existing manifests (claude, codex, opencode, kimi, grok,
  pi) port **verbatim as data files**. The asymmetric hysteresis publisher
  (positive signals publish immediately; absence needs repeated confirmation
  plus a startup grace period) ports with them — it is what makes the sidebar's
  "waiting on you" trustworthy.
- **Launch adapters:** one model — how a binary takes its opening line — with
  argv, `--flag`, and typed-into-the-PTY delivery modes. Anything on `PATH`
  registers; known agents are auto-detected. Login-shell `PATH` hydration ports
  (source of "works in my terminal, not in the app" bugs).
- **Sessions:** a session binds one terminal to one issue. A dead session stays
  pinned to its issue; the operator chooses resume / respawn / release — stellr
  never auto-acts. Ad-hoc shells (no issue) and skill/on-ramp launches also use
  the same terminal primitive.
- **Spawn context bundle**, composed fresh per spawn (no memory store): issue
  title/body, its blockers' titles and closing comments, the repo's agent
  conventions file if present (`AGENTS.md`, `CLAUDE.md`, or `CONTRIBUTING.md`,
  first found), and the chosen role's prompt. Archived per-run under a gitignored app-data path for
  auditability.

## 5. Frontend & UX redesign (`web/`)

Structural port, visual redesign. Svelte 5 (runes) + Vite + TypeScript +
Tailwind v4 + vendored shadcn-svelte primitives + phosphor icons + xterm.js
(with webgl/canvas/fit/search addons). No SvelteKit, no SSR, no CDN — all
assets bundled.

- The **star-map stays an imperative canvas island** behind its narrow seam
  (mount / receive model / emit selection). chartr's ~1.7 kLOC renderer and its
  876-line headless test suite port as the contract, then get extended.
- **Chart readability at issue scale** (hundreds of issues, not twenty
  tickets): smooth zoom/pan with level-of-detail labels, milestone cluster
  hulls, a minimap, live search/filter that dims non-matches, deterministic
  seeded layout so stars never move between sessions (chartr's invariant:
  status is not a layout input).
- **Native feel:** window state restore, system tray, OS notifications when a
  session that ran longer than a configurable threshold lands / blocks / dies
  (chartr's unshipped "it's done" spec, done natively via Tauri), global
  summon shortcut.
- **Streamlined workflows:** Ctrl+K command palette reaching every action;
  full keyboard navigation of the map; claim-and-spawn from a focused star in
  two keystrokes; issue peek pane with open-in-browser and open-in-editor.
- **Visual polish:** a fresh token-based theme designed for first-class light
  and dark modes. The discipline carries over — every colour a token, every
  component a vendored primitive, chrome monochrome with a single emphasis
  hue — but the olive palette does not. Star-map status hues remain exempt
  data-viz colour behind the token seam (chartr's `tokens.ts` pattern).
- Terminal theming flows through a token→xterm-theme resolve seam; the
  renderer is never edited to re-theme it.

## 6. Packaging, CI, releases

- **Tauri bundler** replaces chartr's Makefile/goreleaser/AppImage-relocation
  machinery entirely: NSIS installer for Windows 11 (WebView2 preinstalled
  there), `.dmg` for macOS, AppImage + `.deb` for Linux.
- CI (GitHub Actions): build + test matrix on `windows-latest`,
  `macos-latest`, `ubuntu-latest`; ConPTY round-trip smoke test on Windows on
  every change; `cargo clippy -D warnings`, `cargo fmt --check`, frontend
  `check`/`build`/vitest gate merges. Releases via `tauri-action` on tags.

## 7. Testing strategy

| Layer | Approach |
| --- | --- |
| `core` | Dense unit suite on graph/status/frontier derivation — the port's equivalent of chartr's 537-line parser spec. Property tests on frontier monotonicity (closing a blocker never shrinks the frontier). |
| `github` | Integration tests against a mock GitHub (wiremock): pagination, dependency edges, textual fallback, rate-limit and offline paths. |
| `term` | Detection engine on captured byte-stream fixtures per agent; PTY round-trip tests per OS; ConPTY smoke on Windows CI. |
| `server` | Route + websocket tests; snapshot-on-reconnect; slow-consumer drop. |
| `web` | Ported star-map contract suite (headless canvas degradation); vitest + svelte-check. |

## 8. Build order (milestones)

1. **M1 — the chart.** `core` + `github` + `server` + ported star-map; `serve`
   mode only. Deliverable: a usable read-only GitHub-issue star-map in any
   browser or IDE pane, on Windows 11.
2. **M2 — the shell.** Tauri window, redesigned theme, tray, single-instance,
   deep links, installers for all three OSes.
3. **M3 — the multiplexer.** `term` crate, ad-hoc shells, claim/spawn/release
   sessions off the frontier.
4. **M4 — the senses.** Agent-state detection, session notifications, command
   palette, map-at-scale polish (clusters, minimap, LOD).

Each milestone is releasable; M1 alone delivers the cross-IDE issue chart.

## 9. Licensing & attribution

chartr is MIT-licensed (Copyright (c) 2026 John Goh). Portions of stellr are
direct ports or verbatim copies of chartr code — at minimum the star-map canvas
renderer and its test suite, the agent detection TOML manifests, and the shape
of the Go backend's logic — which are "substantial portions" under the MIT
terms. stellr therefore must, and will:

- Ship its own `LICENSE` (MIT) **plus** retain chartr's copyright and
  permission notice — a `LICENSE-chartr` (or `NOTICE`) file at the repo root
  containing chartr's full MIT text, referenced from `LICENSE`.
- Credit chartr (and its own lineage: wayfinder-maps, herdr, the `/wayfinder`
  skill) in the README's related-projects/acknowledgements section with a link
  to the upstream repo.
- Keep ported-file provenance honest: files ported substantially intact carry a
  header comment noting they derive from chartr (MIT).

This is a condition of the port, not optional polish; it lands in M1 with the
repo scaffold.

## 10. Decisions log

- Full port (multiplexer included), not chart-only. — owner, 2026-07-29
- GitHub Issues is the sole v1 data source; wayfinder maps dropped (Provider
  trait keeps the seam). — owner, 2026-07-29
- Tauri-first desktop app; embedded axum server in-process (not pure IPC, not
  sidecar); `serve` mode preserved for IDE-pane portability. — owner, 2026-07-29
- Fresh repository; Go repo is reference only. — owner, 2026-07-29
- Name: **stellr**. — owner, 2026-07-29
- Attribution per chartr's MIT license (retained notice, README credit,
  provenance headers). — owner, 2026-07-29

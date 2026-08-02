# stellr

**A star-map of your GitHub issues, and an agent multiplexer to work them.**

stellr charts a repository's issues as an interactive star-map — "blocked by"
relationships as edges, milestones as clusters, closed issues as resolved
stars — and lets you take an unblocked issue off the frontier, pick an agent
CLI, and open a terminal session with the issue and its blockers' resolutions
already composed into the opening prompt.

Rust + Tauri 2 desktop app for Windows 11, macOS, and Linux; the same binary
runs headless (`stellr serve`) so the UI can live in VS Code's Simple Browser,
t3code's preview pane, or any browser.

**Status: M1 is available.** The headless server synchronizes GitHub issues and
serves the interactive star-map in a browser or IDE pane. The approved design
spec remains at
[`docs/specs/2026-07-29-stellr-port-design.md`](docs/specs/2026-07-29-stellr-port-design.md).

## Quick start (M1)

Install the stable Rust toolchain, Node.js 24 with npm, and the GitHub CLI.
Authenticate with `gh auth login`, or set `GITHUB_TOKEN` in the environment.

From the repository root in PowerShell:

```powershell
npm.cmd --prefix web ci
npm.cmd --prefix web run build
cargo.exe run -p stellr-app -- serve
```

Open the printed `stellr cockpit` URL in any browser. In VS Code, use
**Simple Browser: Show** and paste the same URL. Add a space from the sidebar
using a local repository path or an `owner/repo` name; stellr synchronizes its
issues and renders them as the star-map.

## Lineage & acknowledgements

stellr is a Rust/Tauri port and generalization of
[chartr](https://github.com/rengwu/chartr) (Go, MIT, © 2026 John Goh) — the
agent multiplexer with a map of the work. Portions of stellr are direct ports
of chartr code; chartr's license notice is retained in
[`LICENSE-chartr`](LICENSE-chartr).

Further upstream:

- [wayfinder-maps](https://github.com/rengwu/wayfinder-maps) — where the
  star-map started
- [herdr](https://github.com/ogulcancelik/herdr) — the terminal agent
  multiplexer that inspired chartr
- [mattpocock/skills](https://github.com/mattpocock/skills) — the original
  `/wayfinder` skill and planning method

## License

MIT — see [LICENSE](LICENSE), which incorporates the retained chartr notice
([LICENSE-chartr](LICENSE-chartr)).

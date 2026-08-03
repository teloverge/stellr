# M1 Release Constellation Validation

**Issue:** #54

**Artifact version:** `m1`

**Milestone:** `M1 — the chart`

## Frozen evidence boundary

- Bootstrap cutoff: `2026-07-31T00:00:00Z`
- First recorded issue event: `2026-07-31T00:34:52Z`
- Final recorded lifecycle event: `2026-08-02T21:56:45Z`
- Ending cutoff: `2026-08-03T00:44:50Z`

The bootstrap cutoff predates the first M1 issue. The ending cutoff follows the
closure of #17, the final M1 issue, and is the explicit live-evidence boundary
previously exercised by the fail-closed preview workflow.

## Reviewed publication unit

The native preview command returned this domain-separated review digest:

`sha256:9ce441afcfa485783f234d73f56521692b45901542bd8db7665e1f2dca1e445c`

The exact digest was accepted. The immutable tracked assets are:

| Asset | Bytes | SHA-256 |
| --- | ---: | --- |
| `m1.svg` | 119,995 | `1a2f129eaed6a8a17c9540a3a8b9d4830567971c120248ee10ff5376337c34e9` |
| `m1.png` | 127,474 | `a8438622a990c6bcde6cdb553e42124a0d8b65fd2839ad21416fd859ad223b5a` |
| `m1-story.json` | 20,628 | `f74fea3ac424456458c98d41a1a7d7dc1346c12099e299b8d19174f22955a6ee` |

Acceptance revalidated all four preview files and wrote the three versioned
assets before atomically replacing the README reference.

## Story evidence

- Visible issues: #1 through #17, all members of M1.
- Direct external prerequisites: none.
- Hidden derivation-support issues: none.
- Final blocker edges: 23.
- Normalized lifecycle events: 34.
- Replay beats: 8, and every beat lists its provider event IDs in the manifest.
- Final state: all 17 visible issues are `resolved`.

The opening events create the 17 milestone issues. Subsequent beats contain the
live GitHub closure event IDs in chronological order; the final two beats are
#16 at `2026-08-02T21:16:42Z` and #17 at `2026-08-02T21:56:45Z`.

## Local browser evidence

The generated `review.html` was opened through bundled native Playwright using
Microsoft Edge and Firefox on Windows 11.

| Browser/mode | Viewport | Result |
| --- | --- | --- |
| Edge, motion | 1440 x 1000 | 1202 x 677 responsive SVG, 17 stars, 23 edges, animation active, no page errors or horizontal overflow |
| Edge, motion | 480 x 900 | 434 x 245 responsive SVG, no page errors or horizontal overflow |
| Edge, reduced motion | 1200 x 900 | replay hidden, animation disabled, final scene opacity 1, 17 stars and 23 edges visible |
| Firefox, motion | 1440 x 1000 | SVG and animation loaded with 17 stars and 23 edges, no page errors or horizontal overflow |

The Firefox run used the locally installed Playwright Firefox build directly
because the bundled library expected a newer cached revision. No additional
Firefox degradation was observed: the SVG loaded at its intrinsic dimensions
without page errors, and reduced motion selected the PNG fallback.

## README delivery paths

The accepted README block uses:

- `docs/assets/readme-showcase/m1.svg` as the primary animated image;
- `docs/assets/readme-showcase/m1.png` for `prefers-reduced-motion`;
- an ordinary Markdown link to `m1.png` for strict renderers;
- adjacent text stating that 17 visible issues are resolved.

## Rendered GitHub evidence

The publication commits were pushed to
`codex/issues-47-54-readme-showcase`, and Playwright loaded the rendered branch
README from GitHub with anonymous browser contexts.

| Browser/mode | Viewport | GitHub result |
| --- | --- | --- |
| Edge, motion | 1440 x 1000 | HTTP 200; `currentSrc` is `m1.svg`; intrinsic 1200 x 675; rendered 838 x 471; no page errors or horizontal overflow |
| Edge, motion | 480 x 900 | HTTP 200; `currentSrc` is `m1.svg`; rendered 414 x 233; no page errors or horizontal overflow |
| Edge, reduced motion | 1200 x 900 | HTTP 200; `currentSrc` is `m1.png`; intrinsic 1600 x 900; rendered 758 x 426 |
| Firefox, motion | 1440 x 1000 | HTTP 200; `currentSrc` is `m1.svg`; intrinsic 1200 x 675; rendered 838 x 471; no page errors or horizontal overflow |
| Firefox, reduced motion | 1200 x 900 | HTTP 200; `currentSrc` is `m1.png`; intrinsic 1600 x 900; rendered 758 x 426 |

The ordinary Markdown link resolved to GitHub's versioned `m1.png` blob page,
returned HTTP 200, and rendered the exact PNG at its intrinsic 1600 x 900
dimensions. This separately proves the strict-Markdown path rather than
inferring it from the `<picture>` source.

## Native Windows gates

All completion gates ran from the isolated native Windows worktree:

| Command | Result |
| --- | --- |
| `cargo.exe fmt --all -- --check` | Passed |
| `cargo.exe clippy --workspace --all-targets --locked -- -D warnings` | Passed with no warnings |
| `cargo.exe test --workspace --locked` | 137 passed, 0 failed, 2 intentional ignores |

The test suite includes the release-story, deterministic SVG/PNG, live preview,
README contract, and digest-gated atomic acceptance checks used by this
publication.

## Native commands

```powershell
cargo.exe run -p stellr-showcase -- preview `
  --milestone 'M1 — the chart' `
  --version 'm1' `
  --from-cutoff '2026-07-31T00:00:00Z' `
  --cutoff '2026-08-03T00:44:50Z'

cargo.exe run -p stellr-showcase -- accept `
  --preview 'target\readme-showcase\m1' `
  --digest 'sha256:9ce441afcfa485783f234d73f56521692b45901542bd8db7665e1f2dca1e445c'
```

# Animated release replay validation

**Date:** 2026-08-02

**Issue:** #51 - Animate the truthful twelve-second release replay

**Environment:** Native Windows 11, `cargo.exe`, integrated browser attempt,
bundled Playwright with the installed Microsoft Edge channel

## Contract exercised

The SVG keeps one fixed `1200 x 675` graph and adds a script-free twelve-second
CSS replay over the explicit final-state layer:

| Time | Phase | Verified behavior |
| --- | --- | --- |
| 0-1 seconds | Reveal | The opening state and heading rise from muted to full prominence. |
| 1-9 seconds | Replay | Manifest-backed status frames, focus, captions, and resolved-edge motion replay. |
| 9-11 seconds | Final hold | The final release scene remains available for inspection. |
| 11-12 seconds | Soft reset | The final scene crossfades to the muted opening state. |

Every frame reuses the canonical node coordinates and final topology. The CSS
contains no transform animation; paint changes use opacity while resolved-edge
particles use stroke dash offset. The final layer is the default presentation
when animation is unavailable. A `prefers-reduced-motion: reduce` rule hides the
replay and forces that final layer to full opacity.

The SVG safety gate independently parses every keyframe declaration and allows
only opacity, fill, stroke, stroke dash array, and stroke dash offset. Transform,
`x`, `y`, path, and other geometry-changing animation properties fail closed.

## Evidence mapping

The fixture exercises three deterministic beats:

| Beat | Replay offset | Source evidence | Synchronized changes | CURRENT | READY | New edge motion |
| ---: | ---: | --- | --- | --- | --- | --- |
| 0 | 2,666 ms | `C10` | `#10`, `#20` | `#10` resolved | `#20` | `#10 -> #20` |
| 1 | 5,333 ms | `A20` | `#20` | `#20` claimed | None | None |
| 2 | 8,000 ms | `C20` | `#20`, `#30` | `#20` resolved | `#30` | `#20 -> #30` |

Generation fails closed when a beat's source event IDs do not exactly match the
normalized manifest events assigned to that beat. CURRENT precedence is
claimed, resolved, frontier, then lowest issue number. READY includes only
issues newly entering the frontier.

Before SVG generation, the renderer also rebuilds the entire canonical story
from the recorded issue snapshots and lifecycle events. A manifest that keeps
real event IDs but substitutes invented initial, beat, final, topology, or
coordinate state fails the canonical comparison.

## Deterministic outputs

```powershell
cargo.exe test -p stellr-showcase --test static_preview
cargo.exe test -p stellr-showcase --test static_preview `
  write_local_static_preview_for_visual_review -- --ignored --exact
```

The focused suite passed five ordinary tests; the explicit ignored writer also
passed. The current outputs are:

| Review output | Bytes | SHA-256 |
| --- | ---: | --- |
| `final.svg` | 15,581 | `0F33CF093DF3766F4C75DE84275DDCA04F23D9572EAFBE27EA13178F3620FB52` |
| `final.png` | 45,955 | `91E57921A15999A63F0C434FFD731EBD0E9EFDD5F38A3D6EA1643FC585C1A32E` |
| `story.json` | 3,420 | `FD9C285D50B774703164B277D3C3DAC24E1761004672A4273C8E6576BE5A8796` |
| `review.html` | 22,030 | `42F27F42957B2FB623DEA532BA4DEA42629B6FCC8CDD9E0A9A5965B7234F4C52` |

The unchanged PNG and manifest hashes show that adding replay motion did not
alter the canonical evidence or final raster scene.

Final workspace gates passed `cargo.exe fmt --all -- --check`,
`cargo.exe clippy --workspace --all-targets -- -D warnings`, and
`cargo.exe test --workspace --locked` with 107 tests passed and the two
intentional live/review tests ignored.

## Visual review

The integrated browser runtime failed twice during native Windows sandbox
initialization. The approved fallback used bundled Playwright with installed
Edge to capture the local review page at 500, 3,900, 6,600, 9,250, and 11,500
milliseconds. Those frames showed fixed node and edge geometry, initial blocked
states, the real resolved and claimed transitions, synchronized resolved/ready
changes, two-second final-state inspection, and the soft reset.

The first pass exposed an event caption overlapping a permanent label on nodes
whose label anchor is above the star. The corrected renderer places the event
caption on the opposite side. A forced reduced-motion capture showed only the
final state: solid resolved `#10` and `#20`, hollow frontier `#30`, hollow blocked
`#40`, and no CURRENT, READY, caption, or particle overlays.

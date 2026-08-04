# Static release preview validation

**Date:** 2026-08-02

**Issue:** #50 - Render a deterministic final constellation preview
**Environment:** Native Windows 11, `cargo.exe`, bundled Playwright with the
installed Microsoft Edge channel

## Contract exercised

`render_static_preview` accepts a canonical `ReleaseStory` and returns four
in-memory byte buffers without writing files:

- a script-free, self-contained SVG with a `1200 x 675` view box;
- a `1600 x 900` PNG rasterized from that exact final SVG scene;
- the canonical JSON story manifest;
- a self-contained local HTML review page containing the SVG and escaped
  manifest.

The fixture includes release issues, one external prerequisite, resolved and
unresolved dependency edges, solid and hollow completion shapes, every relevant
summary field, and an unsafe overlong title. Repeated renders are byte-identical.
The title is XML-escaped and truncated after forty Unicode grapheme clusters.

PNG text uses the bundled `Roboto-Regular.ttf` rather than the machine font
catalog. The font is pinned to Google Fonts commit
`376ff5c1ab3952ec7d324a2222f1018a09b2a437`, is accompanied by its Apache 2.0
license, and has SHA-256
`F9CED9AC76E56DA8CCA1048B21E0C7DA83740296A949E1997AE5DB04D7B18CEC`.

## Focused evidence

```powershell
cargo.exe test -p stellr-showcase --test static_preview
cargo.exe test -p stellr-showcase --test static_preview `
  write_local_static_preview_for_visual_review -- --ignored --exact
```

The focused suite passed two ordinary tests; the explicit ignored test wrote the
review-only outputs below `target/readme-showcase/issue-50-review/`.

| Review output | Bytes | Limit |
| --- | ---: | ---: |
| `final.svg` | 3,395 | 768,000 |
| `final.png` | 45,955 | 1,572,864 |
| `story.json` | 3,420 | 1,048,576 |
| `review.html` | 9,844 | Not a publication artifact |

An oversized manifest regression failed before any preview was returned.
Existing SVG safety tests continue to reject malformed XML, active elements,
event handlers, external references, CSS imports, escaped CSS, and oversized
SVG input.

The ordinary regression test also pins SHA-256 digests for all four outputs and
decodes the PNG to verify representative pixels for the dim external node,
solid resolved node, resolved edge, and hollow frontier and blocked nodes.

## Visual review

The integrated browser accepted the local file URL but queued the tab instead
of opening it in this task. The approved fallback used the bundled native
Playwright package with installed Edge:

```powershell
node.exe playwright\cli.js screenshot --browser chromium --channel msedge `
  --viewport-size "1440,1000" --full-page --wait-for-selector "g#release-heading" `
  --wait-for-timeout 1500 --timeout 30000 `
  file:///D:/tmp/stellr-readme-release-constellation-design/target/readme-showcase/issue-50-review/review.html `
  output/playwright/issue-50-review-2.png
```

The first capture exposed overlapping labels and oversized marker-scaled
arrowheads. The corrected capture showed alternating bounded labels with a
black readability halo, compact user-space arrowheads ending at star rims,
directed solid mint and dashed pale paths, a dim external prerequisite at the
required `0.35` prominence, and matching release title and summary. The final
PNG was inspected separately and contains the same final nodes, edges,
statuses, title, and summary.

# README release-constellation compatibility evidence

**Issue:** #47  
**Probe:** `docs/assets/readme-showcase/compatibility-probe.svg`  
**Static fallback:** `docs/assets/readme-showcase/compatibility-probe.png`

This probe is a compatibility fixture, not release history. Its fixed stars and
status transitions exist only to validate the delivery mechanisms that the real
release-story exporter will use.

## Local contract evidence

- Native Windows workspace baseline: 85 Rust tests and 182 web tests passed.
- SVG: 1200 x 675 view box, declarative twelve-second CSS loop, fixed node
  coordinates, title and description metadata, and reduced-motion final state.
- SVG safety: no script, `foreignObject`, transform animation, JavaScript URL,
  external `href`/`src`, remote CSS URL, or import.
- PNG: rasterized from the same SVG with reduced motion forced; 1600 x 900 and
  468,645 bytes.
- README: `<picture>` selects the PNG for reduced motion, the `<img>` fallback
  uses the SVG, and an ordinary Markdown link exposes the PNG independently of
  raw HTML support.

## GitHub-rendered acceptance

GitHub's rendered-README API was queried against
`codex/issues-47-54-readme-showcase`. It preserved the `<picture>`, reduced-motion
`<source>`, animated SVG `<img>`, alternative text, and ordinary Markdown PNG
link. This proves the server-side Markdown/sanitizer contract, but not browser
animation or media-query selection.

Automated inspection in the integrated ChatGPT browser could not start because
its Windows sandbox setup exited before opening a page. With explicit approval,
the runtime gates were instead exercised in native Playwright sessions using
Microsoft Edge and Firefox against the live branch README.

| Gate | Observation | Status |
| --- | --- | --- |
| Primary GitHub browser path | Edge and Firefox loaded the committed SVG from the live README. Screenshots three seconds apart had different SHA-256 hashes in each browser, and visual inspection showed the expected blocked-to-resolved status progression without moving node coordinates. | Pass |
| Reduced motion | Both browsers selected the 1600 x 900 PNG. Two screenshots taken three seconds apart were byte-identical in each browser. | Pass |
| Strict-Markdown PNG path | Both browsers followed the ordinary Markdown link to the repository PNG. | Pass |
| Narrow README width | At a 480 px viewport, the document remained 480 px wide and the image fit from x=33 to x=447 in both browsers. | Pass |
| Firefox degradation | Firefox rendered and animated the SVG; the PNG reduced-motion and explicit-link fallbacks also worked. | Pass |
| GitHub SVG safety | GitHub preserved the required image elements; the live asset executed only its declarative CSS animation and the committed SVG passes the fail-closed XML-aware validator. | Pass |

### Runtime evidence

- Edge animated frames:
  `481EBF4082EE4C6D7E6FF328D6A48FB863A4F27AF0FB3EDE9AA8F371B7C25070`
  and
  `C8B0C971471F75F3385C4F671A822CB78778E7EB1898340AB6E34C5B5F71F919`.
- Firefox animated frames:
  `4DFF47E4C903EA770BC76C82F18F5ED685635A11068741C6752CA9593494E841`
  and
  `284831CE1A60A15644D368ED15D7DDA91E2247B9EDC2014156401426A799FB0C`.
- Edge reduced-motion frames shared SHA-256
  `0C0C56C84EB59AEC547456F0FD89FA94166A94AC344161F5036D682BBC9B52AD`.
- Firefox reduced-motion frames shared SHA-256
  `5E29F19CE54404CDB2ED3B49971E777DA115E8068437E097557905F9D1A5462E`.

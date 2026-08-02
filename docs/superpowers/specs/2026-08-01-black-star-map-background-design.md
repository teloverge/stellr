# Black Star-Map Background Design

## Goal

Make Stellr's embedded star map render on a pure black field by default so the
existing celestial palette has the intended contrast in VS Code Simple Browser
and other supported browser surfaces.

## Design

- Change the default root theme from the current light palette to a dark palette.
- Set the shared `--background` token to pure black (`#000`).
- Keep the existing star, label, edge, glow, and animation palette unchanged.
- Use light foreground and supporting dark-theme tokens for future interface
  chrome, so the document and canvas remain visually consistent.
- Keep the renderer's existing token seam: `StarMap.svelte` reads
  `--background` and passes it to the canvas renderer. Do not add a canvas-only
  color override or theme toggle.

## Verification

- Add a frontend regression test that loads the application stylesheet and
  asserts the default background token is pure black with a light foreground.
- Run the frontend test, build, and Svelte checks.
- Rebuild and restart the embedded server, then verify the real browser surface
  has a black page and canvas while the graph remains legible.

## Scope

This change does not add theme selection, system-theme detection, persistence,
or a new star-map palette.

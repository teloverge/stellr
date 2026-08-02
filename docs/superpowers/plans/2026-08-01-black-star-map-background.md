# Black Star-Map Background Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Stellr's page and canvas render on a pure black field by default while preserving the existing celestial palette.

**Architecture:** Keep the existing theme-token seam: the document owns `--background`, `StarMap.svelte` reads the inherited token, and the canvas renderer fills with that value. Change the default document token set to the existing dark values with a pure-black background; do not add renderer-specific theme logic.

**Tech Stack:** Svelte 5, TypeScript 6, CSS custom properties, Vitest 4 with jsdom, Vite+

## Global Constraints

- Use native Windows PowerShell and native Windows executables only.
- Set the shared `--background` token to pure black (`#000`).
- Keep the existing star, label, edge, glow, and animation palette unchanged.
- Do not add theme selection, system-theme detection, persistence, or dependencies.
- Preserve all unrelated Issue #14 work and the dirty primary checkout.
- Do not commit, push, or mutate the GitHub tracker without separate user authorization.

---

### Task 1: Make the embedded star map dark by default

**Files:**
- Modify: `web/src/lib/StarMap.test.ts`
- Modify: `web/src/app.css`
- Modify: `CHANGELOG.md`

**Interfaces:**
- Consumes: inherited CSS custom property `--background` read by `StarMap.svelte` through `getComputedStyle(host)`
- Produces: `--background: #000` and light-on-dark document tokens for the default root theme

- [ ] **Step 1: Write the failing regression test**

Add the application stylesheet import and this test to `web/src/lib/StarMap.test.ts`:

```ts
import '../app.css'

it('uses the default black document background for the renderer', () => {
  const setBackground = vi.spyOn(Renderer.prototype, 'setBackground')
  const target = document.createElement('div')
  document.body.appendChild(target)

  const component = mount(StarMap, {
    target,
    props: { space: space(42) },
  })
  mounted.push(component)
  flushSync()

  expect(setBackground).toHaveBeenCalledWith('#000')
})
```

This catches the observed regression: restoring a light default token makes the real wrapper pass a light background to the canvas renderer.

- [ ] **Step 2: Run the focused test and verify RED**

Run from `web`:

```powershell
vp run test -- src/lib/StarMap.test.ts
```

Expected: FAIL because `setBackground` receives the current light `oklch(0.98 0.005 250)` token instead of `#000`.

- [ ] **Step 3: Make the default theme black**

Replace the separate `:root` and `.dark` token blocks in `web/src/app.css` with one dark-default block:

```css
:root,
.dark {
  --background: #000;
  --foreground: oklch(0.93 0.005 250);
  --muted: oklch(0.22 0.01 250);
  --muted-foreground: oklch(0.65 0.01 250);
  --border: oklch(0.28 0.01 250);
  --primary: oklch(0.7 0.15 250);
  --destructive: oklch(0.65 0.2 25);
}
```

Do not modify `web/src/lib/starmap/theme.ts` or the renderer drawing code.

- [ ] **Step 4: Verify GREEN and the full frontend**

Run from `web`:

```powershell
vp run test -- src/lib/StarMap.test.ts
vp run test
vp run check
vp run build
```

Expected: focused test passes; all frontend tests pass; Svelte reports zero errors and warnings; the production bundle builds.

- [ ] **Step 5: Record the visible change**

Append this bullet under `## Unreleased` in `CHANGELOG.md`:

```markdown
- Made the embedded star map use a pure-black default background so its stars,
  labels, and dependency paths retain their intended contrast in browser panes.
```

- [ ] **Step 6: Verify the embedded browser artifact**

Rebuild the native workspace and restart `stellr serve` on an available loopback port with a fresh per-run token. Confirm in a real Chromium iframe and VS Code Simple Browser that:

- the page and full canvas are pure black;
- the stars, labels, edges, and glows remain legible;
- the embedded JS and CSS assets load successfully;
- the API remains unauthorized without the session token.

- [ ] **Step 7: Leave the work ready for user review**

Run `git status --short` and `git diff --check`. Report the exact verification results and keep all changes uncommitted and unpushed.

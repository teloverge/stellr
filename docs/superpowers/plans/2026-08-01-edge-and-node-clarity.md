# Edge and Node Clarity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make dependency flow and issue completion unmistakable on Stellr's pure-black star map without changing graph layout or focus behavior.

**Architecture:** Keep all behavior inside the existing imperative canvas island. Issue #34 changes only the edge context constant and `#drawEdge`; Issue #35 changes only the issue-core block inside `#drawStar`. Each task owns a new focused canvas-recording test file so the two implementations can proceed concurrently without editing the same test file.

**Tech Stack:** Svelte 5, TypeScript 6, Canvas 2D, Vitest 4, Vite+

## Global Constraints

- Run only native Windows PowerShell and native Windows executables from `D:\tmp\stellr-issue-14`.
- Preserve every existing uncommitted Issue #14 change; do not commit, push, close issues, or modify tracker labels.
- Keep deterministic node positions, camera fit, pan/zoom, selection, session overlays, focus analysis, and the pure-black background unchanged.
- Keep CURRENT/READY path edges at full opacity and use 0.45 for unrelated edges.
- Motion remains exclusive to resolved edges and keeps its current speed.
- Resolved issue cores remain solid; every other visual state becomes hollow.
- Issue #14 retains its existing bright CURRENT double rings.
- Use TDD: write each behavior test first, run it red for the expected visual mismatch, then implement the smallest production change.
- Do not edit `web/package.json`; Vite+ may regenerate `devEngines`, which the controller will remove after verification.

---

### Task 1: Issue #34 — Stronger dependency direction and resolved flow

**Files:**
- Modify: `web/src/lib/starmap/starmap.ts` only at `CONTEXT_EDGE_ALPHA` and inside `#drawEdge`
- Create: `web/src/lib/starmap/edge-visual.test.ts`

**Interfaces:**
- Consumes: the existing `StarMap.setModel(tickets, sessions, currentIssue)` seam and blocker-to-dependent edge direction.
- Produces: the same renderer seam with stronger line, arrow, and resolved-motion paint; no new public production API.

- [ ] **Step 1: Write the failing canvas tests**

Create a real `StarMap` with a recording `CanvasRenderingContext2D` double that records each stroke's `lineWidth`, `strokeStyle`, dash pattern, line cap, and `globalAlpha`; each filled polygon's `fillStyle`; and each filled arc's radius, color, and alpha. Use a literal four-ticket fixture containing one resolved edge and one unresolved edge.

Add separate tests that prove these consumer-visible behaviors:

```ts
expect(resolvedStroke).toMatchObject({
  lineWidth: 3,
  strokeStyle: 'rgba(190,225,200,0.82)',
  dash: [],
  lineCap: 'round',
})

expect(unresolvedStroke).toMatchObject({
  lineWidth: 2.4,
  strokeStyle: 'rgba(174,192,218,0.62)',
  dash: [7, 7],
  lineCap: 'round',
})
```

Record the arrow polygon coordinates and hand-check that its blocker-to-dependent extent is 12 world pixels long with a 6.5 world-pixel half-width. Assert arrow colors exactly:

```ts
expect(resolvedArrow.fillStyle).toBe('#d9f3df')
expect(unresolvedArrow.fillStyle).toBe('#c8d5e8')
```

For the resolved edge, assert three 2.6-radius particle cores and three 5-radius halos. Assert no particle or halo radii on the unresolved edge. At a deterministic clock position, assert core alpha follows `0.45 + 0.5 * Math.sin(Math.PI * u)` and halo alpha follows `0.14 + 0.18 * Math.sin(Math.PI * u)` using literal expected values derived from the fixture's known `u` values.

Set the unresolved edge on the CURRENT/READY path and leave the resolved edge unrelated. Assert recorded `globalAlpha` is 1 for the path treatment and 0.45 for the unrelated line, arrow, particle cores, and halos.

- [ ] **Step 2: Run the focused test and verify RED**

Run from `web`:

```powershell
vp exec vitest run src/lib/starmap/edge-visual.test.ts
```

Expected: assertions fail because widths remain 1.8/1.3, dashes remain `[4, 6]`, arrows remain 7 by 3.8, motion remains two 1.7-radius particles without halos, and unrelated edge alpha remains 0.2.

- [ ] **Step 3: Implement the approved edge grammar**

Change `CONTEXT_EDGE_ALPHA` to `0.45`. Inside `#drawEdge`:

```ts
if (e.satisfied) {
  g.strokeStyle = 'rgba(190,225,200,0.82)'
  g.lineWidth = 3
  g.setLineDash([])
} else {
  g.strokeStyle = 'rgba(174,192,218,0.62)'
  g.lineWidth = 2.4
  g.setLineDash([7, 7])
}
g.lineCap = 'round'
```

Keep the current `u` and quadratic-curve position formulas and speed. For each of three particles, paint the 5-radius halo first and the 2.6-radius core second, using:

```ts
const wave = Math.sin(u * Math.PI)
g.fillStyle = `rgba(190,225,200,${0.14 + 0.18 * wave})`
g.arc(fx, fy, 5, 0, TAU)
g.fill()
g.fillStyle = `rgba(220,255,230,${0.45 + 0.5 * wave})`
g.arc(fx, fy, 2.6, 0, TAU)
g.fill()
```

Use `k < 3`, `k / 3`, arrow length `ah = 12`, half-width `aw = 6.5`, and arrow colors `#d9f3df` / `#c8d5e8`. Do not add motion to unresolved edges.

- [ ] **Step 4: Run focused and existing renderer tests**

```powershell
vp exec vitest run src/lib/starmap/edge-visual.test.ts
vp exec vitest run src/lib/starmap/starmap.test.ts
```

Expected: both commands exit 0 with no warnings.

- [ ] **Step 5: Self-review without committing**

Confirm only the owned constant, `#drawEdge`, and `edge-visual.test.ts` changed. Report the red and green commands with counts; do not commit.

---

### Task 2: Issue #35 — Solid completed and hollow incomplete issue cores

**Files:**
- Modify: `web/src/lib/starmap/starmap.ts` only inside the core-paint block of `#drawStar`
- Create: `web/src/lib/starmap/core-visual.test.ts`

**Interfaces:**
- Consumes: existing `VisualState`, `STAR` palette, scaled core radius, and CURRENT ring behavior.
- Produces: solid `resolved` cores and hollow `frontier`, `blocked`, `claimed`, and `out_of_scope` cores through the unchanged renderer seam.

- [ ] **Step 1: Write failing core-shape tests**

Create a real `StarMap` with a recording canvas context. Use one literal ticket per visual state and record radial-gradient fills, black disk fills, colored strokes, line widths, and arc radii.

Assert resolved paints the existing radial-gradient core and does not paint a black core disk. For each non-resolved state, assert:

```ts
expect(core.blackDisk).toMatchObject({ radius: scaledRadius, fillStyle: '#000' })
expect(core.rim.strokeStyle).toBe(expectedStatusCoreColor)
expect(core.rim.outerRadius).toBeCloseTo(scaledRadius)
```

The rim uses `lineWidth = Math.max(2.2, scaledRadius * 0.32)` and is drawn at `scaledRadius - lineWidth / 2`, so its outer boundary remains exactly the existing scaled radius. Assert the existing glow is still painted for every state.

Set an incomplete current issue and assert two additional white strokes remain at scaled radius plus 8 and plus 13, with the existing opacity and line widths.

- [ ] **Step 2: Run the focused test and verify RED**

```powershell
vp exec vitest run src/lib/starmap/core-visual.test.ts
```

Expected: assertions fail because every state currently paints the same solid radial-gradient core and no hollow black disk/rim exists.

- [ ] **Step 3: Implement the approved completion grammar**

Keep the existing glow block unchanged. Replace only the core block with a state branch:

```ts
if (n.vstate === 'resolved') {
  const cg = g.createRadialGradient(x, y, 0, x, y, cr * 1.35)
  cg.addColorStop(0, hexA(c.core, 1))
  cg.addColorStop(0.6, hexA(c.core, 0.92))
  cg.addColorStop(0.82, hexA(c.core, 0.45))
  cg.addColorStop(1, hexA(c.core, 0))
  g.fillStyle = cg
  g.beginPath()
  g.arc(x, y, cr * 1.35, 0, TAU)
  g.fill()
} else {
  g.fillStyle = '#000'
  g.beginPath()
  g.arc(x, y, cr, 0, TAU)
  g.fill()
  const rimWidth = Math.max(2.2, cr * 0.32)
  g.strokeStyle = hexA(c.core, 0.95)
  g.lineWidth = rimWidth
  g.beginPath()
  g.arc(x, y, cr - rimWidth / 2, 0, TAU)
  g.stroke()
}
```

Do not change flare, claimed rings, session overlays, CURRENT rings, selection rings, hit testing, labels, or radius calculation.

- [ ] **Step 4: Run focused and existing renderer tests**

```powershell
vp exec vitest run src/lib/starmap/core-visual.test.ts
vp exec vitest run src/lib/starmap/starmap.test.ts
```

Expected: both commands exit 0 with no warnings.

- [ ] **Step 5: Self-review without committing**

Confirm only the `#drawStar` core block and `core-visual.test.ts` changed. Report the red and green commands with counts; do not commit.

---

### Task 3: Integrate, review, and verify the combined slice

**Files:**
- Modify if needed: `web/src/lib/starmap/starmap.ts`
- Modify: `CHANGELOG.md`
- Verify: `web/src/lib/starmap/edge-visual.test.ts`
- Verify: `web/src/lib/starmap/core-visual.test.ts`

**Interfaces:**
- Consumes: Tasks 1 and 2 in the shared Issue #14 worktree.
- Produces: one coherent embedded star-map artifact satisfying Issues #34 and #35.

- [ ] **Step 1: Review parallel edits for overlap**

Inspect `git diff` and confirm Task 1 changed only edge-owned regions while Task 2 changed only core-owned regions. Resolve no behavior by guesswork; any overlap must be reconciled against the approved spec.

- [ ] **Step 2: Run the complete frontend gate**

```powershell
vp run test
vp run check
vp run build
```

Expected: every Vitest file passes, Svelte reports 0 errors and 0 warnings, and Vite emits the production bundle.

- [ ] **Step 3: Remove generated Vite+ metadata**

If `web/package.json` contains only a generated `devEngines.packageManager` block, remove that exact block and verify `git diff -- web/package.json` is empty.

- [ ] **Step 4: Update Unreleased notes**

Add the newest Unreleased bullet:

```markdown
- Strengthened dependency lines, arrows, and resolved-edge motion, and made
  incomplete issue cores hollow while completed issues remain solid.
```

- [ ] **Step 5: Run the native workspace gate**

Stop only the verified `D:\tmp\stellr-issue-14\target\debug\stellr.exe` review process, then run:

```powershell
cargo.exe fmt --all -- --check
cargo.exe test --workspace --locked
cargo.exe clippy --workspace --all-targets --locked -- -D warnings
cargo.exe build --workspace --locked
```

Expected: every command exits 0.

- [ ] **Step 6: Rebuild and visually verify the real page**

Restart `stellr.exe serve --addr 127.0.0.1:39420 --issue 14`, verify root/assets/tokened API return 200 and bare API returns 401, then capture a headed-browser screenshot. Confirm thicker readable context edges, dominant CURRENT/READY path, larger arrows, brighter resolved motion, hollow incomplete cores, solid completed cores, and unchanged #14 CURRENT rings.

- [ ] **Step 7: Final scope audit**

Run `git diff --check`, `git status --short`, and `git diff --stat`. Keep every change uncommitted and unpushed. Do not close Issues #34 or #35 until the user separately authorizes tracker completion.

# Selected Node Edge Emphasis Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Emphasize every incoming and outgoing edge directly incident to the selected node so its immediate graph relationships are easy to trace.

**Architecture:** Keep selection as transient state inside the existing imperative canvas renderer. Paint non-selected edges through the existing CURRENT/READY focus path, then paint selected incident edges once in a final edge pass with exact width and arrow scaling while retaining semantic colors, dashes, direction, and motion.

**Tech Stack:** TypeScript 6, Vitest 4, Svelte 5, HTML Canvas 2D, native Windows PowerShell, Rust/Cargo workspace validation.

## Global Constraints

- Run only native Windows executables and PowerShell paths; do not use WSL or Linux toolchains.
- An edge is selected exactly when `selected !== null && (edge.from === selected || edge.to === selected)`.
- Selected edges set canvas `globalAlpha` to `1` while retaining the built-in alpha of semantic stroke and fill colors.
- Selected edge stroke width is exactly `1.7` times its ordinary width.
- Selected arrowhead length and half-width are exactly `1.25` times their ordinary dimensions.
- Preserve semantic colors, solid/dashed patterns, arrow direction, motion eligibility, particle count, particle speed, and particle size.
- Render selected incident edges once after all non-selected edges; do not double-paint them.
- Do not change graph topology, node positions, camera behavior, neighboring nodes, labels, hover behavior, persistence, model/adapter contracts, or transitive path analysis.

---

## File Structure

- `web/src/lib/starmap/starmap.ts`: owns the selected-edge predicate, two-pass edge ordering, and selected stroke/arrow scaling within the existing canvas island.
- `web/src/lib/starmap/edge-visual.test.ts`: records canvas strokes and fills to pin direct-edge scope, focus interaction, dependency/workflow styles, arrow dimensions, deselection, and isolated-node behavior.

### Task 1: Render selected direct connections above ordinary edges

**Files:**
- Modify: `web/src/lib/starmap/edge-visual.test.ts:64-270`
- Modify: `web/src/lib/starmap/starmap.ts:39-48`
- Modify: `web/src/lib/starmap/starmap.ts:887-1019`

**Interfaces:**
- Consumes: existing `StarMap.select(num: number | null): void`, `RenderEdge.from`, `RenderEdge.to`, `#focus.pathEdges`, and `#drawEdge(...)` semantic painting.
- Produces: private `#isSelectedEdge(edge: RenderEdge): boolean` and private `#drawEdge(g: CanvasRenderingContext2D, edge: RenderEdge, selected?: boolean): void` behavior; no public API changes.

- [ ] **Step 1: Extend the canvas harness and write failing selection tests**

Change the `paint` helper so a test can apply a selection transition before the recorded frame:

```ts
function paint(
  tickets: Ticket[],
  currentIssue: number | null = null,
  selections: readonly (number | null)[] = [],
): { strokes: Stroke[]; fills: Fill[] } {
  const { ctx, strokes, fills } = recordingContext()
  HTMLCanvasElement.prototype.getContext = (() => ctx) as never
  frames = []
  const host = document.createElement('div')
  Object.defineProperties(host, {
    clientWidth: { value: 1000 },
    clientHeight: { value: 700 },
  })
  document.body.appendChild(host)
  const map = new StarMap()
  map.mount(host)
  map.setModel(tickets, {}, currentIssue)
  for (const selection of selections) map.select(selection)
  frames.shift()!(1_000)
  map.destroy()
  return { strokes, fills }
}
```

Add a fixture with one selected incoming edge, one selected outgoing edge, one unrelated edge, and one isolated node:

```ts
const SELECTION_FIXTURE: Ticket[] = [
  { num: 1, slug: '1', title: 'resolved source', type: 'task', status: 'resolved', blockedBy: [], parentIssue: null, frontier: false },
  { num: 2, slug: '2', title: 'selected middle', type: 'task', status: 'open', blockedBy: [1], parentIssue: null, frontier: false },
  { num: 3, slug: '3', title: 'blocked destination', type: 'task', status: 'open', blockedBy: [2], parentIssue: null, frontier: false },
  { num: 4, slug: '4', title: 'unrelated source', type: 'task', status: 'open', blockedBy: [], parentIssue: null, frontier: true },
  { num: 5, slug: '5', title: 'unrelated current', type: 'task', status: 'open', blockedBy: [4], parentIssue: null, frontier: false },
  { num: 6, slug: '6', title: 'isolated', type: 'task', status: 'open', blockedBy: [], parentIssue: null, frontier: true },
]

function edgeStrokes(render: { strokes: Stroke[] }): Stroke[] {
  return render.strokes.filter((stroke) =>
    ['rgba(190,225,200,0.82)', 'rgba(174,192,218,0.62)', 'rgba(170,145,255,0.78)'].includes(stroke.color),
  )
}

function arrowDimensions(fill: Fill): { length: number; halfWidth: number } {
  const [tip, baseA, baseB] = fill.points
  const base = { x: (baseA.x + baseB.x) / 2, y: (baseA.y + baseB.y) / 2 }
  return {
    length: Math.hypot(tip.x - base.x, tip.y - base.y),
    halfWidth: Math.hypot(baseA.x - base.x, baseA.y - base.y),
  }
}

function expectArrowDimensions(fill: Fill, length: number, halfWidth: number): void {
  const dimensions = arrowDimensions(fill)
  expect(dimensions.length).toBeCloseTo(length)
  expect(dimensions.halfWidth).toBeCloseTo(halfWidth)
}
```

Add one state-transition test. With issue `5` as CURRENT, every dependency edge is ordinary context; selecting `2` must override context only for `1 -> 2` and `2 -> 3`. Then deselect and select isolated `6` to prove the ordinary styles return and unrelated edges never change:

```ts
it('scopes selection emphasis to direct dependency edges and restores ordinary treatment', () => {
  const selected = paint(SELECTION_FIXTURE, 5, [2])
  const selectedEdges = edgeStrokes(selected)
  expect(selectedEdges).toHaveLength(3)
  expect(selectedEdges.find((stroke) => stroke.color === 'rgba(190,225,200,0.82)')).toMatchObject({ width: 5.1, dash: [], alpha: 1 })
  const unresolvedEdges = selectedEdges.filter((stroke) => stroke.color === 'rgba(174,192,218,0.62)')
  expect(unresolvedEdges.find((stroke) => stroke.width === 4.08)).toMatchObject({ dash: [7, 7], alpha: 1 })
  expect(unresolvedEdges.find((stroke) => stroke.width === 2.4)).toMatchObject({ dash: [7, 7], alpha: 0.45 })

  const resolvedArrow = selected.fills.find((fill) => fill.color === '#d9f3df')!
  const unresolvedArrows = selected.fills.filter((fill) => fill.color === '#c8d5e8')
  expect(resolvedArrow.alpha).toBe(1)
  expectArrowDimensions(resolvedArrow, 15, 8.125)
  const selectedUnresolvedArrow = unresolvedArrows.find((fill) => fill.alpha === 1)!
  const contextUnresolvedArrow = unresolvedArrows.find((fill) => fill.alpha === 0.45)!
  expectArrowDimensions(selectedUnresolvedArrow, 15, 8.125)
  expectArrowDimensions(contextUnresolvedArrow, 12, 6.5)
  expectParticleMotion(selected, 1)

  const deselected = edgeStrokes(paint(SELECTION_FIXTURE, 5, [2, null]))
  expect(deselected.map(({ width, alpha }) => ({ width, alpha }))).toEqual([
    { width: 3, alpha: 0.45 },
    { width: 2.4, alpha: 0.45 },
    { width: 2.4, alpha: 0.45 },
  ])
  const isolated = edgeStrokes(paint(SELECTION_FIXTURE, 5, [6]))
  expect(isolated.map(({ width, alpha }) => ({ width, alpha }))).toEqual(
    deselected.map(({ width, alpha }) => ({ width, alpha })),
  )
})
```

Extend the existing lone-child workflow test so selecting child `37` proves that both entry and return edges receive the exact mini-edge treatment:

```ts
const selectedChild = paint(
  [
    { num: 16, slug: '16', title: 'parent', type: 'issue', status: 'open', blockedBy: [], parentIssue: null, frontier: false },
    { num: 37, slug: '37', title: 'child', type: 'task', status: 'open', blockedBy: [], parentIssue: 16, frontier: false },
  ],
  null,
  [37],
)
const selectedMiniStrokes = edgeStrokes(selectedChild)
expect(selectedMiniStrokes).toHaveLength(2)
for (const stroke of selectedMiniStrokes) {
  expect(stroke).toMatchObject({ color: 'rgba(170,145,255,0.78)', width: 4.42, dash: [8, 7], alpha: 1 })
}
for (const arrow of selectedChild.fills.filter((fill) => fill.color === '#c7b8ff')) {
  expectArrowDimensions(arrow, 15, 8.125)
}
```

- [ ] **Step 2: Run the targeted test and verify the new assertions fail for the missing emphasis**

Run:

```powershell
C:\Users\pfdev\.vite-plus\bin\npm.exe --prefix web test -- src/lib/starmap/edge-visual.test.ts
```

Expected: FAIL because incident dependency widths remain `3`/`2.4` instead of `5.1`/`4.08`, context alpha remains `0.45` instead of `1`, mini-edge width remains `2.6` instead of `4.42`, and arrow dimensions remain `12` by `6.5` instead of `15` by `8.125`.

- [ ] **Step 3: Add the minimal two-pass selected-edge implementation**

Add exact renderer constants beside the existing context constants:

```ts
const SELECTED_EDGE_WIDTH_SCALE = 1.7
const SELECTED_EDGE_ARROW_SCALE = 1.25
```

Add a private predicate near the draw methods:

```ts
#isSelectedEdge(edge: RenderEdge): boolean {
  return (
    this.#selected !== null &&
    (edge.from === this.#selected || edge.to === this.#selected)
  )
}
```

Replace the single edge loop in `#draw()` with two passes. The first preserves ordinary focus/context behavior for non-selected edges; the second paints selected incident edges once, last, at renderer alpha `1`:

```ts
for (const edge of this.#edges) {
  if (this.#isSelectedEdge(edge)) continue
  g.save()
  if (focused && !this.#focus.pathEdges.has(edgeKey(edge.from, edge.to))) {
    g.globalAlpha = CONTEXT_EDGE_ALPHA
  }
  this.#drawEdge(g, edge)
  g.restore()
}
for (const edge of this.#edges) {
  if (!this.#isSelectedEdge(edge)) continue
  g.save()
  g.globalAlpha = 1
  this.#drawEdge(g, edge, true)
  g.restore()
}
```

Give `#drawEdge` a defaulted selected flag and apply the exact scales only to stroke width and arrow dimensions:

```ts
#drawEdge(g: CanvasRenderingContext2D, edge: RenderEdge, selected = false): void {
  // Keep the existing geometry and semantic-style derivation.
  const strokeScale = selected ? SELECTED_EDGE_WIDTH_SCALE : 1
  // Existing resolved branch:
  g.lineWidth = 3 * strokeScale
  // Existing incomplete mini-edge branch:
  g.lineWidth = 2.6 * strokeScale
  // Existing unresolved dependency branch:
  g.lineWidth = 2.4 * strokeScale

  // Keep all existing particle code unchanged.
  const arrowScale = selected ? SELECTED_EDGE_ARROW_SCALE : 1
  const ah = 12 * arrowScale,
    aw = 6.5 * arrowScale
  // Keep the existing arrow geometry and semantic fill selection unchanged.
}
```

Do not scale dash arrays or particle dimensions, and do not add a selected color or glow.

- [ ] **Step 4: Run the targeted test and verify the complete edge-visual file passes**

Run:

```powershell
C:\Users\pfdev\.vite-plus\bin\npm.exe --prefix web test -- src/lib/starmap/edge-visual.test.ts
```

Expected: the dependency-edge visual test file passes with no failures or warnings.

- [ ] **Step 5: Run the complete frontend validation**

Run each command from `D:\tmp\stellr-selected-node-edge-emphasis`:

```powershell
C:\Users\pfdev\.vite-plus\bin\npm.exe --prefix web test
C:\Users\pfdev\.vite-plus\bin\npm.exe --prefix web run check
C:\Users\pfdev\.vite-plus\bin\npm.exe --prefix web run build
```

Expected: all Vitest files pass, Svelte reports `0 errors and 0 warnings`, and Vite completes a production build.

- [ ] **Step 6: Run native workspace quality gates**

Run:

```powershell
cargo.exe fmt --all -- --check
cargo.exe clippy --workspace --all-targets --locked -- -D warnings
cargo.exe test --workspace --locked -- --test-threads=1
```

Expected: formatting and Clippy exit `0`; all non-ignored Rust workspace tests pass. Keep the frontend `web/dist` produced in Step 5 available for the embedded-server compile.

- [ ] **Step 7: Review the scoped diff and commit the implementation**

Run:

```powershell
git diff --check
git status --short
git diff -- web/src/lib/starmap/starmap.ts web/src/lib/starmap/edge-visual.test.ts
git add -- web/src/lib/starmap/starmap.ts web/src/lib/starmap/edge-visual.test.ts
git commit -m "feat: emphasize selected node connections"
```

Expected: only the renderer and its edge-visual tests are added to the implementation commit; generated permission-file line-ending rewrites are not included.

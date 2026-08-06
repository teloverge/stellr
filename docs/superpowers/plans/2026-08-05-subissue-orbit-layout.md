# Adaptive Subissue Orbit Layout Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace compact subissue arcs with deterministic adaptive concentric orbits, outward labels, and easier nearest-node selection.

**Architecture:** Keep `computeLayout` as the broad deterministic layout seam, then place valid parent groups parent-first through `placeDirectChildClusters`. Introduce one pure label-geometry module shared by layout scoring and canvas rendering, and keep pointer behavior observable through `StarMap.selectAtScreen`.

**Tech Stack:** TypeScript 6, Vitest 4, Canvas 2D, Svelte 5, native Windows PowerShell/npm workflow.

## Global Constraints

- Preserve broad parent anchors and status-only/temporal coordinate stability.
- Use full parent-centered circular orbits and add concentric rings only as density requires.
- Keep visible star sizes, relationship semantics, focus behavior, and camera behavior unchanged.
- Use deterministic finite fallbacks for invalid or fully obstructed inputs.
- Keep top-level label and hit-target behavior unchanged.

---

### Task 1: Shared outward label geometry

**Files:**
- Create: `web/src/lib/starmap/label-geometry.ts`
- Create: `web/src/lib/starmap/label-geometry.test.ts`
- Modify: `web/src/lib/starmap/starmap.ts`

**Interfaces:**
- Produces: `outwardLabelGeometry(input: OutwardLabelInput): LabelGeometry`
- Produces: `estimateLabelWidth(number: number, title: string, fontSize?: number): number`
- Produces: `clipTitle(title: string, budget: number): string`
- `LabelGeometry` contains `x`, `y`, `align`, and a `box` with `x0`, `y0`, `x1`, `y1`.

- [x] **Step 1: Write failing geometry tests**

Add literal worked examples proving right children produce left-aligned labels,
left children produce right-aligned labels, vertical children stay centered,
and every returned box lies farther from the parent than the child center.

- [x] **Step 2: Run the focused test and verify red**

Run: `npm.exe --prefix web test -- src/lib/starmap/label-geometry.test.ts`

Expected: FAIL because `label-geometry.ts` does not exist.

- [x] **Step 3: Implement the pure geometry seam**

Use a normalized parent-to-child vector, a fixed radial gap, alignment selected
from the vector, and one shared box calculation:

```ts
export function outwardLabelGeometry(input: OutwardLabelInput): LabelGeometry
export function estimateLabelWidth(number: number, title: string, fontSize = 14): number
export function clipTitle(title: string, budget: number): string
```

- [x] **Step 4: Run the focused test and typecheck**

Run: `npm.exe --prefix web test -- src/lib/starmap/label-geometry.test.ts`

Run: `npm.exe --prefix web run check`

Expected: PASS.

---

### Task 2: Adaptive concentric orbit placement

**Files:**
- Modify: `web/src/lib/starmap/layout.ts`
- Modify: `web/src/lib/starmap/cluster-layout.ts`
- Modify: `web/src/lib/starmap/layout.test.ts`
- Modify: `web/src/lib/starmap/cluster-layout.test.ts`

**Interfaces:**
- Extends: `LayoutNode` with optional `title?: string` for deterministic label footprint scoring.
- Preserves: `placeDirectChildClusters(nodes, broadPoints, dependencyEdges): Record<number, Point>`.
- Consumes: shared label geometry and width estimation from Task 1.

- [x] **Step 1: Replace compact-arc expectations with a failing full-orbit test**

Pin four direct children at equal angular intervals on one ring, unchanged broad
parent/unrelated anchors, and snapshot-order independence.

- [x] **Step 2: Run the focused layout tests and verify red**

Run: `npm.exe --prefix web test -- src/lib/starmap/layout.test.ts src/lib/starmap/cluster-layout.test.ts`

Expected: FAIL because current points occupy only compact arcs.

- [x] **Step 3: Implement one-ring deterministic placement**

Replace compact arc slots with complete-ring slots and bounded deterministic
rotation candidates. Keep sibling dependency order and hierarchy-depth order.

- [x] **Step 4: Run focused tests and verify green**

Run the Task 2 focused command. Expected: PASS for one-ring behavior.

- [x] **Step 5: Add a failing dense-group multi-ring test**

Use fourteen children with long titles. Assert at least two distinct radii,
minimum star clearance, non-overlapping shared label boxes, finite points, and
stable output when input order reverses.

- [x] **Step 6: Implement adaptive ring allocation and scoring**

Allocate ring capacity from circumference and occupied footprint, evaluate
bounded expansion levels and rotations, and score star/label/node/line
clearance with line crossings carrying the strongest penalty. Keep invalid
hierarchies at broad coordinates.

- [x] **Step 7: Run focused tests and typecheck**

Run the Task 2 focused command and `npm.exe --prefix web run check`.

Expected: PASS.

---

### Task 3: Outward rendering and nearest subissue hit targets

**Files:**
- Modify: `web/src/lib/starmap/starmap.ts`
- Modify: `web/src/lib/starmap/starmap.test.ts`

**Interfaces:**
- Preserves: `StarMap.selectAtScreen(sx, sy): number | null`.
- Consumes: `outwardLabelGeometry` for valid subissues.
- Top-level nodes continue through the existing above/below label solver and hit radius.

- [x] **Step 1: Add failing renderer tests**

Through the mounted `StarMap` seam, assert labels for right/left/vertical
subissues use outward alignment, while top-level labels remain centered.

- [x] **Step 2: Run the focused renderer test and verify red**

Run: `npm.exe --prefix web test -- src/lib/starmap/starmap.test.ts`

Expected: FAIL because every label is currently centered above or below.

- [x] **Step 3: Render subissue labels from shared geometry**

Try bounded radial distances for subissues, collision-check the shared boxes,
store per-label text alignment, and leave the top-level solver unchanged.

- [x] **Step 4: Add failing interaction tests**

Assert a click outside a subissue's visible/top-level target still selects the
subissue, the same offset misses a top-level issue, and overlapping targets
select the nearest center with issue number as the exact-distance tie-breaker.

- [x] **Step 5: Implement nearest-node hit testing**

Use a larger minimum screen-space radius only for valid subissues, gather all
eligible nodes, and select by distance then issue number instead of array order.

- [x] **Step 6: Run renderer tests and typecheck**

Run: `npm.exe --prefix web test -- src/lib/starmap/starmap.test.ts`

Run: `npm.exe --prefix web run check`

Expected: PASS.

---

### Task 4: Full validation and review

**Files:**
- Modify only if a validation or review finding requires a scoped correction.

**Interfaces:**
- Verifies the complete frontend and repository behavior; produces no new API.

- [x] **Step 1: Run frontend validation**

Run: `npm.exe --prefix web test`

Run: `npm.exe --prefix web run check`

Run: `npm.exe --prefix web run build`

- [x] **Step 2: Run repository validation**

Run: `cargo.exe test --workspace --locked`

Run: `cargo.exe fmt --all -- --check`

Run: `cargo.exe clippy --workspace --all-targets --locked -- -D warnings`

Run: `git diff --check`

- [ ] **Step 3: Validate the dense graph visually on native Windows**

Launch the local app/preview, inspect the Encrydle-shaped graph at normal and
reduced viewport sizes, and exercise selection of neighboring subissues.

Blocked evidence: the native server loaded the Encrydle route in the shared
preview, but repeated preview snapshot and evaluation calls failed, so no
screenshot-based visual claim is recorded.

- [x] **Step 4: Run two-axis code review from design commit `7204c6d`**

Review `git diff 7204c6d...HEAD` against repository standards and
`docs/superpowers/specs/2026-08-05-subissue-orbit-layout-design.md`; correct
confirmed findings and rerun affected gates.

- [x] **Step 5: Commit the completed implementation**

Stage only the plan, implementation, and tests, then commit on the current
branch with a focused message.

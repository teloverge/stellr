# Node Visual Hierarchy and 2.5D Rendering Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make in-progress work visually dominant, reduce ring and arrow clutter, and render stable 2.5D sphere nodes without changing graph topology or direct-only selection behavior.

**Architecture:** Derive a narrow `WorkPriority` at the renderer adapter boundary, then let the Canvas 2D island use it for palette, metrics, rings, and label order. Keep depth entirely in paint layers and preserve PR #87's selected-last incident-edge pass while shrinking all arrowheads.

**Tech Stack:** Svelte 5, TypeScript 6, Canvas 2D, Vitest 4, Rust/Tauri, native Windows PowerShell.

## Global Constraints

- Use native Windows executables only; do not use WSL or Linux tooling.
- Work only in `D:\tmp\stellr-selected-node-edge-emphasis` on `codex/node-visual-hierarchy-2-5d`; preserve the dirty primary checkout.
- Terminal issue states override stale workflow labels.
- Open priority is `in-progress`, assigned fallback, `ready-for-agent`, ordinary frontier, then blocked.
- The canonical ready label is exactly `ready-for-agent`; do not add a `ready` alias.
- Keep deterministic 2D coordinates, camera, historical topology, URLs, persistence, and server endpoints unchanged.
- Do not add WebGL, Three.js, Z coordinates, orbit controls, auto-rotation, or force simulation.
- Show at most one status ring plus one selection ring.
- Blocked, completed, out-of-scope, and ordinary frontier nodes have no outer status ring.
- All arrowheads use length `8` and half-width `4`; selected strokes retain the `1.7` multiplier.
- Selected emphasis stays limited to direct incoming and outgoing incident edges.
- Do not automatically relabel existing issues.

---

### Task 1: Derive Workflow Priority at the Adapter Boundary

**Files:**
- Create: `web/src/lib/starmap/work-priority.ts`
- Create: `web/src/lib/starmap/work-priority.test.ts`
- Modify: `web/src/lib/starmap/model.ts`
- Modify: `web/src/lib/starmap/adapt.ts`
- Modify: `web/src/lib/starmap/adapt.test.ts`

**Interfaces:**
- Consumes: `Status`, issue labels, and assignees from `SpaceModel`.
- Produces: `WorkPriority = 'in_progress' | 'ready' | 'frontier' | 'blocked' | 'terminal'`.
- Produces: `deriveWorkPriority(input: WorkPriorityInput): WorkPriority`.
- Produces: `ticketWorkPriority(ticket): WorkPriority` for existing renderer fixtures.
- Produces: optional `Ticket.workPriority?: WorkPriority`; production adapter output always sets it.

- [ ] **Step 1: Write the failing priority table**

Create `work-priority.test.ts`:

```ts
import { describe, expect, it } from 'vitest'
import { deriveWorkPriority } from './work-priority'

describe('deriveWorkPriority', () => {
  it.each([
    ['resolved overrides stale labels', 'resolved', ['in-progress'], ['ada'], 'terminal'],
    ['out of scope overrides stale labels', 'out_of_scope', ['ready-for-agent'], [], 'terminal'],
    ['literal in progress outranks a blocker', 'blocked', ['IN-PROGRESS'], [], 'in_progress'],
    ['claimed is the assigned fallback', 'claimed', [], ['ada'], 'in_progress'],
    ['an assignee is the assigned fallback', 'frontier', [], ['ada'], 'in_progress'],
    ['ready requires an unblocked frontier', 'frontier', ['READY-FOR-AGENT'], [], 'ready'],
    ['ready cannot override blocked', 'blocked', ['ready-for-agent'], [], 'blocked'],
    ['unlabelled unblocked work stays frontier', 'frontier', [], [], 'frontier'],
  ] as const)('%s', (_name, status, labels, assignees, expected) => {
    expect(deriveWorkPriority({ status, labels, assignees })).toBe(expected)
  })
})
```

- [ ] **Step 2: Run red**

```powershell
C:\Users\pfdev\.vite-plus\bin\npm.exe --prefix web test -- src/lib/starmap/work-priority.test.ts
```

Expected: FAIL because `work-priority.ts` does not exist.

- [ ] **Step 3: Implement the pure priority contract**

Create `work-priority.ts`:

```ts
import type { Status } from '../model'
import type { Ticket } from './model'

export type WorkPriority = 'in_progress' | 'ready' | 'frontier' | 'blocked' | 'terminal'

export interface WorkPriorityInput {
  status: Status
  labels: readonly string[]
  assignees: readonly string[]
}

const hasLabel = (labels: readonly string[], expected: string): boolean =>
  labels.some((label) => label.toLowerCase() === expected)

export function deriveWorkPriority(input: WorkPriorityInput): WorkPriority {
  if (input.status === 'resolved' || input.status === 'out_of_scope') return 'terminal'
  if (hasLabel(input.labels, 'in-progress')) return 'in_progress'
  if (input.status === 'claimed' || input.assignees.length > 0) return 'in_progress'
  if (input.status === 'frontier' && hasLabel(input.labels, 'ready-for-agent')) return 'ready'
  return input.status === 'blocked' ? 'blocked' : 'frontier'
}

export function ticketWorkPriority(
  ticket: Pick<Ticket, 'status' | 'frontier' | 'readyForAgent' | 'workPriority'>,
): WorkPriority {
  if (ticket.workPriority) return ticket.workPriority
  if (ticket.status === 'resolved' || ticket.status === 'out_of_scope') return 'terminal'
  if (ticket.status === 'claimed') return 'in_progress'
  if (ticket.readyForAgent) return 'ready'
  return ticket.frontier ? 'frontier' : 'blocked'
}
```

Add a type-only `WorkPriority` import and `workPriority?: WorkPriority` to `Ticket`. In `toRendererModel`, derive priority once, assign it, and set `readyForAgent` only when priority is `ready`.

- [ ] **Step 4: Extend adapter assertions**

Make `adapt.test.ts` prove literal in-progress, assigned fallback, terminal override, truthful ready, and blocked-with-ready-label behavior:

```ts
expect(model.map((ticket) => ticket.workPriority)).toEqual([
  'in_progress', 'terminal', 'in_progress', 'terminal', 'blocked',
])
expect(model.map((ticket) => ticket.readyForAgent)).toEqual([
  false, false, false, false, false,
])
```

Add a separate unblocked `ready-for-agent` fixture and expect `workPriority: 'ready'` and `readyForAgent: true`.

- [ ] **Step 5: Run green**

```powershell
C:\Users\pfdev\.vite-plus\bin\npm.exe --prefix web test -- src/lib/starmap/work-priority.test.ts src/lib/starmap/adapt.test.ts
```

Expected: both files pass.

- [ ] **Step 6: Commit**

```powershell
git add -- web/src/lib/starmap/work-priority.ts web/src/lib/starmap/work-priority.test.ts web/src/lib/starmap/model.ts web/src/lib/starmap/adapt.ts web/src/lib/starmap/adapt.test.ts
git commit -m "feat(web): derive workflow visual priority"
```

---

### Task 2: Apply Priority Metrics, Palette, and Label Ordering

**Files:**
- Modify: `web/src/lib/starmap/theme.ts`
- Create: `web/src/lib/starmap/theme.test.ts`
- Modify: `web/src/lib/starmap/starmap.ts`
- Modify: `web/src/lib/starmap/starmap.test.ts`

**Interfaces:**
- Consumes: `WorkPriority` and `ticketWorkPriority` from Task 1.
- Produces: `priorityStarStyle(vstate: VisualState, priority: WorkPriority): StarStyle`.
- Produces: `priorityLabelColor(vstate: VisualState, priority: WorkPriority): string`.
- Produces: private renderer `Node.priority: WorkPriority`.
- Preserves: `visualState(ticket)` and every public layout/camera API.

- [ ] **Step 1: Write failing metric tests**

Create `theme.test.ts`:

```ts
import { describe, expect, it } from 'vitest'
import { priorityLabelColor, priorityStarStyle } from './theme'

describe('priorityStarStyle', () => {
  it('orders active work without changing semantic palettes', () => {
    expect(priorityStarStyle('blocked', 'in_progress')).toMatchObject({ core: '#ffd873', r: 8.1, gr: 42 })
    expect(priorityStarStyle('frontier', 'ready')).toMatchObject({ core: '#8ad8ff', r: 7.2, gr: 34 })
    expect(priorityStarStyle('frontier', 'frontier')).toMatchObject({ core: '#8ad8ff', r: 6.2, gr: 28 })
    expect(priorityStarStyle('blocked', 'blocked')).toMatchObject({ core: '#e2c3c3', r: 4.5, gr: 20 })
    expect(priorityStarStyle('resolved', 'terminal')).toMatchObject({ core: '#b9d6c4', r: 5.4, gr: 24 })
    expect(priorityStarStyle('out_of_scope', 'terminal')).toMatchObject({ core: '#948da4', r: 4.5, gr: 18 })
    expect(priorityLabelColor('blocked', 'in_progress')).toBe('#ffe6a0')
    expect(priorityLabelColor('frontier', 'ready')).toBe('#b3e5ff')
  })
})
```

- [ ] **Step 2: Run red**

```powershell
C:\Users\pfdev\.vite-plus\bin\npm.exe --prefix web test -- src/lib/starmap/theme.test.ts
```

Expected: FAIL because `priorityStarStyle` is absent.

- [ ] **Step 3: Implement exact metrics**

Add to `theme.ts`:

```ts
export function priorityStarStyle(vstate: VisualState, priority: WorkPriority): StarStyle {
  if (priority === 'in_progress') return { ...STAR.claimed, r: 8.1, gr: 42 }
  if (priority === 'ready') return { ...STAR.frontier, r: 7.2, gr: 34 }
  if (priority === 'frontier') return { ...STAR.frontier, r: 6.2, gr: 28 }
  return STAR[vstate]
}

export function priorityLabelColor(vstate: VisualState, priority: WorkPriority): string {
  if (priority === 'in_progress') return LABEL.claimed
  if (priority === 'ready' || priority === 'frontier') return LABEL.frontier
  return LABEL[vstate]
}
```

- [ ] **Step 4: Ingest priority without changing structure**

Add `priority` to private `Node`. Set it with `ticketWorkPriority(t)` in both model-ingestion paths. Treat priority changes as paint/ticker changes, but do not add priority to `structureSignature`, layout input, or camera logic. Replace direct style reads in radius, glow, body, and label color with the priority-aware style.

- [ ] **Step 5: Write the failing label-order test**

Add a crowded `starmap.test.ts` fixture whose issue numbers oppose desired priority. Select issue `5`, make issue `6` the distinct CURRENT node, and assign in-progress `40`, ready `30`, frontier `20`, and blocked `10`. Assert retained label order:

```ts
expect(labels.map((label) => label.text.slice(0, 2))).toEqual(['05', '06', '40', '30', '20', '10'])
```

- [ ] **Step 6: Implement label ordering**

Keep selected first and a distinct CURRENT node second. Then order `in_progress`, `ready`, `frontier`, `blocked`, resolved, and out-of-scope. Use issue number only as the final same-priority tie-breaker.

- [ ] **Step 7: Run green**

```powershell
C:\Users\pfdev\.vite-plus\bin\npm.exe --prefix web test -- src/lib/starmap/theme.test.ts src/lib/starmap/starmap.test.ts
```

Expected: both files pass and deterministic layout assertions remain green.

- [ ] **Step 8: Commit**

```powershell
git add -- web/src/lib/starmap/theme.ts web/src/lib/starmap/theme.test.ts web/src/lib/starmap/starmap.ts web/src/lib/starmap/starmap.test.ts
git commit -m "feat(web): prioritize active work in the map"
```

---

### Task 3: Render 2.5D Spheres and the Two-Ring Grammar

**Files:**
- Modify: `web/src/lib/starmap/starmap.ts`
- Modify: `web/src/lib/starmap/core-visual.test.ts`
- Modify: `web/src/lib/StarMap.svelte`
- Modify: `web/src/lib/StarMap.test.ts`

**Interfaces:**
- Consumes: `Node.priority` and `priorityStarStyle` from Task 2.
- Produces: `StarMap.setReducedMotion(reduced: boolean): void`.
- Preserves: selection, session overlays, label text, and Canvas 2D ownership.

- [ ] **Step 1: Write failing sphere-layer tests**

Extend the `core-visual.test.ts` recorder so gradients retain origin/edge coordinates and arcs retain center/radius. Assert paint order is semantic glow, offset contact shadow, semantic body gradient, small above-left specular gradient, then one internal boundary.

For a ready body, assert exact body stops:

```ts
expect(bodyGradient.stops).toEqual([
  { at: 0, color: 'rgba(138,216,255,1)' },
  { at: 0.48, color: 'rgba(138,216,255,0.98)' },
  { at: 0.82, color: 'rgba(47,155,224,0.92)' },
  { at: 1, color: 'rgba(47,155,224,0.62)' },
])
expect(shadow.arc!.y).toBeGreaterThan(body.arc!.y)
expect(specular.arc!.radius).toBeLessThan(body.arc!.radius / 3)
```

- [ ] **Step 2: Write failing ring-count tests**

Add an `outerRings` helper that excludes the internal boundary by radius. Assert:

```ts
expect(outerRings(paint(IN_PROGRESS))).toHaveLength(1)
expect(outerRings(paint(READY_CHILD))).toHaveLength(1)
expect(outerRings(paint(BLOCKED))).toHaveLength(0)
expect(outerRings(paint(RESOLVED))).toHaveLength(0)
expect(outerRings(paint({ ...BLOCKED, parentIssue: 99 }))).toHaveLength(0)
expect(outerRings(paint(READY_CHILD, null, READY_CHILD.num))).toHaveLength(2)
expect(outerRings(paint(BLOCKED, null, BLOCKED.num))).toHaveLength(1)
```

Extend `paint` with a `selectedIssue` argument that calls `map.select` after
`setModel`. Record a selected click frame and assert no expanding flare stroke
is painted. Keep separate coverage proving a distinct CURRENT node receives no
outer CURRENT ring and retains its label/path semantics.

- [ ] **Step 3: Run red**

```powershell
C:\Users\pfdev\.vite-plus\bin\npm.exe --prefix web test -- src/lib/starmap/core-visual.test.ts
```

Expected: FAIL against the black disk, subissue rim, paired claimed/CURRENT rings, and ring flare.

- [ ] **Step 4: Implement sphere layers**

Calculate `priorityStarStyle` once. Keep the semantic glow, then paint:

- contact shadow centered at `y + cr * 0.72`, radius `cr * 1.08`, black alpha `0.26` to `0`;
- body gradient with an above-left origin and the exact stops from Step 1;
- specular centered at `x - cr * 0.32`, `y - cr * 0.34`, radius `cr * 0.28`, white alpha `0.46` to `0`;
- one internal boundary at `cr - lineWidth / 2`.

Use `c.core`, `c.glow`, and `hexA` for the body stops: core at `1`, core at
`0.98`, glow at `0.92`, and glow at `0.62`. Do not add a new semantic hue.

- [ ] **Step 5: Implement the ring grammar**

Delete `SUBISSUE_RIM`, the subissue rim, both CURRENT strokes, and the flare stroke. Paint one breathing amber ring at `cr + 7 + beat` for `in_progress`, one steady cyan ring at `cr + 7` for `ready`, and one white selection ring at `cr + 13` when a status ring exists or `cr + 7` otherwise. Keep `flare` only as a short semantic-glow multiplier.

- [ ] **Step 6: Add reduced-motion wiring**

Add private `#reducedMotion = false` and:

```ts
setReducedMotion(reduced: boolean): void {
  this.#reducedMotion = reduced
}
```

Use midpoint beat `0.5` when reduced motion is true. Continue decrementing the
private flare timer, but draw it as `fl > 0 ? 1 : 0` under reduced motion so the
glow remains visually static and then clears instead of interpolating. Add two
recorded reduced-motion frames while the timer is positive and assert identical
glow geometry/alpha. In `StarMap.svelte`, subscribe to
`matchMedia('(prefers-reduced-motion: reduce)')`, set the initial value, forward
changes, and remove the listener during cleanup. In `StarMap.test.ts`, assert
initial and changed values reach the same renderer instance.

- [ ] **Step 7: Run green**

```powershell
C:\Users\pfdev\.vite-plus\bin\npm.exe --prefix web test -- src/lib/starmap/core-visual.test.ts src/lib/StarMap.test.ts src/lib/starmap/starmap.test.ts
```

Expected: all files pass with maximum-two-ring and reduced-motion coverage.

- [ ] **Step 8: Commit**

```powershell
git add -- web/src/lib/starmap/starmap.ts web/src/lib/starmap/core-visual.test.ts web/src/lib/StarMap.svelte web/src/lib/StarMap.test.ts
git commit -m "feat(web): render dimensional issue nodes"
```

---

### Task 4: Shrink Arrowheads Without Weakening Direct Selection

**Files:**
- Modify: `web/src/lib/starmap/starmap.ts`
- Modify: `web/src/lib/starmap/edge-visual.test.ts`

**Interfaces:**
- Consumes: the existing selected-edge incident predicate and selected-last pass.
- Produces: shared arrow geometry with length `8` and half-width `4`.
- Preserves: selected stroke scale `1.7`, semantic styling, motion, full selected opacity, and direct-only order.

- [ ] **Step 1: Change arrow expectations first**

Update every ordinary, selected dependency, and selected parent/subissue assertion:

```ts
expectArrowDimensions(arrow, 8, 4)
```

Keep exact selected/non-selected stroke-order assertions and explicitly compare selected and context arrow dimensions for equality.

- [ ] **Step 2: Run red**

```powershell
C:\Users\pfdev\.vite-plus\bin\npm.exe --prefix web test -- src/lib/starmap/edge-visual.test.ts
```

Expected: FAIL because current ordinary arrows are `12` by `6.5` and selected arrows are `15` by `8.125`.

- [ ] **Step 3: Implement compact shared geometry**

Delete `SELECTED_EDGE_ARROW_SCALE` and use:

```ts
const ah = 8
const aw = 4
```

Do not change stroke scaling, paint order, alpha, color, dashes, curves, tangent direction, particles, or `#isSelectedEdge`.

- [ ] **Step 4: Run edge and node regressions**

```powershell
C:\Users\pfdev\.vite-plus\bin\npm.exe --prefix web test -- src/lib/starmap/edge-visual.test.ts src/lib/starmap/core-visual.test.ts
```

Expected: both files pass; direct-only selection and maximum-two-ring assertions stay green.

- [ ] **Step 5: Commit**

```powershell
git add -- web/src/lib/starmap/starmap.ts web/src/lib/starmap/edge-visual.test.ts
git commit -m "feat(web): reduce graph arrowheads"
```

---

### Task 5: Document, Validate, and Add the Tracker Label

**Files:**
- Modify: `CHANGELOG.md`
- Verify: every file changed since `origin/main`
- External mutation: create the approved `in-progress` label in `teloverge/stellr`

**Interfaces:**
- Consumes: Tasks 1-4.
- Produces: newest-first Unreleased notes and the approved tracker label.
- Produces no issue relabeling, push, PR, merge, release, or installer.

- [ ] **Step 1: Add the newest Unreleased entry**

Insert directly under `## Unreleased`:

```markdown
- Clarified active-work priority with dimensional issue nodes, a maximum of two
  status/selection rings, compact arrows, and literal `in-progress` label
  support ahead of `ready-for-agent` and blocked work.
```

- [ ] **Step 2: Run frontend verification**

```powershell
C:\Users\pfdev\.vite-plus\bin\npm.exe --prefix web test
C:\Users\pfdev\.vite-plus\bin\npm.exe --prefix web run check
C:\Users\pfdev\.vite-plus\bin\npm.exe --prefix web run build
```

Expected: all Vitest files pass, Svelte reports `0 errors and 0 warnings`, and Vite exits `0`.

- [ ] **Step 3: Run native workspace verification**

```powershell
cargo.exe fmt --all -- --check
cargo.exe clippy --workspace --all-targets --locked -- -D warnings
cargo.exe test --workspace --locked -- --test-threads=1
```

Expected: all commands exit `0`; only explicitly ignored tests remain unrun.

- [ ] **Step 4: Verify scope and generated-file cleanliness**

```powershell
git diff --check origin/main...HEAD
git status --short --branch
git diff --stat origin/main...HEAD
```

If Tauri changes only the nine known `crates/app/permissions/autogenerated/*.toml` files, first prove `git diff --ignore-space-at-eol --exit-code` for those exact paths, then restore only those files. Never restore a semantic diff.

- [ ] **Step 5: Perform native visual verification**

Build and launch the updated native desktop binary from the isolated worktree:

```powershell
cargo.exe build --package stellr-app --release --bin stellr-desktop
Start-Process -FilePath target\release\stellr-desktop.exe -WorkingDirectory target\release
```

Inspect the live map at ordinary and selected zoom levels. Confirm an assigned
or `in-progress` issue is the strongest sphere with one amber ring, a
`ready-for-agent` issue has one quieter cyan ring, blocked and completed nodes
have no outer status ring, selection adds exactly one white ring, nodes retain
their positions, and arrowheads remain readable at the compact size. Capture
the selected-ready and selected-in-progress states when live data provides
them; otherwise use the focused canvas recordings as the deterministic evidence
for those combinations and state that limitation explicitly.

- [ ] **Step 6: Commit changelog after gates pass**

```powershell
git add -- CHANGELOG.md
git commit -m "docs: record node visual hierarchy"
```

- [ ] **Step 7: Create and verify the approved tracker label**

Recheck that it is absent:

```powershell
gh label list --repo teloverge/stellr --limit 100 --json name --jq ".[].name"
```

If absent, create exactly:

```powershell
gh label create in-progress --repo teloverge/stellr --color fbca04 --description "Work is actively being implemented"
```

Verify without applying it to any issue:

```powershell
gh label list --repo teloverge/stellr --search in-progress --json name,color,description
```

Expected: one `in-progress` label, color `fbca04`, with the approved description.

- [ ] **Step 8: Run final committed-state verification**

```powershell
C:\Users\pfdev\.vite-plus\bin\npm.exe --prefix web test
git status --short --branch
git log --oneline origin/main..HEAD
```

Expected: frontend tests pass, the worktree is clean, and the branch contains only the design, plan, implementation, and changelog commits.

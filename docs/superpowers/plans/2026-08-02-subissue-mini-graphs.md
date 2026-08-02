# Subissue Mini-Graphs Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver Issues #39, #40, and #41 by carrying GitHub native parent identity into Stellr and rendering each parent/subissue hierarchy as a directed, state-aware mini-graph.

**Architecture:** Add one nullable `parent_issue` field across the existing provider/core/server/web issue contract. Derive typed display topology in a pure web module; dependency rank continues to use only `blocked_by`, while layout springs and focus may consume the directed display topology. Keep canvas paint policy behind focused pure helpers so curve direction, edge state, and animation can be tested independently.

**Tech Stack:** Native Windows Rust/Cargo, Octocrab GraphQL, Serde JSON cache, Svelte 5/TypeScript 6, Vitest/jsdom, existing canvas StarMap renderer.

## Global Constraints

- Native Windows 11 only; use PowerShell, `vp`, `cargo.exe`, and Windows paths.
- Approved design source: `D:\dev\stellr\docs\superpowers\specs\2026-08-02-subissue-mini-graphs-design.md`.
- Work Issues #39, #40, and #41 in blocker order; only the unblocked frontier carries `ready-for-agent`.
- Use strict red-green-refactor and observe every focused failure before production changes.
- `blocked_by` remains the only relationship used to derive readiness/status and dependency rank.
- Parent-entry and child-return edges never affect dependency rank.
- Preserve black background, CURRENT/READY precedence, solid/hollow node grammar, deterministic layout, and all non-mini-graph dependency styling.
- Missing or out-of-space parent data never prevents the map from rendering.
- Older cached snapshots without `parent_issue` must deserialize successfully.
- Update `CHANGELOG.md` only under the newest `Unreleased` section.

---

### Task 1: Native parent identity contract for Issue #39

**Files:**
- Modify: `crates/core/src/model.rs`
- Modify: `crates/core/src/derive.rs`
- Modify: `crates/github/src/sync.rs`
- Modify: `crates/github/tests/sync_test.rs`
- Modify: `crates/github/src/cache.rs`
- Modify: Rust fixtures constructing `RawIssue` or `Star` under `crates/`
- Modify: `web/src/lib/model.ts`
- Modify: frontend fixtures constructing `Star` under `web/src/`

**Interfaces:**
- Produces: `RawIssue.parent_issue: Option<u64>`, `Star.parent_issue: Option<u64>`, JSON `parent_issue: number | null`, and TypeScript `Star.parent_issue: number | null`.
- Preserves: all existing blocker and status behavior.

- [ ] **Step 1: Add failing core model compatibility tests**

Extend `model_round_trips` with a child whose `parent_issue` is `Some(16)`. Add a literal old-model JSON test that omits `parent_issue` and expects `None` after deserialization. The old JSON must be handwritten rather than serialized by the new model.

```rust
assert_eq!(round_tripped.spaces[0].stars[0].parent_issue, Some(16));

let old: Model = serde_json::from_str(OLD_MODEL_WITHOUT_PARENT).unwrap();
assert_eq!(old.spaces[0].stars[0].parent_issue, None);
```

- [ ] **Step 2: Run the focused core model test RED**

```powershell
cargo.exe test -p stellr-core model::tests --locked
```

Expected: compile failure because `Star` has no `parent_issue` field.

- [ ] **Step 3: Add the nullable model fields minimally**

```rust
pub struct Star {
    // existing fields
    #[serde(default)]
    pub parent_issue: Option<u64>,
}

pub struct RawIssue {
    // existing fields
    #[serde(default)]
    pub parent_issue: Option<u64>,
}
```

Update all Rust literals with `parent_issue: None` except the focused fixtures.

- [ ] **Step 4: Add failing derivation tests for copy and self-parent rejection**

Change the core test fixture to accept `parent_issue: Option<u64>`. Append `None` to every existing `issue(...)` fixture call, then assert:

```rust
let stars = derive(&[
    issue(16, IssueState::Open, &[], &[], None),
    issue(39, IssueState::Open, &[], &[], Some(16)),
    issue(40, IssueState::Open, &[], &[39], Some(40)),
]);
assert_eq!(stars[1].parent_issue, Some(16));
assert_eq!(stars[2].parent_issue, None);
assert_eq!(stars[2].status, Status::Blocked);
```

The last assertion catches hierarchy leaking into status derivation.

- [ ] **Step 5: Run the derivation test RED**

```powershell
cargo.exe test -p stellr-core derive::tests --locked
```

Expected: copied child parent is missing or self-parent remains present.

- [ ] **Step 6: Implement parent derivation minimally**

In `derive`, set:

```rust
parent_issue: issue.parent_issue.filter(|parent| *parent != issue.number),
```

Do not require the parent to exist in the snapshot; the web topology boundary owns out-of-space filtering.

- [ ] **Step 7: Add failing GitHub provider mapping tests**

Extend the literal GraphQL node fixture with `parent: Option<u64>` and emit:

```rust
"parent": parent.map(|number| json!({ "number": number }))
```

Give Issue 3 parent 16 and another issue no parent. Assert `RawIssue.parent_issue` is `Some(16)` and `None`. Also require the outgoing query body to contain `parent { number }`.

- [ ] **Step 8: Run the provider test RED**

```powershell
cargo.exe test -p stellr-github --test sync_test fetch_maps_complete_issue_shape_and_merges_dependency_sources --locked
```

Expected: the response shape is ignored or `IssueNode` cannot map the new fixture expectation.

- [ ] **Step 9: Implement provider mapping**

Add `parent { number }` to `FETCH_ISSUES_QUERY`, then:

```rust
#[derive(Deserialize)]
struct ParentIssue {
    number: u64,
}

struct IssueNode {
    // existing fields
    parent: Option<ParentIssue>,
}
```

Map `parent_issue: node.parent.map(|parent| parent.number)` into `RawIssue`.

- [ ] **Step 10: Add and pass old-cache compatibility coverage**

In cache tests, write a literal snapshot JSON whose `RawIssue` lacks `parent_issue`; load it through `Cache::load` and assert `snapshot.issues[0].parent_issue == None`.

```powershell
cargo.exe test -p stellr-github cache::tests --locked
```

- [ ] **Step 11: Extend the web contract and fixtures**

Add:

```ts
export interface Star {
  // existing fields
  parent_issue: number | null
}
```

Update every frontend `Star` fixture with `parent_issue: null`; no renderer behavior changes in this task.

- [ ] **Step 12: Run Issue #39 GREEN gates**

```powershell
cargo.exe test -p stellr-core --locked
cargo.exe test -p stellr-github --locked
Set-Location web
vp run test
vp run check
```

Expected: all tests pass and Svelte reports 0 errors/warnings.

- [ ] **Step 13: Commit and advance the tracker frontier**

```powershell
git add crates web
git commit -m "feat: expose native issue parents"
```

After verification, close #39 with evidence, remove `ready-for-agent` from #39, and add `ready-for-agent` to now-unblocked #40. Do not modify parent #16's body/title/labels.

---

### Task 2: Pure directed mini-graph topology for Issue #40

**Files:**
- Modify: `web/src/lib/starmap/model.ts`
- Modify: `web/src/lib/starmap/adapt.ts`
- Modify: `web/src/lib/starmap/adapt.test.ts`
- Create: `web/src/lib/starmap/workflow.ts`
- Create: `web/src/lib/starmap/workflow.test.ts`

**Interfaces:**
- Consumes: the structural `WorkflowNode` fields `num`, `parentIssue`, and `blockedBy`; topology must not depend on renderer-only fields or status.
- Produces: `workflowEdges(nodes: WorkflowNode[]): WorkflowEdge[]`.

```ts
export type WorkflowRole = 'dependency' | 'entry' | 'sequence' | 'return'

export interface WorkflowNode {
  num: number
  blockedBy: number[]
  parentIssue: number | null
}

export interface WorkflowEdge {
  from: number
  to: number
  roles: WorkflowRole[]
  child: number | null
}
```

Roles are sorted in `dependency`, `entry`, `sequence`, `return` order. Edges are sorted by `from`, then `to`; duplicate `(from,to)` edges merge roles and retain the non-null child.

- [ ] **Step 1: Write a failing adapter test**

Set `parent_issue: 16` on a source star and assert the renderer ticket has `parentIssue: 16`; assert a null source remains null.

```powershell
Set-Location web
vp exec vitest run src/lib/starmap/adapt.test.ts
```

Expected: `parentIssue` is absent.

- [ ] **Step 2: Add the renderer field and adapter mapping**

```ts
export interface Ticket {
  // existing fields
  parentIssue: number | null
}
```

Map `parentIssue: star.parent_issue` and update renderer ticket fixtures with `parentIssue: null`.

- [ ] **Step 3: Write failing topology tests**

Use literal tickets to assert these exact edges:

```ts
expect(workflowEdges(independentChildren)).toEqual([
  { from: 14, to: 34, roles: ['entry'], child: 34 },
  { from: 14, to: 35, roles: ['entry'], child: 35 },
  { from: 34, to: 14, roles: ['return'], child: 34 },
  { from: 35, to: 14, roles: ['return'], child: 35 },
])

expect(workflowEdges(sequentialChildren)).toEqual([
  { from: 16, to: 37, roles: ['entry'], child: 37 },
  { from: 37, to: 38, roles: ['dependency', 'sequence'], child: 38 },
  { from: 38, to: 16, roles: ['return'], child: 38 },
])
```

Also cover a lone child, nested parent/child groups, self-parent, missing parent, and a dependency edge outside any mini-graph.

- [ ] **Step 4: Run topology tests RED**

```powershell
vp exec vitest run src/lib/starmap/workflow.test.ts
```

Expected: module-not-found for `./workflow`.

- [ ] **Step 5: Implement topology minimally**

Build the existing dependency edges first. Group valid children by `parentIssue`, compute sibling incoming/outgoing dependency sets, then merge entry and return roles through one `(from,to)` keyed map. Do not read status in topology construction. Both the full renderer `Ticket` and layout's lighter `LayoutNode` must satisfy `WorkflowNode` structurally.

- [ ] **Step 6: Run topology and adapter tests GREEN**

```powershell
vp exec vitest run src/lib/starmap/workflow.test.ts src/lib/starmap/adapt.test.ts
vp run check
```

- [ ] **Step 7: Commit Task 2**

```powershell
git add web/src/lib/starmap
git commit -m "feat(web): derive subissue workflow loops"
```

---

### Task 3: Cycle-safe deterministic layout for Issue #40

**Files:**
- Modify: `web/src/lib/starmap/layout.ts`
- Modify: `web/src/lib/starmap/layout.test.ts`
- Modify: `web/src/lib/starmap/starmap.ts`
- Test: `web/src/lib/starmap/starmap.test.ts`

**Interfaces:**
- `edgesOf(nodes)` remains dependency-only and preserves its current signature.
- `computeLayout(nodes)` uses `workflowEdges(nodes)` only for spring relaxation.
- `structureSignature(nodes)` includes sorted workflow edge roles.

- [ ] **Step 1: Write failing rank and signature tests**

For #16→#37→#38→#16, assert dependency ranks remain `{16: 0, 37: 0, 38: 1}`. Assert adding `parentIssue: 16` changes `structureSignature`, while changing only statuses does not.

- [ ] **Step 2: Run layout tests RED**

```powershell
vp exec vitest run src/lib/starmap/layout.test.ts
```

Expected: parent identity does not change signature and hierarchy springs are absent.

- [ ] **Step 3: Integrate workflow springs without changing rank**

Keep `rankOf` calling dependency-only `edgesOf`. In `computeLayout`, use `workflowEdges(sorted)` for edge springs. In `structureSignature`, encode each edge as:

```ts
`${edge.from}>${edge.to}:${edge.roles.join('+')}`
```

- [ ] **Step 4: Write a renderer stability regression**

Change only ticket status and assert the renderer preserves node coordinates; then add a parent relationship and assert one structural layout recomputation occurs.

- [ ] **Step 5: Run layout/renderer suites GREEN**

```powershell
vp exec vitest run src/lib/starmap/layout.test.ts src/lib/starmap/starmap.test.ts
vp run check
```

- [ ] **Step 6: Commit Task 3**

```powershell
git add web/src/lib/starmap/layout.ts web/src/lib/starmap/layout.test.ts web/src/lib/starmap/starmap.ts web/src/lib/starmap/starmap.test.ts
git commit -m "feat(web): lay out subissue workflow cycles"
```

---

### Task 4: Curved state-aware mini-graph paint for Issue #40

**Files:**
- Create: `web/src/lib/starmap/workflow-visual.ts`
- Create: `web/src/lib/starmap/workflow-visual.test.ts`
- Modify: `web/src/lib/starmap/starmap.ts`
- Modify: `web/src/lib/starmap/edge-visual.test.ts`
- Modify: `web/src/lib/starmap/core-visual.test.ts`

**Interfaces:**

```ts
export type WorkflowVisualState = 'incomplete' | 'completed' | 'traversed'

export function workflowVisualState(
  edge: WorkflowEdge,
  tickets: Map<number, Ticket>,
): WorkflowVisualState

export function curveSide(edge: WorkflowEdge, reverseExists: boolean): -1 | 1
```

State precedence: traversed (`source.status === 'resolved' && target.status === 'frontier'`), then completed (associated child resolved), then incomplete.

- [ ] **Step 1: Write failing pure visual-policy tests**

Assert:

```ts
expect(workflowVisualState(entryToOpenChild, tickets)).toBe('incomplete')
expect(workflowVisualState(returnFromResolvedChild, tickets)).toBe('completed')
expect(workflowVisualState(sequenceFromResolvedToFrontier, tickets)).toBe('traversed')
expect(workflowVisualState(sequenceFromResolvedToBlocked, tickets)).toBe('incomplete')
```

Assert reverse parent/child edges receive opposite curve sides, while a single directed edge receives the deterministic default side.

- [ ] **Step 2: Run pure visual tests RED**

```powershell
vp exec vitest run src/lib/starmap/workflow-visual.test.ts
```

Expected: module-not-found.

- [ ] **Step 3: Implement visual policy minimally**

Use `edge.child` to resolve completed/incomplete state and source/target tickets for traversed state. `curveSide` must be deterministic from direction, not input order.

- [ ] **Step 4: Extend the recording canvas test and verify RED**

Record quadratic control points as well as endpoints. Paint a lone incomplete child and assert two strokes curve to opposite sides, both arrows point in their edge direction, and the incomplete stroke/arrow use:

```text
stroke: rgba(170,145,255,0.78), width 2.6, dash [8,7]
arrow: #c7b8ff
```

Paint a resolved child and assert the existing resolved/mint solid stroke and arrow. Assert a resolved-but-not-traversed mini-graph edge paints no radius-5/2.6 motion particles.

- [ ] **Step 5: Implement curved renderer edges**

Store `WorkflowEdge` plus visual state in `#edges`. Draw every mini-graph edge as a quadratic curve. When a reverse edge exists, use opposite bow directions. Keep the existing 12×6.5 arrow geometry tangent to the quadratic curve at its midpoint; do not aim it along the straight endpoint vector.

Non-mini-graph dependencies keep their current colors, widths, dashes, and satisfied-edge motion.

- [ ] **Step 6: Add failing incomplete-child rim coverage**

In `core-visual.test.ts`, paint an incomplete child and assert one violet rim outside its existing hollow core. Paint a resolved child and assert no violet rim. CURRENT and READY fixtures must prove their existing rings are painted after/outside the relationship rim.

- [ ] **Step 7: Implement the relationship rim**

Use a single token constant in renderer code:

```ts
const SUBISSUE_RIM = 'rgba(170,145,255,0.82)'
```

Draw the rim only when `parentIssue !== null` and the node is neither `resolved` nor `out_of_scope`. Preserve the existing core fill/hollow paths.

- [ ] **Step 8: Run Issue #40 GREEN gates**

```powershell
vp exec vitest run src/lib/starmap/workflow.test.ts src/lib/starmap/layout.test.ts src/lib/starmap/workflow-visual.test.ts src/lib/starmap/edge-visual.test.ts src/lib/starmap/core-visual.test.ts src/lib/starmap/starmap.test.ts
vp run check
vp run build
```

- [ ] **Step 9: Commit and advance the tracker frontier**

```powershell
git add web/src/lib/starmap
git commit -m "feat(web): render subissue workflow loops"
```

After verification, close #40 with evidence, remove `ready-for-agent` from #40, and add `ready-for-agent` to now-unblocked #41.

---

### Task 5: Cycle-safe focus and traversed motion for Issue #41

**Files:**
- Modify: `web/src/lib/starmap/focus.ts`
- Modify: `web/src/lib/starmap/focus.test.ts`
- Modify: `web/src/lib/starmap/starmap.ts`
- Modify: `web/src/lib/starmap/edge-visual.test.ts`

**Interfaces:**
- `analyzeFocus(tickets, requestedCurrent)` consumes `workflowEdges(tickets)`.
- Directed search tries CURRENT→READY first, then READY→CURRENT to preserve existing dependency focus paths.
- `Focus.pathEdges` always stores each display edge in its actual arrow direction.

- [ ] **Step 1: Write failing mini-graph focus tests**

Assert:

```ts
// Current parent enters its ready root child.
expect([...analyzeFocus(parentAndRoot, 16).pathEdges]).toEqual(['16>37'])

// Resolved sibling leads to the next ready child without looping through parent.
expect([...analyzeFocus(sequential, 16).pathEdges]).toEqual(['16>37', '37>38'])

// Existing ready-to-current blocker path remains directed 8>12>14.
expect([...analyzeFocus(existingDependencyFixture, 14).pathEdges]).toEqual(['8>12', '12>14'])
```

Also include a full parent cycle and prove the search terminates with one stable shortest path.

- [ ] **Step 2: Run focus tests RED**

```powershell
vp exec vitest run src/lib/starmap/focus.test.ts
```

Expected: no parent-entry path is found.

- [ ] **Step 3: Implement bidirectional search policy**

Build adjacency from `workflowEdges`. Search `current → ready`; if absent, search `ready → current`. When the fallback search is used, retain actual directed edge keys rather than reversing them. Keep closed intermediate-node filtering, except a resolved node may be traversed when it lies on a CURRENT→READY mini-graph route.

- [ ] **Step 4: Add failing traversed-motion renderer tests**

For a mini-graph sequence:

- resolved source + frontier destination: exactly three halo/core particle pairs;
- resolved source + blocked destination: zero particles;
- open source + frontier destination: zero particles;
- contextual traversed edge: particle effective alpha includes `CONTEXT_EDGE_ALPHA`;
- CURRENT/READY path edge: full alpha.

- [ ] **Step 5: Gate mini-graph motion on traversed state**

Render particle motion for mini-graph edges only when `workflowVisualState(...) === 'traversed'`. Leave existing non-mini dependency satisfied-edge motion unchanged.

- [ ] **Step 6: Run Issue #41 GREEN gates**

```powershell
vp exec vitest run src/lib/starmap/focus.test.ts src/lib/starmap/workflow-visual.test.ts src/lib/starmap/edge-visual.test.ts src/lib/starmap/starmap.test.ts
vp run test
vp run check
vp run build
```

- [ ] **Step 7: Commit and resolve the ticket**

```powershell
git add web/src/lib/starmap
git commit -m "feat(web): focus traversed subissue routes"
```

After verification, close #41 with evidence and remove `ready-for-agent` from it. Leave parent #16 open until the complete parent acceptance gate passes.

---

### Task 6: Release note, native gate, and live Windows acceptance

**Files:**
- Modify: `CHANGELOG.md`
- Verify: all changed Rust/web contracts and the embedded browser

**Interfaces:**
- Consumes: completed #39→#40→#41 implementation.
- Produces: final Issue #16 evidence and a clean local branch ready for the user's integration choice.

- [ ] **Step 1: Add the newest Unreleased note**

Add one newest bullet describing native subissue mini-graphs, directed entry/sequence/return arrows, incomplete violet grammar, and traversed motion. Do not edit older release sections.

- [ ] **Step 2: Run final frontend verification**

```powershell
Set-Location web
vp run test
vp run check
vp run build
```

Remove only Vite+'s generated `devEngines` block after the last `vp` command and validate `package.json` as JSON.

- [ ] **Step 3: Run final native verification**

```powershell
Set-Location ..
cargo.exe fmt --all -- --check
cargo.exe test --workspace --locked
cargo.exe clippy --workspace --all-targets --locked -- -D warnings
cargo.exe build --workspace --locked
```

- [ ] **Step 4: Run headed embedded-browser acceptance**

Start the freshly built binary on a fixed loopback port with `--issue 16 --no-token`. In a headed Windows browser, verify:

1. #14 has independent loops through #34 and #35;
2. #16 has the sequence #16→#37→#38→#16;
3. all arrows and opposing curves are visible on black;
4. incomplete routes/rims are violet and completed routes/nodes use resolved grammar;
5. only source-resolved-to-destination-ready mini-graph links animate;
6. wide/narrow M1 chrome and issue detail remain functional;
7. browser console reports 0 errors/warnings.

- [ ] **Step 5: Final diff and tracker audit**

```powershell
git diff --check
git status --short --branch
git diff --stat origin/main...HEAD
gh api 'repos/teloverge/stellr/issues/16/sub_issues?per_page=100' --jq '.[].number'
```

Confirm #39/#40/#41 are closed with no actionable labels, #16 remains open until final evidence is posted, the primary checkout's unrelated changes are untouched, and both approved specs remain in `D:\dev\stellr\docs\superpowers\specs`.

- [ ] **Step 6: Commit final evidence-facing change**

```powershell
git add CHANGELOG.md
git commit -m "docs: record subissue mini-graphs"
```

- [ ] **Step 7: Review and finish the branch**

Run the two-axis standards/spec review against `origin/main...HEAD`, address findings test-first, rerun affected full gates, then use `superpowers:finishing-a-development-branch` to present the three integration options. Do not push, merge, or create a PR before the user chooses.

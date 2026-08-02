# Session-Focus Star Map Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Stellr identify the issue owned by the current conversation, foreground actionable `ready-for-agent` work and its dependency path, and remove decorative clutter.

**Architecture:** The CLI places optional viewer-session context in the cockpit URL through `--issue`; it does not contaminate the shared GitHub model. A pure frontend focus module combines that issue number with adapter-derived actionability, while the renderer consumes the result for emphasis and camera framing without changing deterministic node positions.

**Tech Stack:** Rust 2024, Clap 4, Axum, Svelte 5, TypeScript 6, Canvas 2D, Vitest 4, Vite+

## Global Constraints

- Use native Windows PowerShell and native Windows executables only.
- Keep the pure-black canvas and remove only the decorative parallax starfield.
- Current conversation identity comes only from `stellr serve --issue <positive-number>`.
- An actionable issue must be both `frontier` and labeled `ready-for-agent`, case-insensitively.
- Preserve deterministic status-independent node positions and all pan, zoom, selection, and session-overlay behavior.
- Multiply issue core radii by 1.25, leave glow radii unchanged, and use a 10-16 px adaptive label range.
- Render unrelated context at 0.3 opacity and unrelated edges at 0.2 of their existing opacity.
- Focus fit uses 150 world units of padding, 0.8 of the available fit scale, and a maximum scale of 1.0.
- Preserve all unrelated Issue #14 work and the dirty primary checkout.
- Do not add dependencies, commit, push, or mutate the GitHub tracker without separate user authorization.

---

### Task 1: Derive actionable work and its path in a pure module

**Files:**
- Modify: `web/src/lib/starmap/model.ts`
- Modify: `web/src/lib/starmap/adapt.ts`
- Modify: `web/src/lib/starmap/adapt.test.ts`
- Create: `web/src/lib/starmap/focus.ts`
- Create: `web/src/lib/starmap/focus.test.ts`

**Interfaces:**
- Consumes: `Ticket[]`, optional `Ticket.readyForAgent`, and `currentIssue: number | null`
- Produces: `analyzeFocus(tickets, currentIssue): Focus` and `edgeKey(from, to): string`

- [ ] **Step 1: Write the failing adapter test**

Set three literal fixture combinations in `adapt.test.ts` and assert:

```ts
expect(model.map((ticket) => ticket.readyForAgent)).toEqual([
  true,  // frontier + READY-FOR-AGENT
  false, // frontier without the label
  false, // blocked + ready-for-agent
])
```

- [ ] **Step 2: Write failing focus-analysis tests**

Create `focus.test.ts` with this primary case:

```ts
const tickets: Ticket[] = [
  ticket(8, 'frontier', [], true),
  ticket(12, 'blocked', [8]),
  ticket(14, 'blocked', [12]),
  ticket(21, 'frontier', [], true),
]
const focus = analyzeFocus(tickets, 14)
expect(focus.current).toBe(14)
expect(focus.ready).toEqual([8, 21])
expect([...focus.pathNodes]).toEqual([8, 12, 14])
expect([...focus.pathEdges]).toEqual(['8>12', '12>14'])
expect([...focus.emphasized]).toEqual([14, 8, 21, 12])
```

Add literal cases for no current issue, current-is-ready, missing current,
resolved intermediates, and a cycle.

- [ ] **Step 3: Run tests and verify RED**

From `web`:

```powershell
vp run test -- src/lib/starmap/adapt.test.ts src/lib/starmap/focus.test.ts
```

Expected: compile failures because `readyForAgent`, `Focus`, and `analyzeFocus`
do not exist.

- [ ] **Step 4: Add adapter-owned actionability**

Add to `Ticket`:

```ts
readyForAgent?: boolean
```

Set it in `toRendererModel`:

```ts
readyForAgent:
  star.status === 'frontier' &&
  star.labels.some((label) => label.toLowerCase() === 'ready-for-agent'),
```

- [ ] **Step 5: Implement deterministic focus analysis**

Create `focus.ts` with:

```ts
import type { Ticket } from './model'

export interface Focus {
  current: number | null
  ready: number[]
  pathNodes: Set<number>
  pathEdges: Set<string>
  emphasized: Set<number>
}

export function edgeKey(from: number, to: number): string {
  return `${from}>${to}`
}
```

Build blocker-to-dependent adjacency from `blockedBy`. For each actionable issue
in ascending number order, breadth-first search toward the current issue. Skip
resolved and out-of-scope intermediate dependents, record one parent per visited
node, reconstruct the shortest found path, and union its nodes and edges. A
visited set makes cycles finite. Order `ready` as actionable path origins first,
then remaining actionable issues, each subgroup ascending. Build `emphasized`
by inserting current, ordered ready nodes, then path nodes.

- [ ] **Step 6: Verify GREEN**

From `web`:

```powershell
vp run test -- src/lib/starmap/adapt.test.ts src/lib/starmap/focus.test.ts
vp run check
```

Expected: adapter and all focus cases pass; type checking is clean.

---

### Task 2: Render the execution hierarchy without decorative clutter

**Files:**
- Modify: `web/src/lib/starmap/starmap.ts`
- Modify: `web/src/lib/starmap/starmap.test.ts`

**Interfaces:**
- Consumes: `analyzeFocus`, `edgeKey`, `currentIssue`, and `Ticket.readyForAgent`
- Produces: focus-aware `setModel(tickets, sessions?, currentIssue?)` rendering

- [ ] **Step 1: Write failing starfield-removal test**

Extend the recording canvas context to capture every `fillRect(x, y, w, h)`.
Mount an empty renderer, advance one frame, and assert exactly one rectangle is
painted: the full-canvas background. The current renderer fails because it adds
254 tiny decorative rectangles.

- [ ] **Step 2: Write failing focus-label and stability tests**

Use the literal `#8 -> #12 -> #14` fixture plus unrelated ready #21:

```ts
sm.setModel(tickets, {}, 14)
```

Advance a frame and assert:

```ts
expect(texts.some((text) => text.startsWith('CURRENT · 14'))).toBe(true)
expect(texts.some((text) => text.startsWith('READY · 08'))).toBe(true)
```

Assert the rendered labels reflect the pure `analyzeFocus` result, and retain
the existing byte-for-byte stable-position test. Do not add a test-only getter
to the production renderer.

- [ ] **Step 3: Run renderer tests and verify RED**

From `web`:

```powershell
vp run test -- src/lib/starmap/starmap.test.ts
```

Expected: decorative rectangles remain, focus labels are absent, and the third
`setModel` argument is unsupported.

- [ ] **Step 4: Remove the decorative starfield**

Delete `StarLayer`, `makeStarfield`, `#starfield`, `#drawStarfield`, and the call
to `#drawStarfield(g)`. Keep the initial background `fillRect` in `#draw`.

- [ ] **Step 5: Add focus state and scaled radii**

Import `analyzeFocus`, `edgeKey`, and `Focus`. Add:

```ts
const ISSUE_RADIUS_SCALE = 1.25
const CONTEXT_ALPHA = 0.3
const CONTEXT_EDGE_ALPHA = 0.2

#focus: Focus = analyzeFocus([], null)

#radius(n: Node): number {
  return STAR[n.vstate].r * ISSUE_RADIUS_SCALE
}
```

Extend the seam:

```ts
setModel(
  tickets: Ticket[],
  sessions: Record<number, SessionState> = {},
  currentIssue: number | null = null,
): void
```

Recompute `#focus` before drawing or refitting. Replace every core-radius
consumer in drawing, hit testing, session orbits, selection rings, and label
obstacles with `#radius(n)`. Leave glow radius `gr` unchanged.

- [ ] **Step 6: Apply node and edge emphasis**

Around each node draw, keeping every node fully visible when no focus exists:

```ts
g.save()
g.globalAlpha =
  this.#focus.emphasized.size === 0 || this.#focus.emphasized.has(n.num)
    ? 1
    : CONTEXT_ALPHA
this.#drawStar(g, n, this.#clock)
g.restore()
```

In `#drawEdge`, preserve existing colors and multiply `globalAlpha` by
`CONTEXT_EDGE_ALPHA` only when a focus exists and `pathEdges` does not contain
`edgeKey(e.from, e.to)`. Draw two white rings around the current node at scaled
radius plus 8 and 13 pixels.

- [ ] **Step 7: Apply focus labels and larger type**

Use:

```ts
const fs = clamp(13 * Math.pow(this.#cam.s, 0.3), 10, 16)
```

Sort labels as current, ready, selected, path, then existing status priority.
Build text with:

```ts
const current = this.#focus.current === v.n.num
const ready = this.#focus.ready.includes(v.n.num)
const prefix = current && ready
  ? 'CURRENT / READY · '
  : current
    ? 'CURRENT · '
    : ready
      ? 'READY · '
      : ''
let text = prefix + (v.n.num < 10 ? '0' : '') + v.n.num
```

Add `alpha` to `LabelDraw`; draw emphasized labels at 1 and context at 0.3. If
the emphasized set is empty, draw every label at 1. Current and ready labels
enter the collision solver first and reserve slots.

- [ ] **Step 8: Fit the compact focus set**

In `#refit`, choose emphasized nodes when the set is nonempty, otherwise all
nodes. Use:

```ts
const focused = this.#focus.emphasized.size > 0
const pad = focused ? 150 : 90
const available = Math.min(
  availW / (maxx - minx || 1),
  availH / (maxy - miny || 1),
)
const s = focused
  ? clamp(available * 0.8, 0.15, 1.0)
  : clamp(available, 0.15, 1.4)
```

If focus changes on a same-structure push, call `#refit(false)` without changing
any node's `x` or `y`.

- [ ] **Step 9: Verify GREEN and renderer invariants**

From `web`:

```powershell
vp run test -- src/lib/starmap/starmap.test.ts
vp run test
vp run check
vp run build
```

Expected: no decorative points, focus tests pass, all existing collision,
session, camera, selection, and stable-position tests remain green, and the
production bundle builds with zero Svelte errors/warnings.

---

### Task 3: Carry the conversation issue in the cockpit URL

**Files:**
- Modify: `crates/app/src/cli.rs`
- Modify: `crates/app/src/main.rs`
- Modify: `crates/app/tests/serve_test.rs`

**Interfaces:**
- Consumes: `stellr serve --issue <positive u64>`
- Produces: `ServeArgs.issue: Option<NonZeroU64>` and cockpit URLs whose query contains `issue=N`

- [ ] **Step 1: Write failing CLI and URL integration tests**

In `crates/app/src/main.rs`, extend the existing parser tests:

```rust
assert_eq!(args.issue, None);

let parsed = Cli::try_parse_from([
    "stellr", "serve", "--addr", "127.0.0.1:0", "--issue", "14",
]).unwrap();
let Command::Serve(args) = parsed.command;
assert_eq!(args.issue.map(NonZeroU64::get), Some(14));

assert!(Cli::try_parse_from(["stellr", "serve", "--issue", "0"]).is_err());
```

Import `std::num::NonZeroU64` inside the test module.

In `crates/app/tests/serve_test.rs`, launch the authenticated server with
`--issue 14` and assert both query values independently:

```rust
let issue = tokened_url
    .query_pairs()
    .find_map(|(name, value)| (name == "issue").then(|| value.into_owned()));
assert_eq!(issue.as_deref(), Some("14"));
```

Also launch the `--no-token` case with `--issue 14`, parse it as `reqwest::Url`,
and assert that its query has exactly `issue=14` while the root and public
assets remain available. Build the model URL with `url.join("api/model")`
instead of string concatenation so the page query cannot corrupt the path.

- [ ] **Step 2: Run the app tests and verify RED**

```powershell
cargo.exe test -p stellr-app --locked
```

Expected: compilation or assertion failure because `ServeArgs` has no `issue`
field and startup URLs omit it.

- [ ] **Step 3: Add the positive issue argument**

In `crates/app/src/cli.rs`:

```rust
use std::num::NonZeroU64;

#[derive(Args)]
pub struct ServeArgs {
    #[arg(long, default_value = "127.0.0.1:8787")]
    pub addr: String,
    #[arg(long)]
    pub no_token: bool,
    #[arg(long)]
    pub issue: Option<NonZeroU64>,
}
```

- [ ] **Step 4: Generate the session-context URL**

Add this pure helper in `crates/app/src/main.rs` and use it after the listener binds:

```rust
fn cockpit_url(
    address: std::net::SocketAddr,
    token: Option<&str>,
    issue: Option<std::num::NonZeroU64>,
) -> String {
    let mut query = Vec::new();
    if let Some(token) = token {
        query.push(format!("token={token}"));
    }
    if let Some(issue) = issue {
        query.push(format!("issue={issue}"));
    }
    let suffix = if query.is_empty() {
        String::new()
    } else {
        format!("?{}", query.join("&"))
    };
    format!("http://{address}/{suffix}")
}
```

Call it as:

```rust
let url = cockpit_url(address, session_token.as_deref(), args.issue);
```

- [ ] **Step 5: Verify GREEN**

```powershell
cargo.exe test -p stellr-app --locked
cargo.exe fmt --all -- --check
```

Expected: parser and live-server URL tests pass; formatting exits 0.

---

### Task 4: Preserve and pass browser session focus

**Files:**
- Modify: `web/src/lib/control.svelte.ts`
- Modify: `web/src/lib/control.test.ts`
- Modify: `web/src/App.svelte`
- Modify: `web/src/lib/StarMap.svelte`
- Modify: `web/src/lib/StarMap.test.ts`

**Interfaces:**
- Consumes: positive `issue=N` from `window.location.href`
- Produces: `pageIssue(): number | null` and `StarMap` prop `currentIssue: number | null`

- [ ] **Step 1: Write failing URL and wrapper tests**

Import `pageIssue` in `web/src/lib/control.test.ts`. Change the token test URL to:

```ts
'http://stellr.test:4173/chart?token=page-secret&issue=14&ignored=yes#s=o-r'
```

Assert:

```ts
expect(replaceState).toHaveBeenCalledWith(
  null,
  '',
  '/chart?issue=14&ignored=yes#s=o-r',
)
expect(pageIssue()).toBe(14)
```

Add a table test proving absent, zero, negative, fractional, nonnumeric, and
unsafe values return `null`. In `StarMap.test.ts`, spy on
`Renderer.prototype.setModel`, mount with `currentIssue: 14`, and assert the
third argument is `14`.

- [ ] **Step 2: Run focused tests and verify RED**

From `web`:

```powershell
vp run test -- src/lib/control.test.ts src/lib/StarMap.test.ts
```

Expected: failure because `pageIssue` and `currentIssue` do not exist.

- [ ] **Step 3: Parse the retained issue parameter**

Add to `control.svelte.ts`:

```ts
export function pageIssue(): number | null {
  const raw = new URL(window.location.href).searchParams.get('issue')
  if (raw === null || !/^\d+$/.test(raw)) return null
  const issue = Number(raw)
  return Number.isSafeInteger(issue) && issue > 0 ? issue : null
}
```

Do not remove `issue` in `takePageToken`; delete only `token`.

- [ ] **Step 4: Pass focus through the component seam**

In `App.svelte`:

```ts
import { Control, pageIssue, takePageToken } from './lib/control.svelte'
const currentIssue = pageIssue()
```

Render `<StarMap {space} {currentIssue} />`. In `StarMap.svelte`, add:

```ts
let {
  space,
  currentIssue = null,
  select,
}: {
  space: SpaceModel
  currentIssue?: number | null
  select?: (issueNumber: number) => void
} = $props()
```

Make the existing effect call:

```ts
renderer?.setModel(toRendererModel(space), {}, currentIssue)
```

- [ ] **Step 5: Verify GREEN**

From `web`:

```powershell
vp run test -- src/lib/control.test.ts src/lib/StarMap.test.ts
vp run check
```

Expected: URL, wrapper, and type checks pass with zero warnings.

---

### Task 5: Record and verify the complete native slice

**Files:**
- Modify: `CHANGELOG.md`
- Verify: all files changed by Tasks 1-4

**Interfaces:**
- Consumes: final web bundle and `stellr serve --issue 14`
- Produces: an uncommitted, fully verified Issue #14 worktree and live review URL

- [ ] **Step 1: Update Unreleased release notes**

Add under `## Unreleased`:

```markdown
- Refocused the star map on the current conversation issue and actionable
  `ready-for-agent` paths, with denser issue markers and no decorative starfield.
```

- [ ] **Step 2: Run complete frontend verification**

From `web`:

```powershell
vp run test
vp run check
vp run build
```

Expected: all tests pass, zero Svelte errors/warnings, and a production bundle
is emitted.

- [ ] **Step 3: Remove Vite+ metadata if generated**

Inspect `git diff -- web/package.json`. If Vite+ added only
`devEngines.packageManager`, remove that exact generated block with
`apply_patch`; do not revert any user-authored package change.

- [ ] **Step 4: Run complete native verification**

Stop only the verified `D:\tmp\stellr-issue-14\target\debug\stellr.exe` process
holding port 39420, then run:

```powershell
cargo.exe fmt --all -- --check
cargo.exe test --workspace --locked
cargo.exe clippy --workspace --all-targets --locked -- -D warnings
cargo.exe build --workspace --locked
```

Expected: formatting, all Rust tests, Clippy, and the workspace build exit 0.

- [ ] **Step 5: Restart and verify the embedded artifact**

Start hidden:

```powershell
Start-Process `
  -FilePath 'D:\tmp\stellr-issue-14\target\debug\stellr.exe' `
  -ArgumentList @('serve','--addr','127.0.0.1:39420','--issue','14') `
  -WindowStyle Hidden
```

Confirm the printed URL contains both `token` and `issue=14`. Verify root, JS,
and CSS return 200; bare `/api/model` returns 401; tokened `/api/model` returns
200; the scrubbed browser URL retains `issue=14`.

- [ ] **Step 6: Perform the real Simple Browser review**

Have the user open the refreshed URL and confirm:

- #14 reads as CURRENT;
- #8 reads as READY;
- `#8 -> #12 -> #14` is the dominant path;
- decorative background points are gone;
- issue nodes and labels are larger and compact;
- remaining issues stay visible but subdued.

- [ ] **Step 7: Inspect final scope**

```powershell
git status --short
git diff --check
git diff --stat
```

Report exact results. Keep the branch uncommitted and unpushed.

# Subissue Mini-Graphs Design

**Parent scope:** Issue #16 — M1 chrome

**Status:** Approved for implementation on 2026-08-02

## Goal

Render GitHub's native parent/subissue relationships as small directed workflow
graphs inside the existing Stellr map. A parent issue should visibly lead into
its available child work, sibling blockers should order that work, and terminal
children should lead back to their parent.

The feature must preserve the existing meanings of blocker dependencies,
CURRENT/READY focus, completed/incomplete node cores, deterministic layout, and
the black-background visual grammar.

## Domain Vocabulary

- **Parent issue:** the GitHub issue to which one or more native subissues are
  attached.
- **Child issue:** an issue whose native GitHub `parent` points to another issue
  in the same Stellr space.
- **Sibling:** another child of the same parent.
- **Root child:** a child with no blocker among its siblings.
- **Leaf child:** a child that blocks no sibling.
- **Entry edge:** a directed parent-to-root-child edge.
- **Sequence edge:** an existing blocker-to-blocked dependency edge between
  siblings.
- **Return edge:** a directed leaf-child-to-parent edge.
- **Traversed edge:** a workflow edge whose source is resolved and whose target
  is now frontier/ready.

These terms describe presentation topology. Native blocker data remains the
sole authority for readiness and status derivation.

## Chosen Approach

Add `parent_issue` to each issue and derive mini-graph topology in one pure web
module. Do not fold parent/subissue relationships into `blocked_by`.

GitHub exposes native parent and subissue data through its GraphQL schema and
CLI. Fetching `parent { number }` for each issue is sufficient because Stellr
already fetches every issue in a repository; it avoids a second paginated
subissue connection and makes each child record self-describing.

Alternatives rejected:

1. **Treat parent links as blockers.** This is smaller but wrong: it changes
   readiness semantics and injects cycles into dependency ranking.
2. **Add a graph-level relationship collection to `SpaceModel`.** This is more
   general, but duplicates relationships already expressible through each
   child's parent number and expands the server contract unnecessarily.

References:

- <https://docs.github.com/en/graphql/reference/issues>
- <https://docs.github.com/en/issues/tracking-your-work-with-issues/using-issues/browsing-sub-issues>

## Provider and Model Contract

The GitHub query adds:

```graphql
parent { number }
```

The value flows through these existing boundaries:

```text
GitHub IssueNode
  -> stellr_core::RawIssue.parent_issue
  -> stellr_core::Star.parent_issue
  -> cached snapshot / server JSON
  -> web Star.parent_issue
  -> renderer Ticket.parentIssue
```

Rust uses `Option<u64>` and deserializes an absent field as `None`. This keeps
older cached snapshots readable. Server JSON always emits either a number or
`null`, so the TypeScript contract is `number | null`.

The core derivation layer copies the relationship after rejecting a self-parent
reference. Status derivation continues to inspect only `blocked_by`, issue
state, and assignees.

## Mini-Graph Topology

A pure topology function receives renderer tickets and returns typed display
edges. For every parent present in the same space:

1. collect its children;
2. find sibling sequence edges by intersecting each child's `blockedBy` list
   with the child set;
3. add an entry edge from the parent to every root child;
4. retain the existing blocker-to-blocked sequence edges;
5. add a return edge from every leaf child to the parent.

Examples from the live Stellr tracker:

- Issues #34 and #35 are independent children of #14, so the graph contains
  #14→#34→#14 and #14→#35→#14.
- Issue #38 is blocked by sibling #37 under parent #16, so the graph contains
  #16→#37→#38→#16.

A lone child is both root and leaf, producing two opposite-direction curved
edges between it and its parent. If a native parent is absent from the current
space, the relationship is dropped. Duplicate `(from, to)` relationships are
merged into one display edge carrying all applicable roles.

Nested subissues work without a special case: an issue may be a child in one
mini-graph and a parent in another.

## Layout and Focus

Dependency ranking remains based only on `blocked_by`. Parent-entry and
child-return edges never participate in rank calculation, so their intentional
cycles cannot inflate ranks.

The deterministic physics layout uses the union of dependency and mini-graph
edges as springs. Its structure signature includes parent relationships and
edge roles, so changing hierarchy recomputes layout while status-only pushes
leave every star fixed.

CURRENT-to-READY focus traversal uses the directed display topology. This lets
a current parent point to its root work and lets completed sibling steps lead
to the next ready child. The traversal remains cycle-safe through its existing
visited-node discipline.

## Visual Grammar

All mini-graph edges are curved and directional with the existing enlarged,
high-contrast arrow geometry. Opposite-direction edges between the same pair
bend to opposite sides so neither is hidden.

An edge's state is:

- **Incomplete route:** the child associated with the relationship is not
  resolved. Draw a high-contrast violet dashed curve and violet arrow.
- **Completed route:** the associated child is resolved. Draw the existing
  solid resolved/mint edge style.
- **Traversed route:** the source is resolved and the destination is
  frontier/ready. Use the resolved/mint style plus the existing high-contrast
  motion treatment.

State precedence is traversed, then completed, then incomplete. In particular,
a resolved sibling leading to its newly ready sibling is traversed even though
the destination child is not yet complete.

For sequence edges, the destination is the associated child. For entry and
return edges, their shared child is the associated child.

Incomplete child nodes retain their existing hollow, status-colored core and
gain a subtle violet relationship rim. Resolved child nodes retain the existing
solid resolved style without the violet rim. CURRENT rings, READY emphasis, and
the selected/detail state remain visually dominant.

Dependency edges outside a subissue mini-graph keep their current visual
grammar unchanged.

## Error and Compatibility Behavior

- Missing `parent_issue` data means no hierarchy edge; the map otherwise works
  normally.
- Self-parent references are removed in core derivation.
- Parent references to issues outside the current space are ignored by the web
  topology function.
- Existing dependency edges are not duplicated when they also carry a
  mini-graph role.
- Failed GitHub refresh continues to expose the last cached hierarchy as stale,
  using the existing cached/offline behavior.
- No tracker mutation is performed by the viewer; GitHub remains authoritative.

## Testing Strategy

Provider tests will prove the GraphQL request and mapping of present/absent
parents across pagination. Core tests will prove serialization, old-cache
compatibility, self-parent rejection, and unchanged status derivation.

Pure web tests will prove:

- independent children: #14→#34→#14 and #14→#35→#14;
- sequential children: #16→#37→#38→#16;
- lone-child opposing edges;
- missing parents and duplicate-role merging;
- hierarchy cycles do not affect dependency rank;
- hierarchy changes affect the structure signature while status-only changes do
  not;
- focus traversal follows the directed mini-graph without looping.

Renderer seam tests will prove curve separation, arrow direction, violet dashed
incomplete styling, solid resolved styling, traversed motion, and node-rim
precedence. Existing dependency, CURRENT/READY, core, edge, and deterministic
layout suites remain green.

Full acceptance uses the live #14/#34/#35 and #16/#37/#38 relationships in the
embedded Windows browser and confirms zero console errors or warnings.

## Acceptance Criteria

- GitHub native parent/subissue relationships appear without textual fallbacks.
- Independent children form separate parent-child loops.
- Sibling blockers replace parallel parent loops with a directed sequence.
- Entry, sequence, and return arrows are all visible and correctly directed.
- Incomplete routes and child rims are violet and dashed/hollow as specified.
- Completed routes and children use the existing resolved grammar.
- Only traversed source-resolved-to-target-ready links animate.
- Parent cycles do not change blocker-derived readiness or dependency rank.
- Cached stale maps remain navigable with their last known hierarchy.
- All frontend/native checks and headed Windows browser acceptance pass.

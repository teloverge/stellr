# Markdown Relationship Line Restoration Design

**Date:** 2026-08-05  
**Status:** Approved

## Problem

Stellr draws every dependency and parent relationship it receives, but the
active `teloverge/encrydle` snapshot reaches the renderer with no relationships.
The issue bodies contain 17 dependency references under `## Blocked by` and 26
hierarchy references under `## Parent`. The GitHub fallback parser recognizes
only same-line forms such as `Blocked by #17`, so it ignores the references on
the following Markdown list lines. With an empty topology, the renderer has no
lines to draw.

## Goals

- Restore dependency lines described by `Blocked by` and `Blocks` sections.
- Restore hierarchy lines described by `Parent` sections when GitHub supplies
  no native parent relationship.
- Preserve native GitHub relationships and the existing same-line fallback.
- Use issue bodies already fetched in an ordinary snapshot or stored in the
  local cache; do not add a GitHub request.
- Keep the canvas renderer and its fixed spatial topology unchanged.

## Non-goals

- Mutating GitHub issues or recreating relationships in the tracker.
- Inferring relationships from arbitrary prose or task-list mentions.
- Changing line colors, geometry, animation, layout, or focus behavior.
- Fetching completed issue history or any additional provider page.

## Accepted Markdown Grammar

The relationship scanner continues to ignore fenced code blocks and existing
container prefixes. It additionally recognizes case-insensitive ATX headings
named exactly `Blocked by`, `Blocks`, or `Parent`, with an optional closing
heading marker.

Within a recognized section, same-repository issue references such as `#17`
are collected from subsequent content lines until the next ATX heading. Empty
lines and `None` text are harmless. Existing same-line forms such as
`Blocked by #17, #19` and `Blocks #20` remain supported.

Dependency references are sorted and deduplicated. A Markdown parent fallback
is accepted only when it names exactly one unique issue. If a native GitHub
parent exists, it wins. Multiple distinct Markdown parents are treated as
ambiguous and do not create a fallback parent relationship.

## Data Flow

Relationship enrichment remains in the GitHub adapter boundary:

1. Obtain the ordinary issue snapshot or load the existing local cache.
2. Scan each already-present issue body for inline and section relationships.
3. Union parsed `Blocked by` references with native blockers.
4. Invert parsed `Blocks` references onto their target issues.
5. Fill a missing native parent from an unambiguous parsed `Parent` reference.
6. Pass the enriched `RawIssue` values through the existing derivation,
   workflow-edge, layout, and canvas-rendering path.

The enrichment operation is deterministic and idempotent, so applying it to an
old cache or a new provider snapshot produces the same relationships without
duplicates. Existing caches can therefore recover their visible lines without
a special history import or additional GitHub endpoint.

## Failure Handling

- Unknown headings do not affect section state beyond ending the current
  recognized relationship section.
- References inside fenced code remain ignored.
- Missing target issues remain harmless; the existing workflow topology drops
  dangling edges from the visible map.
- Ambiguous Markdown parents do not override or invent parent authority.
- Malformed references are ignored without failing the entire snapshot.

## Testing

Tests will be written before production changes and will cover:

- Encrydle-style `## Blocked by` lists producing dependency references;
- `## Blocks` lists producing inverted dependency relationships;
- `## Parent` followed by one reference producing a fallback parent;
- native parent precedence over the Markdown fallback;
- multiple Markdown parents remaining ambiguous;
- section termination at the next heading;
- fenced examples remaining excluded;
- deduplication across native, inline, and section references;
- enrichment of a cache-shaped `RawIssue` collection without provider access;
- the existing renderer receiving and painting the recovered workflow edges.

Verification uses the focused Rust and frontend suites first, followed by the
native Windows formatting, strict Clippy, complete workspace tests, frontend
check/tests/build, and the Windows-only NSIS development build when preparing
the local installer.

## Acceptance Criteria

- The current Encrydle issue-body format yields its dependency and parent
  topology from already-available data.
- Connected nodes receive visible lines through the unchanged renderer.
- Existing inline and native relationship behavior remains green.
- No new GitHub API or GraphQL request is introduced.
- The checkout and Windows build remain reproducible with native toolchains.

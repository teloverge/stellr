# GitHub GraphQL issue dependencies

**Verified:** 2026-07-30 against GitHub's live GraphQL API

**Decision:** M1 will read native blocker identities from `Issue.blockedBy` and
merge them with the textual fallback scanner.

## Live schema result

An authenticated introspection of `__type(name: "Issue")` returned these
dependency fields:

| Field | GraphQL type |
| --- | --- |
| `blockedBy` | `IssueConnection!` |
| `blocking` | `IssueConnection!` |
| `issueDependenciesSummary` | `IssueDependenciesSummary!` |

`blockedBy` and `blocking` accept the standard connection arguments `after`,
`before`, `first`, and `last`, plus `orderBy`. The connection exposes `nodes`,
`edges`, `pageInfo`, and `totalCount`.

GitHub's current schema reference documents the same fields and shapes:

- [`Issue.blockedBy` and `Issue.blocking`](https://docs.github.com/en/graphql/reference/issues#issue)
- [`IssueConnection`](https://docs.github.com/en/graphql/reference/issues#issueconnection)
- [`IssueDependenciesSummary`](https://docs.github.com/en/graphql/reference/issues#issuedependenciessummary)

No schema-preview header was required.

## Live repository smoke result

The probe queried `teloverge/stellr` Issue #7, which has native dependency
relationships. GitHub returned:

```json
{
  "number": 7,
  "blockedBy": {
    "totalCount": 4,
    "nodes": [
      { "number": 6, "state": "OPEN" },
      { "number": 5, "state": "OPEN" },
      { "number": 4, "state": "OPEN" },
      { "number": 2, "state": "CLOSED" }
    ]
  },
  "blocking": {
    "totalCount": 1,
    "nodes": [
      { "number": 8, "state": "OPEN" }
    ]
  },
  "issueDependenciesSummary": {
    "blockedBy": 3,
    "blocking": 1,
    "totalBlockedBy": 4,
    "totalBlocking": 1
  }
}
```

This also demonstrates the summary semantics:
`issueDependenciesSummary.blockedBy` reports the open blockers, while
`issueDependenciesSummary.totalBlockedBy` includes open and closed blockers.
The summary is useful for a quick gate, but it cannot provide the issue numbers
needed for graph edges.

GitHub announced issue dependencies as generally available with GraphQL API
support and a maximum of 50 relationships in each direction:
[Dependencies on issues](https://github.blog/changelog/2025-08-21-dependencies-on-issues/).

## Decision for GraphQL sync

Task 7 must:

1. Select `blockedBy(first: 50) { nodes { number } }` for every fetched issue.
   GitHub's documented relationship limit makes that page size exhaustive.
2. Set `RawIssue.blocked_by` to the sorted, deduplicated union of those native
   issue numbers and the textual `Blocked by #N` references.
3. Continue inverting textual `Blocks #N` references onto the target issue
   after all issue pages load.
4. Keep the merge at the GitHub-provider boundary so `stellr-core` remains
   independent of how an edge was discovered.

The sync query does not need `blocking` or `issueDependenciesSummary` to build
the graph: querying each issue's `blockedBy` connection supplies the canonical
edge direction and identities directly.

## Windows-safe reproduction

Set `GITHUB_TOKEN` in the current process without printing it, then pass the
GraphQL document as JSON on standard input. This avoids native Windows argument
quoting stripping quotes inside a query passed with `-f query=...`.

```powershell
$query = @'
query {
  __type(name: "Issue") {
    fields {
      name
      type { kind name ofType { kind name } }
      args { name type { kind name ofType { kind name } } }
    }
  }
}
'@

@{ query = $query } |
  ConvertTo-Json -Compress |
  gh api graphql --input -
```

Repository smoke query:

```graphql
query {
  repository(owner: "teloverge", name: "stellr") {
    issue(number: 7) {
      number
      blockedBy(first: 10) {
        totalCount
        nodes { number state title }
      }
      blocking(first: 10) {
        totalCount
        nodes { number state title }
      }
      issueDependenciesSummary {
        blockedBy
        blocking
        totalBlockedBy
        totalBlocking
      }
    }
  }
}
```

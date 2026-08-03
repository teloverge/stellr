# Live release-story source evidence

**Issue:** #49

**Repository:** `teloverge/stellr`

**Milestone:** `M1 — the chart`

The showcase-specific `ReleaseHistorySource` remains separate from Stellr's
runtime `Provider`. Both use the same authenticated `GithubGraphqlClient`, so
authentication rejection, rate limiting, transport failure, GraphQL errors,
and malformed JSON retain the established typed provider behavior.

## Recorded contract evidence

- A mocked live release resolves its milestone and previous release, follows
  issue and lifecycle pagination, reconstructs the same `ReleaseStory` shape as
  a recorded fixture, and records every relevant provider event ID.
- A first release uses an explicit UTC starting cutoff without querying a
  previous release.
- A missing next-page cursor fails as partial history rather than generating a
  story from the first page.
- HTTP 401, exhausted rate limit, and malformed JSON remain distinct typed
  failures.
- The normalized manifest contains no token, response-header, issue-body,
  unrelated-label, or local-path fields.

## Live read-only evidence

GitHub accepted the exact milestone, release, issue, blocker, and filtered
lifecycle query fields used by the source. At validation time the repository
reported 4 milestones and 38 issues; issue #47 exposed the expected blocker and
timeline connection shapes.

The ignored live smoke test was then run explicitly with:

```powershell
cargo.exe test -p stellr-showcase --test github_release_source `
  live_stellr_m1_builds_through_the_public_source_seam --locked -- `
  --ignored --nocapture
```

It built a non-empty M1 story and beat sequence through
`GithubReleaseHistorySource` using the explicit window
`2026-07-31T00:00:00Z` through `2026-08-02T22:35:00Z`. The operation was
read-only and wrote no preview, tracked artifact, or tracker mutation.

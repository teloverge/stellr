# Publish the M1 Release Constellation

**Issue:** #54

**Branch:** `codex/issues-47-54-readme-showcase`

**Environment:** Native Windows 11 and PowerShell

## Frozen release identity

- Release artifact version: `m1`
- Milestone: `M1 — the chart`
- Bootstrap cutoff: `2026-07-31T00:00:00Z`
- Ending cutoff: `2026-08-03T00:44:50Z`

The bootstrap precedes the milestone's first issue. The ending cutoff is the
already-reviewed frozen live-evidence boundary used by issue #52 and follows
the final M1 issue closure.

## Tasks

- [x] Generate a live preview for the exact frozen boundary and record its
  domain-separated review digest, file hashes, story counts, visible issues,
  hidden support, topology, and evidence-backed beats.
- [x] Inspect the local review page at full and narrow widths, verify the
  reduced-motion final state, the strict-Markdown PNG path, and Firefox's
  fallback/degradation behavior.
- [x] Accept only the exact reviewed digest, publish immutable `m1` SVG, PNG,
  and story JSON files, and confirm the README points to those exact assets.
- [ ] Record the release evidence in `docs/validation`, update the Unreleased
  changelog, and run the native Windows formatting, lint, workspace-test, and
  release-showcase gates.
- [ ] Commit and push the publication, inspect the rendered GitHub branch README
  at desktop and narrow widths, obtain final spec and standards reviews, then
  close #54 only when every acceptance criterion has current evidence.

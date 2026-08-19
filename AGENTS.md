# Repository instructions

- Keep Windows and macOS packaging checks platform-specific. Do not replace
  their native CI runners with WSL or cross-compilation for release evidence.
- Maintain release notes as an append-only, newest-first changelog with a
  separate section for every shipped version and an `Unreleased` section for
  pending work.
- Never fold new changes into an older release section or repeat cumulative
  changes under every version.

## Agent skills

### Issue tracker

Issues and PRDs are tracked in this repository's GitHub Issues. See `docs/agents/issue-tracker.md`.

### Triage labels

Use the default five-role triage vocabulary. See `docs/agents/triage-labels.md`.

### Domain docs

Use the single-context domain-documentation layout. See `docs/agents/domain.md`.

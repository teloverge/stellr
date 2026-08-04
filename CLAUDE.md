# Repository instructions

Assume a native Windows 11 development environment.

- Do not use WSL, Linux shells, Linux toolchains, or `/mnt/*` paths for development, builds, tests, benchmarks, or validation.
- Run commands with native Windows PowerShell or `cmd.exe` from Windows paths such as `D:\dev`.
- Use native Windows executables and toolchains (`cargo.exe`, `rustc.exe`, `bun.exe`, and `powershell.exe`).
- If native Windows command execution is unavailable, stop and report the environment limitation instead of substituting WSL results.
- Maintain release notes as an append-only, newest-first changelog with a separate section for every shipped version and an `Unreleased` section for pending work.
- Never fold new changes into an older release section or repeat cumulative changes under every version.

## Agent skills

### Issue tracker

Issues and PRDs are tracked in this repository's GitHub Issues. See `docs/agents/issue-tracker.md`.

### Triage labels

Use the default five-role triage vocabulary. See `docs/agents/triage-labels.md`.

### Domain docs

Use the single-context domain-documentation layout. See `docs/agents/domain.md`.

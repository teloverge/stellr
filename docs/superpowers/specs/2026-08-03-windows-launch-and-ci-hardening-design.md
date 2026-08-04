# Windows Launch and CI Hardening Design

**Date:** 2026-08-03
**Status:** Approved for implementation

## Problem

The packaged Windows executable is currently linked as a console application
(`WINDOWS_CUI`). Launching it from the Start menu or an installed shortcut
therefore creates a terminal window before the native Stellr window appears.
That terminal is an implementation detail and should not be visible during an
ordinary desktop launch.

Stellr is also a real command-line application. The same executable provides
`serve`, help, version, and error output. Switching the executable to the
Windows GUI subsystem without preserving those console paths would fix the
desktop symptom by silently breaking supported CLI behavior.

The Windows application-process smoke test has a separate reliability gap. It
uses a fixed 30-second polling loop and reports only that no window appeared.
The same commit passed when the failed GitHub Actions job was rerun, with the
desktop step taking 24 seconds overall. Stellr already emits opt-in startup
stage diagnostics, but this smoke does not capture them.

Finally, all six workflow upload steps still use `actions/upload-artifact@v4`.
GitHub's Node 24 runner transition warns because that action line uses an older
internal Node runtime. Stellr's own JavaScript jobs already select Node 24; the
remaining warning is action dependency debt, not a project Node-version
setting to suppress.

## Product Decision

Release builds of `stellr.exe` are Windows GUI-subsystem applications. Normal
desktop launches do not allocate or display a console window. Debug builds
remain console-subsystem applications so developers retain ordinary terminal
diagnostics.

Release invocations that intentionally use the CLI attach to their parent
console before argument parsing or output. This preserves one installed
executable and the existing `stellr serve` command. If parent-console
attachment cannot preserve native PowerShell and `cmd.exe` behavior, the
implementation must stop and revisit the executable boundary; it must not ship
a quiet or detached `serve` command.

Windows startup smoke tests allow a measured 90-second cold-start budget and
capture existing startup diagnostics on failure. A larger legitimate startup
budget is not a success fallback: the test still fails if the window does not
appear or the process exits.

All six workflow references to `actions/upload-artifact@v4` move to the current
supported `v7` line. The Node warning is fixed by upgrading its source rather
than hiding, filtering, or ignoring it.

## Goals

- Launch the installed desktop application without a visible terminal window.
- Preserve interactive release CLI behavior from native PowerShell and
  `cmd.exe`.
- Keep debug-build console behavior unchanged.
- Make Windows startup failures report the last known native startup stage.
- Tolerate legitimate cold-run variance without accepting a missing window.
- Remove the artifact action's obsolete runtime warning by upgrading the
  action dependency.
- Lock these properties down at source, binary, script, and GitHub Actions
  seams.

## Non-Goals

- Remove consoles from debug builds.
- Hide a console after it flashes; release desktop launch must avoid creating
  it in the first place.
- Silence, filter, or waive GitHub Actions runtime warnings.
- Change desktop navigation, repository selection, authentication, or server
  semantics.
- Split Stellr into separate GUI and CLI products unless console attachment is
  proven inadequate during implementation.
- Change installer signing or release-signing policy.

## Design

### Release subsystem

The app crate declares the Windows GUI subsystem only when compiling a release
build for Windows. Conceptually, the crate-level contract is equivalent to:

```rust
#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]
```

The target predicate keeps non-Windows builds untouched. The debug predicate
keeps local `cargo run` and test diagnostics attached to the developer's
console. The final packaged executable must have PE subsystem
`IMAGE_SUBSYSTEM_WINDOWS_GUI` (value 2), verified from the built binary rather
than inferred from source text.

### Launch classification and console attachment

Launch classification happens from raw process arguments before Clap parses
them, because Clap may print help, version, or parse errors and terminate. The
classifier has only two outcomes:

- **desktop-oriented:** no arguments, `open ...`, or a direct `stellr://...`
  activation;
- **console-oriented:** `serve ...`, `--help`, `-h`, `--version`, `-V`, or an
  unrecognized/invalid command whose error belongs in the invoking console.

On Windows release builds, a console-oriented launch attempts to attach to the
parent process's console using `AttachConsole(ATTACH_PARENT_PROCESS)` before
any parser or application output. Standard output and standard error are then
made usable against that attached console when Windows did not establish
usable standard handles automatically. The logic is Windows-specific and is a
no-op for debug and non-Windows builds.

Failure to attach must produce a testable non-zero CLI failure rather than
pretend the command succeeded without visible output. Desktop-oriented launch
does not attach, allocate, show, or later hide a console.

The implementation must explicitly validate native shell behavior. A GUI
subsystem process has different shell waiting semantics from a console
subsystem process, so acceptance is not satisfied merely because bytes can be
redirected from a child process. From PowerShell and `cmd.exe`, the release
`serve` command must print its normal startup information, remain controllable,
and return an observable exit status. If the single-executable approach cannot
meet that behavior reliably, implementation pauses for a revised product
decision, with a separate console entry-point as the likely fallback.

### Windows startup smoke diagnostics

Both Windows smoke scripts replace the fixed `60 * 500 ms` polling loop with a
named `StartupTimeoutSeconds` parameter that defaults to 90 seconds. A
stopwatch/deadline controls the loop so the configured budget is clear and
does not depend on a magic attempt count.

For each spawned Stellr process, the script:

1. enables `STELLR_STARTUP_DIAGNOSTICS=1` only for the child launch and restores
   the caller's environment afterward;
2. redirects standard output and standard error to per-launch temporary logs;
3. waits until the real desktop window is visible, the process exits, or the
   deadline expires;
4. on failure, reports the exit state and captured diagnostic lines, including
   the last `STELLR_DESKTOP_STARTUP_STAGE` or
   `STELLR_DESKTOP_STARTUP_ERROR` marker;
5. still exits non-zero when the window never becomes visible.

The installed-application smoke keeps its existing installer and process
boundary assertions. The application-process smoke remains a real packaged
release launch, not a mocked window test.

### GitHub Actions dependency upgrade

Every current upload site moves together from
`actions/upload-artifact@v4` to `actions/upload-artifact@v7`:

- Linux bundle workflow;
- macOS bundle workflow;
- Windows bundle workflow;
- Windows, macOS, and Linux jobs in the release workflow.

Artifact names, paths, retention behavior, and downstream release consumption
remain unchanged. Workflow contract tests prohibit old `v4` references and
require the six expected `v7` references. GitHub Actions is then run to prove
the upgraded action uploads each platform artifact successfully and the Node
runtime warning is absent.

## Testing Strategy

Implementation follows red-green-refactor.

1. Add focused Rust tests for raw launch classification: bare/open/protocol
   inputs are desktop-oriented; serve/help/version/invalid inputs are
   console-oriented.
2. Add Windows-specific tests around the console-preparation seam without
   attaching the test runner itself to a different console.
3. Add or extend script contract tests to require the 90-second named timeout,
   deadline-based polling, child-scoped startup diagnostics, failure log
   emission, and non-zero timeout behavior.
4. Add a native PowerShell PE-header assertion for the release executable and
   call it after the Windows release build. It must initially observe the
   current `WINDOWS_CUI` value and pass only after the subsystem change produces
   `WINDOWS_GUI`.
5. Add workflow contract coverage requiring exactly six
   `actions/upload-artifact@v7` uses and no `@v4` uses.
6. Run the focused native tests, full Rust test suite, frontend checks, and
   Windows release build locally as appropriate.
7. Exercise the built release CLI from native PowerShell and `cmd.exe`, proving
   visible `serve`/help output, controllability, and exit behavior.
8. Let GitHub's Windows application-process and installed-application smokes
   prove the actual desktop window boundary. All platform bundle uploads must
   also pass with the upgraded action.

Tests must avoid asserting implementation text when they can inspect behavior
or artifacts. The subsystem gate reads the actual PE header; the smoke gate
observes an actual top-level Stellr window; the CLI gate launches the actual
release executable.

## Failure Handling

- A desktop process that exits before showing its window fails immediately and
  prints its captured startup diagnostics.
- A desktop process that remains alive without a visible window for 90 seconds
  fails with its last recorded startup stage.
- A release CLI process that cannot attach to the invoking console does not
  silently continue as if output were available.
- An artifact upload regression fails its owning workflow; no warning filter or
  `continue-on-error` is introduced.
- If `upload-artifact@v7` requires a real input or consumption change, update
  that workflow explicitly and test it rather than pinning back to silence the
  warning.

## Documentation and Release Notes

The `Unreleased` changelog records the console-free installed desktop launch,
the diagnostic startup-smoke hardening, and the artifact action upgrade.
Existing shipped version sections remain unchanged and are not rewritten or
made cumulative.

## Acceptance Criteria

- Launching installed Stellr normally creates the native window without a
  terminal window appearing or remaining visible.
- The packaged release PE subsystem is `IMAGE_SUBSYSTEM_WINDOWS_GUI`.
- Debug builds retain their ordinary console subsystem and diagnostics.
- Release `stellr serve`, help, version, and invalid-command paths remain
  visible and usable from native PowerShell and `cmd.exe`.
- Bare, `open`, and protocol desktop launches never attach or allocate a
  console.
- Both Windows startup smokes use a named 90-second deadline and fail with the
  captured startup stage when the real window does not appear.
- All six artifact upload steps use `actions/upload-artifact@v7`; no `@v4`
  upload references or warning-suppression mechanisms remain.
- GitHub Actions uploads the expected Linux, macOS, and Windows artifacts
  successfully without the obsolete action-runtime warning.
- Focused tests, the full native test matrix, frontend checks, release build,
  packaged process smokes, and workflow checks pass.

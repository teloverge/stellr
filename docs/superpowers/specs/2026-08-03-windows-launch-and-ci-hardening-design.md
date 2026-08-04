# Windows Launch and CI Hardening Design

**Date:** 2026-08-03
**Status:** Approved for implementation
**Amended:** 2026-08-04 - approved separate desktop and CLI entry points after
confirming Windows shell wait semantics are fixed by the PE subsystem.

## Problem

The packaged Windows executable is currently linked as a console application
(`WINDOWS_CUI`). Launching it from the Start menu or an installed shortcut
therefore creates a terminal window before the native Stellr window appears.
That terminal is an implementation detail and should not be visible during an
ordinary desktop launch.

Stellr is also a real command-line application. The current executable
provides `serve`, help, version, and error output. Switching that executable to
the Windows GUI subsystem would fix the desktop symptom by silently breaking
supported CLI behavior: interactive `cmd.exe` does not wait for a GUI-subsystem
process, and attaching that process to the parent console cannot change the
shell's wait and exit-code decision.

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

Windows packages contain two thin entry points backed by the same application
assembly:

- `stellr-desktop.exe` is the packaged GUI entry point. Release builds use the
  Windows GUI subsystem, and the installer shortcut, display icon, and `stellr`
  protocol registration target it.
- `stellr.exe` remains the console-subsystem CLI entry point. It preserves
  `serve`, help, version, invalid-command output, native shell waiting, and exit
  codes from PowerShell and `cmd.exe`.

Debug builds of both entry points remain console-subsystem applications so
developers retain ordinary terminal diagnostics. The Windows installer
includes the CLI beside the desktop executable; it does not duplicate runtime
or domain logic between them.

Windows startup smoke tests allow a measured 90-second cold-start budget and
capture existing startup diagnostics on failure. A larger legitimate startup
budget is not a success fallback: the test still fails if the window does not
appear or the process exits.

All six workflow references to `actions/upload-artifact@v4` move to the current
supported `v7` line. The Node warning is fixed by upgrading its source rather
than hiding, filtering, or ignoring it.

## Goals

- Launch the installed desktop application without a visible terminal window.
- Preserve the installed `stellr` CLI and its interactive behavior from native
  PowerShell and `cmd.exe`.
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
- Split the desktop and CLI into separate products or duplicated application
  implementations; they remain two entry points into one Stellr product and
  shared assembly.
- Change installer signing or release-signing policy.

## Design

### Executable boundary and release subsystem

The app crate exposes a desktop binary in addition to the existing CLI binary.
Only the desktop entry point declares the Windows GUI subsystem, and only when
compiling a release build for Windows. Conceptually, its crate-level contract
is equivalent to:

```rust
#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]
```

The target predicate keeps non-Windows builds untouched. The debug predicate
keeps local desktop-entry diagnostics attached to the developer's console. The
final packaged desktop executable must have PE subsystem
`IMAGE_SUBSYSTEM_WINDOWS_GUI` (value 2), while the CLI executable must retain
`IMAGE_SUBSYSTEM_WINDOWS_CUI` (value 3). Both values are verified from the
built binaries rather than inferred from source text.

### Shared launch assembly

The desktop and CLI entry points are dispatch adapters, not separate
applications. Existing command parsing, working-directory resolution, desktop
launch construction, runtime startup, and error behavior move behind shared
application interfaces where both entry points need them.

The desktop entry point accepts the launch shapes supplied by Windows desktop
integration: bare startup, `open <target>`, and a direct `stellr://...`
activation. It never exposes `serve`, help, version, or general parser errors
through a nonexistent console. Controlled desktop startup failures continue to
use the existing native error dialog.

The console entry point owns the complete CLI grammar. `serve`, help, version,
and invalid-command paths remain ordinary console operations. A CLI `open`
invocation may host the desktop in the console process the user deliberately
started; it does not create an additional terminal window.

On Windows, Tauri bundles `stellr-desktop.exe` as the main application binary
and includes `stellr.exe` as the companion CLI. The Start menu shortcut,
uninstall metadata display icon, and deep-link registration resolve to the
desktop binary. The installed CLI remains independently launchable for users
who intentionally choose the command line.

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

1. Add focused Rust tests for shared desktop launch construction and the
   desktop entry point's bare/open/protocol argument shapes.
2. Add Windows packaging tests that inspect the actual release PE headers,
   require GUI subsystem value 2 for `stellr-desktop.exe`, and require console
   subsystem value 3 for `stellr.exe`.
3. Add an installed-package contract proving the shortcut/display icon and
   protocol activation use the desktop binary while the CLI is also installed.
4. Add or extend script contract tests to require the 90-second named timeout,
   deadline-based polling, child-scoped startup diagnostics, failure log
   emission, and non-zero timeout behavior.
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
- A missing or wrongly linked companion CLI fails the packaging contract; the
  desktop executable is never used as a quiet CLI fallback.
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
- The packaged desktop PE subsystem is `IMAGE_SUBSYSTEM_WINDOWS_GUI`, and the
  companion CLI subsystem is `IMAGE_SUBSYSTEM_WINDOWS_CUI`.
- Debug builds retain their ordinary console subsystem and diagnostics.
- Release `stellr serve`, help, version, and invalid-command paths remain
  visible and usable from native PowerShell and `cmd.exe` through the companion
  CLI.
- The installed shortcut and protocol registration launch
  `stellr-desktop.exe`; bare, `open`, and protocol desktop launches never
  allocate a console.
- The Windows package installs both entry points without duplicating runtime or
  domain logic.
- Both Windows startup smokes use a named 90-second deadline and fail with the
  captured startup stage when the real window does not appear.
- All six artifact upload steps use `actions/upload-artifact@v7`; no `@v4`
  upload references or warning-suppression mechanisms remain.
- GitHub Actions uploads the expected Linux, macOS, and Windows artifacts
  successfully without the obsolete action-runtime warning.
- Focused tests, the full native test matrix, frontend checks, release build,
  packaged process smokes, and workflow checks pass.

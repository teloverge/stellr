# Empty Desktop Startup Shell Design

**Date:** 2026-08-03
**Status:** Approved for implementation

## Problem

The packaged Stellr shortcut launches `stellr.exe` without arguments from the
installation directory. Bare startup currently treats that working directory
as an implicit repository target. Because an installation directory is not a
Git repository, Stellr validates it before creating the native window and
exits with an `invalid repository path` dialog.

This behavior prevents the existing empty-state interface from appearing. It
also makes ordinary desktop startup depend on an unrelated process working
directory.

## Product Decision

Bare desktop startup opens Stellr without an implicit repository. The runtime
loads any persisted spaces and presents the existing empty shell when none are
available. The user can then add either a local repository path, including via
the native directory chooser, or a GitHub `owner/repo` identifier.

Explicit targets remain explicit. `stellr open <target>` and `stellr://` deep
links continue to validate and route their supplied target. Browser `serve`
behavior is unchanged.

## Goals

- Make a normal installed shortcut launch independent of its working
  directory.
- Show the existing `No spaces yet` interface when no spaces are persisted.
- Preserve saved spaces across restarts without inventing an initial space.
- Preserve explicit target validation and routing.
- Lock the behavior down at launch-intent, runtime, frontend, and packaged
  Windows seams.

## Non-Goals

- Automatically open the Windows directory chooser on startup.
- Change the repository-entry controls or visual design.
- Change authentication, polling, deep-link, or browser-server behavior.
- Add a sentinel or synthetic repository to represent the empty state.

## Design

### Launch intent

Desktop launch distinguishes between an optional initial target and the
process working directory. A bare invocation carries no initial target.
`open` and protocol activation carry a target resolved relative to the launch
directory where appropriate.

The working directory remains available only for resolving an explicitly
relative target. It is never itself promoted to a target during bare startup.

### Desktop runtime

Runtime startup accepts an optional initial space:

- when an initial target is present, resolve it, add it if necessary, persist
  it, and route to it;
- when no initial target is present, load the existing space store without
  adding or validating a repository;
- when the store is empty, publish an authoritative empty model to the
  frontend;
- when the store contains spaces, retain the existing restore and default
  selection behavior.

No synthetic space or placeholder repository is written to disk.

### Frontend

The frontend requires no new screen. Its existing authoritative empty-model
state displays `No spaces yet` together with repository-entry controls in the
sidebar. The local-path field, native Browse action, and GitHub repository field
remain the ways to create the first space.

### Error handling

Bare startup cannot fail with a repository-path error because it performs no
repository validation. Failures loading malformed persisted state retain the
store's existing recovery behavior.

Explicit invalid targets continue to report a clear error. This preserves
feedback for commands and deep links where the user actually supplied a
repository target.

## Testing

Implementation follows red-green-refactor:

1. A launch-intent test proves that a no-argument invocation produces no
   initial target while explicit `open` remains targeted.
2. A desktop-runtime test starts with an empty space store and a non-repository
   working directory, then proves the runtime publishes zero spaces without
   persisting a synthetic entry.
3. Existing explicit-target runtime and routing tests remain green.
4. The frontend empty-model test continues to prove that `No spaces yet` is
   visible after an authoritative empty snapshot.
5. The Windows packaged-application smoke launches the installed shortcut
   shape with no arguments and proves the app window becomes visible without
   an invalid-repository dialog.

## Documentation and Release Notes

The M2 native-shell acceptance criterion changes from “bare `stellr` opens the
native shell on the current repository” to “bare `stellr` opens the native
shell without requiring a repository.” The `Unreleased` changelog records the
installed-startup correction; shipped release sections remain unchanged.

## Acceptance Criteria

- Launching Stellr from its installed shortcut with no arguments opens the
  native window.
- An empty installation displays `No spaces yet` and exposes repository-entry
  controls.
- The shortcut working directory is not treated as a repository.
- Persisted spaces still load on restart.
- Explicit `open` and protocol targets still validate and route normally.
- `serve` behavior is unchanged.
- Focused regression tests, the full native test matrix, frontend checks, and
  the Windows installer build and smoke pass.

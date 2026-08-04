# Empty Startup Shell Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make a no-argument desktop launch open Stellr's existing empty shell without treating the process working directory as a repository, while preserving explicit targets and packaged Windows behavior.

**Architecture:** Represent the initial desktop target as optional from CLI dispatch through Tauri startup. Start the shared runtime with an optional initial `SpaceEntry`, restoring persisted route state when available and otherwise allowing the server to publish an empty model. Prove the real installed launch shape by starting the bundled executable from its non-repository installation directory and requiring the main window instead of the old startup error; focused runtime and frontend tests prove the authoritative empty model and its UI.

**Tech Stack:** Rust 2024, Tauri 2, Tokio, Axum runtime, Svelte 5, Vitest, native Windows PowerShell, NSIS, UI Automation.

## Global Constraints

- Use native Windows 11 PowerShell and native Windows executables only; do not use WSL or Linux toolchains.
- A bare launch opens the existing empty shell; it does not force the Windows directory chooser.
- Explicit `open` and `stellr://` targets continue to validate and route normally.
- `serve` behavior is unchanged.
- Do not write a synthetic or placeholder repository to the space store.
- Keep release notes append-only and newest-first; add pending work only under `Unreleased`.
- Preserve the existing frontend repository-entry controls and visual design.

---

### Task 1: Start the desktop runtime without an initial repository

**Files:**
- Modify: `crates/app/src/desktop.rs`
- Modify: `crates/app/tests/desktop_runtime_test.rs`

**Interfaces:**
- Consumes: `DesktopRuntimeOptions`, `SpaceStore`, `SpaceEntry`, `ApplicationRuntime`, and the existing provider boundary.
- Produces: `start_runtime(options, provider) -> Result<ApplicationRuntime, DesktopRuntimeError>` for an empty/persisted store, while `start_runtime_with_entry(options, entry, provider, polling)` retains explicit-entry behavior.

- [ ] **Step 1: Write the failing empty-runtime regression test**

Add a test that passes a real non-repository directory and an empty space-store path to `start_runtime`, then asserts the returned runtime contains zero spaces and no space-store file was created:

```rust
#[tokio::test]
async fn desktop_runtime_starts_without_treating_the_working_directory_as_a_repository() {
    let profile = tempfile::tempdir().unwrap();
    let working_directory = profile.path().join("installed-app");
    std::fs::create_dir(&working_directory).unwrap();
    let spaces_file = profile.path().join("spaces.toml");

    let runtime = start_runtime(
        DesktopRuntimeOptions {
            current_dir: working_directory,
            spaces_file: spaces_file.clone(),
            cache_root: profile.path().join("cache"),
        },
        Arc::new(EmptyProvider),
    )
    .await
    .unwrap();

    assert!(runtime.state().spaces.lock().await.entries().is_empty());
    assert!(!spaces_file.exists());
    runtime.shutdown_handle().shutdown();
    runtime.wait().await.unwrap();
}
```

Keep the existing explicit-repository test, changing it to call `start_runtime_with_entry` with the detected real entry so it continues to prove add-and-persist behavior.

- [ ] **Step 2: Run the focused test and verify the exact failure**

Run:

```powershell
cargo.exe test -p stellr-app --test desktop_runtime_test desktop_runtime_starts_without_treating_the_working_directory_as_a_repository -- --exact --nocapture
```

Expected: FAIL because the current `start_runtime` calls `detect_repo` on the non-repository working directory and returns `CurrentRepository("not a git repo")`.

- [ ] **Step 3: Implement optional initial-entry runtime assembly**

Refactor runtime assembly so the shared internal function accepts `Option<SpaceEntry>`:

```rust
async fn start_runtime_with_initial_entry(
    options: DesktopRuntimeOptions,
    entry: Option<SpaceEntry>,
    provider: Arc<dyn Provider + Send + Sync>,
    polling: Option<PollingControl>,
) -> Result<ApplicationRuntime, DesktopRuntimeError> {
    let mut spaces = SpaceStore::load(options.spaces_file.clone());
    if let Some(entry) = entry
        && !spaces.entries().iter().any(|existing| existing.id == entry.id)
    {
        spaces.add(entry).map_err(DesktopRuntimeError::CurrentRepository)?;
        spaces.save().map_err(DesktopRuntimeError::SaveSpace)?;
    }
    // Build the existing RuntimeOptions and start the runtime unchanged.
}
```

Make `start_runtime` call this helper with `None`. Keep `start_runtime_with_entry` as the explicit-entry wrapper with `Some(entry)` so the acceptance harness and explicit-target paths retain their contract.

- [ ] **Step 4: Run both desktop-runtime tests and verify green**

Run:

```powershell
cargo.exe test -p stellr-app --test desktop_runtime_test -- --nocapture
```

Expected: both the empty-start and explicit-repository persistence tests PASS.

- [ ] **Step 5: Commit the runtime slice**

```powershell
git add crates/app/src/desktop.rs crates/app/tests/desktop_runtime_test.rs
git commit -m "fix: allow desktop runtime without a repository"
```

### Task 2: Carry an optional target through desktop launch and single-instance routing

**Files:**
- Modify: `crates/app/src/main.rs`
- Modify: `crates/app/src/desktop.rs`

**Interfaces:**
- Consumes: parsed `Cli`, `DesktopLaunch`, `TargetResolver`, `PersistedRoute`, and `RouteInbox`.
- Produces: `DesktopLaunch { cwd, target: Option<String>, restore_route }`; `forwarded_route_event(args, cwd) -> Option<NativeRouteEvent>`; optional startup route construction.

- [ ] **Step 1: Write failing launch and forwarding tests**

Change the bare-launch test to build the desktop launch intent and assert `target == None`. Add a desktop forwarding test that proves a second no-argument invocation returns no route event:

```rust
#[test]
fn bare_second_instance_only_reveals_the_existing_window() {
    assert!(forwarded_route_event(&["stellr.exe".into()], r"D:\Apps\Stellr").is_none());
}
```

Retain the existing explicit `open` and protocol forwarding assertions by unwrapping their `Some(NativeRouteEvent::Target { .. })` values.

- [ ] **Step 2: Run the focused tests and verify red**

Run:

```powershell
cargo.exe test -p stellr-app bare_launch -- --nocapture
cargo.exe test -p stellr-app bare_second_instance_only_reveals_the_existing_window -- --exact --nocapture
```

Expected: FAIL because `DesktopLaunch.target` is required and a bare forwarded invocation currently resolves the working directory as a target.

- [ ] **Step 3: Implement optional launch targeting**

Change `DesktopLaunch.target` to `Option<String>`. Dispatch explicit `open` as `Some(args.target)`, a protocol activation as `Some(protocol_target)`, and bare startup as `None`.

In Tauri setup, resolve only a supplied target:

```rust
let target = launch
    .target
    .as_deref()
    .map(|raw| TargetResolver::new(launch.cwd.clone()).resolve(raw))
    .transpose()?;
```

Start the runtime with `target.as_ref().map(RouteTarget::entry)` and construct the initial route as follows:

```rust
fn initial_route(
    target: Option<&RouteTarget>,
    restored: Option<PersistedRoute>,
    restore_route: bool,
) -> Option<PersistedRoute> {
    if restore_route && restored.is_some() {
        return restored;
    }
    target.and_then(|target| PersistedRoute::new(target.space_id.clone(), target.issue))
}
```

Apply a route fragment only when this function returns `Some`. With neither a target nor restored route, load the cockpit URL without a fragment so the frontend can select a persisted first space or show its authoritative empty state.

Return `None` from `forwarded_route_event` for an empty forwarded argument list. The single-instance callback still shows and focuses the window, but pushes into `RouteInbox` only when an event exists.

- [ ] **Step 4: Run focused Rust tests and Clippy**

Run:

```powershell
cargo.exe test -p stellr-app bare_launch -- --nocapture
cargo.exe test -p stellr-app desktop::tests -- --nocapture
cargo.exe test -p stellr-app --test desktop_runtime_test -- --nocapture
cargo.exe clippy -p stellr-app --all-targets --locked -- -D warnings
```

Expected: all commands exit 0; explicit open/protocol tests remain green and the no-argument cases no longer resolve the working directory.

- [ ] **Step 5: Commit the launch slice**

```powershell
git add crates/app/src/main.rs crates/app/src/desktop.rs
git commit -m "fix: open empty shell on bare desktop launch"
```

### Task 3: Prove installed startup and update the product contract

**Files:**
- Modify: `scripts/smoke-windows-nsis.ps1`
- Modify: `docs/superpowers/specs/2026-08-02-m2-native-shell-design.md`
- Modify: `CHANGELOG.md`

**Interfaces:**
- Consumes: installed NSIS executable, its native main-window process boundary, focused empty-runtime/frontend coverage, and the M2 release contract.
- Produces: a clean-runner installed-app smoke that launches from the installation directory and reaches the main window without the invalid-repository dialog; corrected design and release-note text.

- [ ] **Step 1: Make the installed smoke red-capable for this defect**

Update the real NSIS smoke to start the installed executable with its installation directory as `WorkingDirectory` and require a live main window titled `Stellr`. On the pre-fix binary this launch exits with the invalid-repository error; on the fixed binary it creates the main window. The focused runtime test proves zero spaces without a synthetic entry, and the frontend test proves the authoritative empty model renders `No spaces yet`.

The launch must have this shape:

```powershell
$installDirectory = Split-Path -Parent $installedExecutable
$appProcess = Start-Process -FilePath $installedExecutable `
  -WorkingDirectory $installDirectory `
  -PassThru
```

After the window becomes ready, continue the existing WebView2 child-process and clean-shutdown assertions.

- [ ] **Step 2: Correct the written contract and append pending release notes**

Change the M2 CLI description and acceptance criterion so bare `stellr` opens or focuses the desktop app without requiring a repository. Remove the claim that the current directory is automatically captured.

Add the newest `Unreleased` bullet:

```markdown
- Fixed installed desktop startup so a no-argument launch opens the existing
  empty repository-selection shell instead of treating the installation
  directory as a Git repository.
```

- [ ] **Step 3: Run focused script, frontend, and formatting checks**

Run:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/tests/windows-packaging.tests.ps1
npm.cmd --prefix web test -- --run
npm.cmd --prefix web run check
cargo.exe fmt --all -- --check
git diff --check
```

Expected: the packaging contract marker is printed, frontend tests/checks exit 0, Rust formatting is clean, and the diff has no whitespace errors.

- [ ] **Step 4: Run the full native validation matrix**

Run:

```powershell
cargo.exe test --workspace --locked
cargo.exe clippy --workspace --all-targets --locked -- -D warnings
cargo.exe build --workspace --locked
npm.cmd --prefix web run build
```

Expected: every command exits 0 with no test failures or warnings.

- [ ] **Step 5: Build and smoke the Windows installer**

Run:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/build-windows-nsis.ps1
```

On a clean installation boundary, run the generated unsigned development installer through `scripts/smoke-windows-nsis.ps1`. Verify the installed process starts from its installation directory, reaches the main window, launches WebView2, closes cleanly, and uninstalls cleanly. Reinstall the verified development build for the user only after explicit approval for replacing the current local installation.

- [ ] **Step 6: Review the completed changes**

Run the repository `code-review` skill against the branch base. Resolve every actionable standards or correctness finding, then repeat the relevant focused and full verification commands.

- [ ] **Step 7: Commit the final Issue #73 slice**

```powershell
git add scripts/smoke-windows-nsis.ps1 docs/superpowers/specs/2026-08-02-m2-native-shell-design.md CHANGELOG.md docs/superpowers/plans/2026-08-03-empty-startup-shell.md
git commit -m "test: prove empty installed startup"
```

# Windows Launch and CI Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship console-free installed Windows desktop launches, preserve Stellr's native CLI behavior, make Windows startup failures diagnostic, and remove the obsolete artifact-action runtime warning through supported upgrades.

**Architecture:** Split the Windows package into a GUI-subsystem `stellr-desktop.exe` entry point and the existing console-subsystem `stellr.exe` CLI, both backed by one shared Rust launch assembly. Configure Tauri's Windows bundle to use the desktop binary as its main application and include the CLI as a companion binary. Harden both real Windows startup smokes around a shared 90-second deadline and diagnostic-capture contract, then upgrade all six upload actions together.

**Tech Stack:** Rust 2024, Clap 4, Tauri 2.11, native Windows PE/Win32 process semantics, PowerShell 5.1+, NSIS, GitHub Actions, Node.js 24.

## Global Constraints

- Use native Windows 11 PowerShell, `cargo.exe`, `rustc.exe`, and `npm.cmd`; do not use WSL or Linux toolchains for local implementation or validation.
- `stellr-desktop.exe` is the packaged Windows GUI entry point; `stellr.exe` remains the console CLI.
- Release desktop PE subsystem must be `IMAGE_SUBSYSTEM_WINDOWS_GUI` (2); release CLI PE subsystem must be `IMAGE_SUBSYSTEM_WINDOWS_CUI` (3).
- Debug builds of both entry points retain console-subsystem diagnostics.
- Preserve `stellr serve`, help, version, invalid-command, bare, `open`, and protocol behavior at their approved entry points.
- Both Windows startup smokes default to a measured 90-second deadline and remain fail-closed.
- Upgrade the six `actions/upload-artifact` uses to `v7`; do not suppress, filter, or waive runtime warnings.
- Keep release notes append-only and newest-first; add pending work only under `Unreleased`.
- Implement issues #75, #76, and #77 sequentially on one branch and publish one pull request after combined verification and review.

---

### Task 1: Share desktop launch assembly across two entry points (Issue #75)

**Files:**
- Create: `crates/app/src/entrypoints.rs`
- Create: `crates/app/src/desktop_main.rs`
- Modify: `crates/app/src/cli.rs`
- Modify: `crates/app/src/lib.rs`
- Modify: `crates/app/src/main.rs`
- Modify: `crates/app/Cargo.toml`

**Interfaces:**
- Consumes: `Cli`, `Command`, `DesktopLaunch`, `stellr_app::desktop::run`, shared runtime startup, and process arguments.
- Produces: `entrypoints::run_cli() -> Result<(), DynError>`, `entrypoints::run_desktop() -> Result<(), DynError>`, and `entrypoints::desktop_launch_from<I, T>(args, cwd) -> Result<DesktopLaunch, DynError>`.

- [ ] **Step 1: Write failing shared-launch tests**

Move the existing launch-intent tests out of the binary-only module and add public-seam coverage for the desktop entry point's accepted argument shapes:

```rust
#[test]
fn desktop_entry_accepts_bare_open_and_protocol_launches() {
    let cwd = PathBuf::from(r"D:\Apps\Stellr");

    let bare = desktop_launch_from(["stellr-desktop"], cwd.clone()).unwrap();
    assert!(bare.target.is_none());
    assert!(bare.restore_route);

    let open = desktop_launch_from(
        ["stellr-desktop", "open", "teloverge/stellr"],
        cwd.clone(),
    )
    .unwrap();
    assert_eq!(open.target.as_deref(), Some("teloverge/stellr"));
    assert!(!open.restore_route);

    let protocol = desktop_launch_from(
        ["stellr-desktop", "stellr://space?repo=teloverge%2Fstellr"],
        cwd,
    )
    .unwrap();
    assert_eq!(
        protocol.target.as_deref(),
        Some("stellr://space?repo=teloverge%2Fstellr")
    );
}

#[test]
fn desktop_entry_rejects_console_only_serve() {
    let error = desktop_launch_from(
        ["stellr-desktop", "serve"],
        PathBuf::from(r"D:\Apps\Stellr"),
    )
    .unwrap_err();
    assert!(error.to_string().contains("stellr.exe"));
}
```

- [ ] **Step 2: Run the focused tests and verify red**

Run:

```powershell
cargo.exe test -p stellr-app entrypoints::tests -- --nocapture
```

Expected: compilation fails because the shared `entrypoints` module and `desktop_launch_from` interface do not exist.

- [ ] **Step 3: Move command and launch dispatch behind the shared interface**

Export the CLI module from the library, enable Clap's version output, and implement the shared launch adapter:

```rust
pub type DynError = Box<dyn std::error::Error + Send + Sync>;

pub fn desktop_launch_from<I, T>(args: I, cwd: PathBuf) -> Result<DesktopLaunch, DynError>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let cli = Cli::try_parse_from(args)?;
    desktop_launch_from_cli(cli, cwd)
}

fn desktop_launch_from_cli(cli: Cli, cwd: PathBuf) -> Result<DesktopLaunch, DynError> {
    match cli.command {
        Some(Command::Open(args)) => Ok(DesktopLaunch {
            cwd,
            target: Some(args.target),
            restore_route: false,
        }),
        None => Ok(DesktopLaunch {
            cwd,
            restore_route: cli.protocol_target.is_none(),
            target: cli.protocol_target,
        }),
        Some(Command::Serve(_)) => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "serve is available through stellr.exe",
        )
        .into()),
        #[cfg(debug_assertions)]
        Some(Command::Acceptance(_)) => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "acceptance is available through stellr.exe",
        )
        .into()),
    }
}
```

Move the existing runtime/serve dispatch into `run_cli`; make `run_desktop` call `desktop_launch_from(std::env::args_os(), launch_current_dir()?)` and then the existing native desktop runner. Reduce `main.rs` to console error reporting around `run_cli`.

Add the second binary target:

```toml
[[bin]]
name = "stellr-desktop"
path = "src/desktop_main.rs"
```

The desktop adapter declares the release-only Windows GUI subsystem and exits non-zero after the existing native startup error path:

```rust
#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

fn main() {
    if stellr_app::entrypoints::run_desktop().is_err() {
        std::process::exit(1);
    }
}
```

- [ ] **Step 4: Run the entry-point and CLI tests**

Run:

```powershell
cargo.exe test -p stellr-app entrypoints::tests -- --nocapture
cargo.exe test -p stellr-app --test serve_test -- --nocapture
cargo.exe run -p stellr-app --bin stellr -- --version
cargo.exe clippy -p stellr-app --all-targets --locked -- -D warnings
```

Expected: focused tests pass, the CLI prints version `0.1.0`, and Clippy exits 0 without warnings.

- [ ] **Step 5: Commit the shared-entry slice**

```powershell
git add crates/app/Cargo.toml crates/app/src/cli.rs crates/app/src/desktop_main.rs crates/app/src/entrypoints.rs crates/app/src/lib.rs crates/app/src/main.rs
git commit -m "feat: split Windows desktop and CLI entry points"
```

### Task 2: Package and prove both Windows executables (Issue #75)

**Files:**
- Create: `crates/app/tauri.windows.conf.json`
- Create: `scripts/assert-windows-pe-subsystem.ps1`
- Modify: `crates/app/tauri.conf.json`
- Modify: `scripts/build-windows-nsis.ps1`
- Modify: `scripts/smoke-windows-nsis.ps1`
- Modify: `scripts/tests/windows-packaging.tests.ps1`
- Modify: `.github/workflows/windows-bundle.yml`
- Modify: `.github/workflows/release.yml`
- Modify: `CHANGELOG.md`

**Interfaces:**
- Consumes: Cargo binary targets `stellr` and `stellr-desktop`, Tauri `mainBinaryName`, Tauri Windows `bundle.externalBin`, PE optional-header subsystem field, and NSIS uninstall metadata.
- Produces: a Windows installer whose application target is `stellr-desktop.exe` and whose install directory also contains `stellr.exe`; `assert-windows-pe-subsystem.ps1 -ExecutablePath <path> -ExpectedSubsystem WindowsGui|WindowsCui`.

- [ ] **Step 1: Add failing packaging-contract assertions**

Extend the Windows packaging contract to require the platform configuration, dual PE gate, desktop workflow path, and companion CLI installation:

```powershell
$windowsConfig = Get-Content (Join-Path $repo 'crates\app\tauri.windows.conf.json') -Raw | ConvertFrom-Json
Assert-True ($windowsConfig.mainBinaryName -eq 'stellr-desktop') `
  'Windows packages must use the console-free desktop entry point.'
Assert-True ($workflow.Contains('target\release\stellr-desktop.exe')) `
  'The application-process smoke must launch the desktop entry point.'
Assert-True ($buildContract.Contains('binaries/stellr')) `
  'The Windows package must include the companion Stellr CLI.'
Assert-True ($buildContract.Contains('WindowsGui')) 'The build must verify the desktop PE subsystem.'
Assert-True ($buildContract.Contains('WindowsCui')) 'The build must verify the CLI PE subsystem.'
```

- [ ] **Step 2: Run the Windows packaging contract and verify red**

Run:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/tests/windows-packaging.tests.ps1
```

Expected: FAIL because the Windows-specific Tauri configuration and PE assertion script do not exist.

- [ ] **Step 3: Implement behavioral PE inspection**

Create a native PowerShell assertion that reads the real PE header. Validate the DOS `MZ` signature, PE signature, optional-header magic, and subsystem value at offset `0x44` within the optional header:

```powershell
$peOffset = [BitConverter]::ToInt32($bytes, 0x3c)
$optionalHeaderOffset = $peOffset + 24
$magic = [BitConverter]::ToUInt16($bytes, $optionalHeaderOffset)
if ($magic -notin @(0x10b, 0x20b)) { throw "Unsupported PE optional-header magic: 0x$($magic.ToString('x4'))" }
$actual = [BitConverter]::ToUInt16($bytes, $optionalHeaderOffset + 0x44)
$expected = if ($ExpectedSubsystem -eq 'WindowsGui') { 2 } else { 3 }
if ($actual -ne $expected) { throw "Expected $ExpectedSubsystem ($expected), found subsystem $actual in $ExecutablePath." }
```

Exercise the parser in the packaging contract against native `%WINDIR%\explorer.exe` as GUI and `%ComSpec%` as CUI before relying on it for Stellr.

- [ ] **Step 4: Configure and build the dual-entry Windows package**

Set base `mainBinaryName` to `stellr` so Linux and macOS retain their current main binary. Add a Windows override:

```json
{
  "mainBinaryName": "stellr-desktop"
}
```

Set the Cargo package `default-run` to `stellr-desktop` so Tauri can select the application target when two binaries exist. Route that target through the desktop-only dispatcher solely for Windows release builds; debug Windows and every non-Windows build retain the full CLI dispatcher so existing `cargo run -- serve` and packaged Linux/macOS behavior remain compatible.

Before invoking Tauri on Windows, build `stellr.exe`, obtain the native host tuple from `rustc --print host-tuple`, and copy the CLI to the Tauri sidecar name `crates/app/binaries/stellr-<host-tuple>.exe`. Pass `bundle.externalBin = ["binaries/stellr"]` through a temporary Tauri CLI config only during packaging, because a static sidecar declaration would make ordinary Cargo tests require a generated release binary. Remove only that generated copy and temporary config in the build script's `finally` block. After Tauri builds the desktop binary, assert:

```powershell
& $peAssertion -ExecutablePath (Join-Path $repo 'target\release\stellr-desktop.exe') -ExpectedSubsystem WindowsGui
& $peAssertion -ExecutablePath (Join-Path $repo 'target\release\stellr.exe') -ExpectedSubsystem WindowsCui
```

Point the Windows workflow application-process smoke at `target\release\stellr-desktop.exe`. Make the NSIS smoke fall back to `stellr-desktop.exe`, require adjacent `stellr.exe`, and preserve the current DisplayIcon-first discovery so it proves the installed shortcut target.

- [ ] **Step 5: Run focused packaging and release-build proof**

Run:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/tests/windows-packaging.tests.ps1
npm.cmd --prefix web ci
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/build-windows-nsis.ps1
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/assert-windows-pe-subsystem.ps1 -ExecutablePath target\release\stellr-desktop.exe -ExpectedSubsystem WindowsGui
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/assert-windows-pe-subsystem.ps1 -ExecutablePath target\release\stellr.exe -ExpectedSubsystem WindowsCui
```

Expected: packaging contract passes, the unsigned NSIS installer builds, and both real PE assertions pass.

- [ ] **Step 6: Prove native CLI waiting and output**

Run the release CLI directly through both native shells:

```powershell
& .\target\release\stellr.exe --version
cmd.exe /d /c ".\target\release\stellr.exe --version && exit /b 0"
```

Expected: both print `stellr 0.1.0`, wait for completion, and exit 0. Start `serve` on an ephemeral loopback address with controlled test credentials through the existing application-process seam, observe `stellr cockpit:`, then terminate it through the existing bounded process harness.

- [ ] **Step 7: Add the newest Unreleased entry and commit Issue #75**

Prepend this pending release-note entry under `## Unreleased` without changing shipped sections:

```markdown
- Split the Windows desktop and CLI entry points so installed shortcuts and
  protocol launches open without a terminal while `stellr serve` retains
  native PowerShell and Command Prompt behavior.
```

Then commit:

```powershell
git add .github/workflows/windows-bundle.yml .github/workflows/release.yml CHANGELOG.md crates/app/tauri.conf.json crates/app/tauri.windows.conf.json scripts/assert-windows-pe-subsystem.ps1 scripts/build-windows-nsis.ps1 scripts/smoke-windows-nsis.ps1 scripts/tests/windows-packaging.tests.ps1
git commit -m "build: package console-free Windows desktop entry"
```

### Task 3: Make Windows startup failures deadline-bound and diagnostic (Issue #76)

**Files:**
- Modify: `scripts/smoke-windows-application-process.ps1`
- Modify: `scripts/smoke-windows-nsis.ps1`
- Modify: `scripts/tests/release-boundary.tests.ps1`
- Modify: `scripts/tests/windows-packaging.tests.ps1`
- Modify: `CHANGELOG.md`

**Interfaces:**
- Consumes: child process environment, `STELLR_STARTUP_DIAGNOSTICS=1`, process standard output/error, `MainWindowHandle`, and `MainWindowTitle`.
- Produces: `StartupTimeoutSeconds` defaulting to 90, deadline-based window waits, per-launch logs, and failure messages containing exit state plus the last startup marker.

- [ ] **Step 1: Add failing smoke-contract assertions**

For both smoke scripts require these observable contract tokens:

```powershell
Assert-True ($smoke.Contains('[int]$StartupTimeoutSeconds = 90')) `
  'The startup smoke must expose the approved 90-second budget.'
Assert-True ($smoke.Contains('[Diagnostics.Stopwatch]::StartNew()')) `
  'The startup smoke must measure a deadline instead of counting attempts.'
Assert-True ($smoke.Contains('STELLR_STARTUP_DIAGNOSTICS')) `
  'The startup smoke must enable native stage diagnostics for the child.'
Assert-True ($smoke.Contains('RedirectStandardError')) `
  'The startup smoke must capture startup diagnostics.'
Assert-True ($smoke.Contains('STELLR_DESKTOP_STARTUP_STAGE')) `
  'The startup failure must expose the last native startup stage.'
```

- [ ] **Step 2: Run both contract tests and verify red**

Run:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/tests/release-boundary.tests.ps1
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/tests/windows-packaging.tests.ps1
```

Expected: both fail on the missing named timeout and diagnostic capture.

- [ ] **Step 3: Implement child-scoped diagnostics and deadline waits**

Add the named parameter to each smoke. For every desktop launch, allocate unique output/error paths below `$env:RUNNER_TEMP` (falling back to `[IO.Path]::GetTempPath()`), save the previous diagnostic environment value, set it to `1` only while `Start-Process` inherits the environment, and restore or remove it in `finally`.

Start each process with:

```powershell
$process = Start-Process -FilePath $executable `
  -ArgumentList $Arguments `
  -WorkingDirectory $workingDirectory `
  -RedirectStandardOutput $stdoutPath `
  -RedirectStandardError $stderrPath `
  -PassThru
```

Replace the 60-attempt startup loops with:

```powershell
$startup = [Diagnostics.Stopwatch]::StartNew()
while ($startup.Elapsed.TotalSeconds -lt $StartupTimeoutSeconds) {
  Start-Sleep -Milliseconds 500
  $process.Refresh()
  if ($process.HasExited) { throw (New-StellrStartupFailure $process $stdoutPath $stderrPath) }
  if ($process.MainWindowTitle -eq 'Stellr' -and $process.MainWindowHandle -ne 0) { return $process }
}
throw (New-StellrStartupFailure $process $stdoutPath $stderrPath)
```

`New-StellrStartupFailure` reads both logs without deleting them, extracts the final `STELLR_DESKTOP_STARTUP_STAGE=` or `STELLR_DESKTOP_STARTUP_ERROR=` line, and includes it with process id/exit state in the thrown message. The timeout path remains non-zero.

- [ ] **Step 4: Run focused contracts and PowerShell parsing**

Run:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/tests/release-boundary.tests.ps1
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/tests/windows-packaging.tests.ps1
$null = [scriptblock]::Create((Get-Content scripts/smoke-windows-application-process.ps1 -Raw))
$null = [scriptblock]::Create((Get-Content scripts/smoke-windows-nsis.ps1 -Raw))
```

Expected: contract markers print, and both scripts parse without errors.

- [ ] **Step 5: Add the newest Unreleased entry and commit Issue #76**

Prepend this entry under `Unreleased`:

```markdown
- Hardened Windows application startup smokes with a measured 90-second cold
  start budget and captured native startup-stage diagnostics on failure.
```

Then commit:

```powershell
git add CHANGELOG.md scripts/smoke-windows-application-process.ps1 scripts/smoke-windows-nsis.ps1 scripts/tests/release-boundary.tests.ps1 scripts/tests/windows-packaging.tests.ps1
git commit -m "test: diagnose Windows startup smoke failures"
```

### Task 4: Upgrade all bundle artifact uploads (Issue #77)

**Files:**
- Modify: `.github/workflows/linux-bundle.yml`
- Modify: `.github/workflows/macos-bundle.yml`
- Modify: `.github/workflows/windows-bundle.yml`
- Modify: `.github/workflows/release.yml`
- Modify: `scripts/tests/release-boundary.tests.ps1`
- Modify: `CHANGELOG.md`

**Interfaces:**
- Consumes: the six existing artifact names, paths, and `if-no-files-found` contracts.
- Produces: exactly six `actions/upload-artifact@v7` upload steps and zero obsolete upload `@v4` steps.

- [ ] **Step 1: Add a failing exact-count workflow contract**

Read every workflow as one string and assert exact upload-version counts:

```powershell
$workflowText = (Get-ChildItem (Join-Path $repo '.github\workflows') -File |
  ForEach-Object { Get-Content $_.FullName -Raw }) -join "`n"
$v7Uploads = [regex]::Matches($workflowText, 'actions/upload-artifact@v7').Count
$v4Uploads = [regex]::Matches($workflowText, 'actions/upload-artifact@v4').Count
Assert-True ($v7Uploads -eq 6) "Expected six v7 artifact uploads; found $v7Uploads."
Assert-True ($v4Uploads -eq 0) "Obsolete v4 artifact uploads remain: $v4Uploads."
```

- [ ] **Step 2: Run the release-boundary contract and verify red**

Run:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/tests/release-boundary.tests.ps1
```

Expected: FAIL with zero v7 uploads and six remaining v4 uploads.

- [ ] **Step 3: Upgrade the six upload sites without changing inputs**

Change only `actions/upload-artifact@v4` to `actions/upload-artifact@v7` in the Linux, macOS, Windows, and three release jobs. Preserve every `with.name`, `with.path`, and `if-no-files-found` value. Do not add warning filters or `continue-on-error`.

- [ ] **Step 4: Run workflow contracts and inspect the diff**

Run:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/tests/release-boundary.tests.ps1
rg -n "actions/upload-artifact@" .github/workflows
git diff -- .github/workflows
```

Expected: exactly six v7 lines appear, no v4 upload line appears, and the diff changes no artifact input.

- [ ] **Step 5: Add the newest Unreleased entry and commit Issue #77**

Prepend this entry under `Unreleased`:

```markdown
- Upgraded all bundle artifact uploads to `actions/upload-artifact@v7`, removing
  the obsolete action-runtime warning without suppressing it.
```

Then commit:

```powershell
git add .github/workflows/linux-bundle.yml .github/workflows/macos-bundle.yml .github/workflows/windows-bundle.yml .github/workflows/release.yml CHANGELOG.md scripts/tests/release-boundary.tests.ps1
git commit -m "ci: upgrade artifact uploads to v7"
```

### Task 5: Verify, review, and publish one combined pull request

**Files:**
- Modify only files required to resolve verified failures or actionable review findings.

**Interfaces:**
- Consumes: the completed #75, #76, and #77 commits.
- Produces: one reviewed branch and one pull request that closes all three issues after required GitHub checks pass.

- [ ] **Step 1: Run the complete native validation matrix**

Run:

```powershell
Get-ChildItem scripts\tests -Filter *.tests.ps1 | ForEach-Object { & $_.FullName }
cargo.exe fmt --all -- --check
cargo.exe clippy --workspace --all-targets --locked -- -D warnings
cargo.exe test --workspace --locked
cargo.exe build --workspace --locked
npm.cmd --prefix web run check
npm.cmd --prefix web test
npm.cmd --prefix web run build
git diff --check
```

Expected: every command exits 0 and all contract markers print.

- [ ] **Step 2: Build the real unsigned Windows installer and recheck PE boundaries**

Run:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/build-windows-nsis.ps1 -Channel Development
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/assert-windows-pe-subsystem.ps1 -ExecutablePath target\release\stellr-desktop.exe -ExpectedSubsystem WindowsGui
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/assert-windows-pe-subsystem.ps1 -ExecutablePath target\release\stellr.exe -ExpectedSubsystem WindowsCui
```

Expected: unsigned installer creation and both PE assertions pass. Do not replace an existing local installation without separate user authorization.

- [ ] **Step 3: Review the complete branch**

Run the repository `code-review` skill against merge base `origin/main`. Resolve every actionable correctness or standards finding and rerun the proportional focused gates plus the full affected matrix.

- [ ] **Step 4: Commit final review corrections if needed**

```powershell
git add -- .github/workflows/linux-bundle.yml .github/workflows/macos-bundle.yml .github/workflows/release.yml .github/workflows/windows-bundle.yml CHANGELOG.md crates/app/Cargo.toml crates/app/src/cli.rs crates/app/src/desktop_main.rs crates/app/src/entrypoints.rs crates/app/src/lib.rs crates/app/src/main.rs crates/app/tauri.conf.json crates/app/tauri.windows.conf.json scripts/assert-windows-pe-subsystem.ps1 scripts/build-windows-nsis.ps1 scripts/smoke-windows-application-process.ps1 scripts/smoke-windows-nsis.ps1 scripts/tests/release-boundary.tests.ps1 scripts/tests/windows-packaging.tests.ps1
git commit -m "fix: address Windows hardening review"
```

Skip this commit when review produces no code changes; never create an empty commit.

- [ ] **Step 5: Push and open one pull request**

Push `codex/windows-launch-hardening` over native Windows OpenSSH. Open one ready-for-review PR whose body contains:

```markdown
## Summary
- split the Windows desktop and CLI entry points so installed launch is console-free
- make Windows startup smokes deadline-bound and diagnostic
- upgrade all bundle artifact uploads to v7 without suppressing warnings

## Validation
- complete native PowerShell contract suite
- Rust format, Clippy, test, and build matrix
- frontend check, test, and production build
- unsigned Windows NSIS build plus GUI/CUI PE-header assertions

Closes #75
Closes #76
Closes #77
```

- [ ] **Step 6: Wait for GitHub checks**

Wait for the pull request's required CI, Windows bundle/application-process/installed smokes, and Linux/macOS bundle uploads. If a check fails, inspect the exact live logs, fix only the demonstrated defect, rerun local proportional gates, push the correction, and wait again. Confirm the artifact runtime warning is absent rather than merely hidden.

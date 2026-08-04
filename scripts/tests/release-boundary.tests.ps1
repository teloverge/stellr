$ErrorActionPreference = 'Stop'

$repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path

function Assert-True([bool]$Condition, [string]$Message) {
  if (-not $Condition) { throw $Message }
}

$processSmokePath = Join-Path $repo 'scripts\smoke-windows-application-process.ps1'
$startupHelperPath = Join-Path $repo 'scripts\windows-startup-diagnostics.ps1'
Assert-True (Test-Path $processSmokePath) 'The real Windows application-process smoke is missing.'
Assert-True (Test-Path $startupHelperPath) 'The shared Windows startup diagnostics helper is missing.'
$processSmoke = Get-Content $processSmokePath -Raw
$startupHelper = Get-Content $startupHelperPath -Raw
Assert-True ($processSmoke.Contains('GITHUB_TOKEN')) 'The process smoke must prove authenticated startup.'
Assert-True ($startupHelper.Contains('Start-Process')) 'The process smoke must launch the real Stellr binary.'
Assert-True ($processSmoke.Contains('second instance')) 'The process smoke must prove second-instance forwarding.'
Assert-True ($processSmoke.Contains('ShowWindowAsync')) 'The process smoke must exercise native focus transitions.'
Assert-True ($processSmoke.Contains('WINDOWS_ROUTE_RESTORED_AFTER_RELAUNCH=true')) `
  'The process smoke must prove route restoration after relaunch.'
Assert-True ($processSmoke.Contains('[int]$StartupTimeoutSeconds = 90')) `
  'The process smoke must expose the approved 90-second startup budget.'
Assert-True ($processSmoke.Contains('windows-startup-diagnostics.ps1')) `
  'The process smoke must use the shared startup diagnostics boundary.'
Assert-True ($startupHelper.Contains('[Diagnostics.Stopwatch]::StartNew()')) `
  'The process smoke must measure a startup deadline instead of counting attempts.'
Assert-True ($startupHelper.Contains('STELLR_STARTUP_DIAGNOSTICS')) `
  'The process smoke must enable native stage diagnostics for each child.'
Assert-True ($startupHelper.Contains('RedirectStandardError')) `
  'The process smoke must capture startup diagnostics.'
Assert-True ($startupHelper.Contains('STELLR_DESKTOP_STARTUP_STAGE')) `
  'The process smoke must report the last native startup stage.'
Assert-True ($startupHelper.Contains('GetExitCodeProcess')) `
  'The process smoke must read redirected child exit codes through the native Windows handle.'
Assert-True ($processSmoke.Contains('New-StellrStartupLog $startupLogRoot ''second-instance''')) `
  'The second-instance launch must use per-launch diagnostics too.'
Assert-True ($processSmoke.Contains('$second.WaitForExit()')) `
  'The second-instance smoke must complete the native process wait before reading ExitCode.'
Assert-True ($processSmoke.Contains('$second.Refresh()')) `
  'The second-instance smoke must refresh the native process before reading ExitCode.'
Assert-True ($processSmoke.Contains('$secondExitCode = Get-StellrProcessExitCode $second')) `
  'The second-instance smoke must use the reliable native exit-code reader.'

$releasePath = Join-Path $repo '.github\workflows\release.yml'
Assert-True (Test-Path $releasePath) 'The fail-closed tagged release workflow is missing.'
$release = Get-Content $releasePath -Raw
Assert-True ($release.Contains('tags:')) 'The release workflow must be tag-triggered.'
Assert-True ($release.Contains('cargo clippy --workspace --all-targets -- -D warnings')) 'Tagged releases must run warnings-denied Clippy.'
Assert-True ($release.Contains('npm --prefix web run check')) 'Tagged releases must run frontend type-checking.'
Assert-True ($release.Contains('WINDOWS_CERTIFICATE_BASE64')) 'Tagged releases must require Windows signing credentials.'
Assert-True ($release.Contains('APPLE_CERTIFICATE')) 'Tagged releases must require macOS signing credentials.'
Assert-True ($release.Contains('UNSIGNED-NOT-FOR-RELEASE')) 'Publication must explicitly reject development artifacts.'
Assert-True ($release.Contains('gh release create')) 'Only the final gated job may create the draft release.'

$workflowText = (Get-ChildItem (Join-Path $repo '.github\workflows') -File |
  ForEach-Object { Get-Content $_.FullName -Raw }) -join "`n"
$v7Uploads = [regex]::Matches($workflowText, 'actions/upload-artifact@v7').Count
$v4Uploads = [regex]::Matches($workflowText, 'actions/upload-artifact@v4').Count
Assert-True ($v7Uploads -eq 6) "Expected six v7 artifact uploads; found $v7Uploads."
Assert-True ($v4Uploads -eq 0) "Obsolete v4 artifact uploads remain: $v4Uploads."

$ci = Get-Content (Join-Path $repo '.github\workflows\ci.yml') -Raw
Assert-True ($ci.Contains('libwebkit2gtk-4.1-dev')) 'Linux CI must install the native Tauri WebKitGTK toolchain.'
Assert-True ($ci.Contains('cargo clippy --workspace --all-targets -- -D warnings')) `
  'Ordinary CI must lint every Rust target with warnings denied.'
Assert-True ($ci.Contains('cargo build --workspace')) 'Ordinary CI must compile the complete workspace.'

$readme = Get-Content (Join-Path $repo 'README.md') -Raw
foreach ($required in @(
  'Desktop mode',
  'Serve mode',
  'Credential precedence',
  'Device authorization',
  'Deep links and single-instance routing',
  'Supported packages'
)) {
  Assert-True ($readme.Contains($required)) "README is missing: $required"
}

$changelog = Get-Content (Join-Path $repo 'CHANGELOG.md')
$unreleased = [Array]::IndexOf($changelog, '## Unreleased')
Assert-True ($unreleased -ge 0) 'The changelog must retain an Unreleased section.'
$nextRelease = $unreleased + 1
while ($nextRelease -lt $changelog.Count -and -not $changelog[$nextRelease].StartsWith('## ')) {
  $nextRelease++
}
$unreleasedBody = $changelog[($unreleased + 1)..($nextRelease - 1)] -join "`n"
Assert-True ($unreleasedBody.Contains('native desktop shell')) `
  'Unreleased must retain the completed native desktop shell entry.'

Write-Output 'RELEASE_BOUNDARY_CONTRACT_PASSED=true'

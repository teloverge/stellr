$ErrorActionPreference = 'Stop'

$repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path

function Assert-True([bool]$Condition, [string]$Message) {
  if (-not $Condition) { throw $Message }
}

$processSmokePath = Join-Path $repo 'scripts\smoke-windows-application-process.ps1'
Assert-True (Test-Path $processSmokePath) 'The real Windows application-process smoke is missing.'
$processSmoke = Get-Content $processSmokePath -Raw
Assert-True ($processSmoke.Contains('GITHUB_TOKEN')) 'The process smoke must prove authenticated startup.'
Assert-True ($processSmoke.Contains('Start-Process')) 'The process smoke must launch the real Stellr binary.'
Assert-True ($processSmoke.Contains('second instance')) 'The process smoke must prove second-instance forwarding.'
Assert-True ($processSmoke.Contains('ShowWindowAsync')) 'The process smoke must exercise native focus transitions.'
Assert-True ($processSmoke.Contains('WINDOWS_ROUTE_RESTORED_AFTER_RELAUNCH=true')) `
  'The process smoke must prove route restoration after relaunch.'

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
Assert-True ($changelog[$unreleased + 2].Contains('native desktop shell')) `
  'The newest Unreleased entry must describe the completed native desktop shell.'

Write-Output 'RELEASE_BOUNDARY_CONTRACT_PASSED=true'

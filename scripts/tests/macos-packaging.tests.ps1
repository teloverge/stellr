$ErrorActionPreference = 'Stop'

$repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path

function Assert-True([bool]$Condition, [string]$Message) {
  if (-not $Condition) { throw $Message }
}

$config = Get-Content (Join-Path $repo 'crates\app\tauri.conf.json') -Raw | ConvertFrom-Json
Assert-True ($config.bundle.icon -contains 'icons/icon.icns') 'The macOS application icon is missing from the bundle.'
Assert-True (Test-Path (Join-Path $repo 'crates\app\icons\icon.icns')) 'The configured ICNS file does not exist.'

$workflowPath = Join-Path $repo '.github\workflows\macos-bundle.yml'
Assert-True (Test-Path $workflowPath) 'The native macOS bundle workflow is missing.'
$workflow = Get-Content $workflowPath -Raw
Assert-True ($workflow.Contains('runs-on: macos-latest')) 'The DMG must build on a native macOS runner.'
Assert-True ($workflow.Contains('aarch64-apple-darwin,x86_64-apple-darwin')) `
  'The workflow must install both universal-binary Rust targets.'
Assert-True ($workflow.Contains('UNSIGNED-NOT-FOR-RELEASE')) 'Development DMGs must be unmistakably unsigned.'
Assert-True ($workflow.Contains('GITHUB_STEP_SUMMARY')) 'The workflow summary must warn that development DMGs are unsigned.'
$releaseWorkflow = Get-Content (Join-Path $repo '.github\workflows\release.yml') -Raw
Assert-True ($releaseWorkflow.Contains('APPLE_CERTIFICATE')) 'Tagged builds must require the Apple certificate secret.'
Assert-True ($releaseWorkflow.Contains('APPLE_CERTIFICATE_PASSWORD')) 'Tagged builds must require the Apple certificate password.'
Assert-True ($releaseWorkflow.Contains('KEYCHAIN_PASSWORD')) 'Tagged builds must require an ephemeral keychain password.'

$buildScript = Join-Path $repo 'scripts\build-macos-universal.sh'
$inspectScript = Join-Path $repo 'scripts\inspect-macos-dmg.sh'
$signingScript = Join-Path $repo 'scripts\assert-macos-signing.sh'
Assert-True (Test-Path $buildScript) 'The universal macOS build script is missing.'
Assert-True (Test-Path $inspectScript) 'The DMG inspection script is missing.'
Assert-True (Test-Path $signingScript) 'The fail-closed macOS signing guard is missing.'

$build = Get-Content $buildScript -Raw
Assert-True ($build.Contains('--target universal-apple-darwin')) 'The build must use Tauri universal-apple-darwin.'
Assert-True ($build.Contains('--bundles dmg')) 'The build must produce a DMG.'
Assert-True (-not $build.Contains('app="$bundle_root/macos/Stellr.app"')) `
  'The build must not require Tauri staging app output after DMG finalization.'

$inspect = Get-Content $inspectScript -Raw
Assert-True ($inspect.Contains('The DMG does not contain Stellr.app')) `
  'The mounted DMG inspection must own the packaged app-presence proof.'
Assert-True ($inspect.Contains('lipo -archs')) 'The packaged binary slices must be inspected with lipo.'
Assert-True ($inspect.Contains('arm64')) 'The inspection must require an arm64 slice.'
Assert-True ($inspect.Contains('x86_64')) 'The inspection must require an x86_64 slice.'
Assert-True ($inspect.Contains('codesign --verify')) 'Signed candidates must pass strict code-signature verification.'
Assert-True ($inspect.Contains('kill -0')) 'Signed candidates must remain alive during the launch gate.'

$signing = Get-Content $signingScript -Raw
Assert-True ($signing.Contains('APPLE_CERTIFICATE APPLE_CERTIFICATE_PASSWORD KEYCHAIN_PASSWORD')) `
  'The macOS signing guard must reject every required credential when missing.'
Assert-True ($signing.Contains('openssl base64 -d -A')) 'The signing guard must reject malformed certificate data.'

Write-Output 'MACOS_PACKAGING_CONTRACT_PASSED=true'

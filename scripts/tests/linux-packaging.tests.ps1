$ErrorActionPreference = 'Stop'

$repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path

function Assert-True([bool]$Condition, [string]$Message) {
  if (-not $Condition) { throw $Message }
}

$workflowPath = Join-Path $repo '.github\workflows\linux-bundle.yml'
Assert-True (Test-Path $workflowPath) 'The native Linux package workflow is missing.'
$workflow = Get-Content $workflowPath -Raw
Assert-True ($workflow.Contains('runs-on: ubuntu-22.04')) 'Linux packages must use the supported Ubuntu 22.04 baseline.'
Assert-True ($workflow.Contains('libwebkit2gtk-4.1-dev')) 'The workflow must install the Tauri WebKitGTK 4.1 toolchain.'
Assert-True ($workflow.Contains('UNSIGNED-NOT-FOR-RELEASE')) 'Development packages must be unmistakably unsigned.'
Assert-True ($workflow.Contains('GITHUB_STEP_SUMMARY')) 'The workflow summary must warn that development packages are unsigned.'
Assert-True ($workflow.Contains('actions/download-artifact@v4')) 'Smoke jobs must download packages into clean runners.'
Assert-True ($workflow.Contains('smoke-appimage')) 'The AppImage must have its own clean-runner smoke job.'
Assert-True ($workflow.Contains('smoke-deb')) 'The deb must have its own clean-runner smoke job.'

$buildScript = Join-Path $repo 'scripts\build-linux-packages.sh'
$smokeScript = Join-Path $repo 'scripts\smoke-linux-package.sh'
Assert-True (Test-Path $buildScript) 'The Linux package build script is missing.'
Assert-True (Test-Path $smokeScript) 'The Linux package smoke script is missing.'

$build = Get-Content $buildScript -Raw
Assert-True ($build.Contains('--bundles appimage,deb')) 'The build must produce both AppImage and deb packages.'
Assert-True ($build.Contains('uname -m')) 'The build must reject non-x86_64 hosts.'

$smoke = Get-Content $smokeScript -Raw
Assert-True ($smoke.Contains('xvfb-run')) 'The smoke must launch the native shell under a display server.'
Assert-True ($smoke.Contains('xdotool search --onlyvisible')) 'The smoke must prove the native shell is visible.'
Assert-True ($smoke.Contains('ss -ltnp')) 'The smoke must inspect the application process listeners.'
Assert-True ($smoke.Contains('127.0.0.1:')) 'The smoke must permit IPv4 loopback listeners.'
Assert-True ($smoke.Contains('[::1]')) 'The smoke must permit IPv6 loopback listeners.'
Assert-True ($smoke.Contains('apt-get install')) 'The deb smoke must install the package on the clean runner.'
Assert-True ($smoke.Contains('apt-get remove')) 'The deb smoke must remove the installed package.'

Write-Output 'LINUX_PACKAGING_CONTRACT_PASSED=true'

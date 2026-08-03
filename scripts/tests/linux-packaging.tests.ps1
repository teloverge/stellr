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
Assert-True ($workflow.Contains('libegl1')) 'The clean AppImage runner must install the EGL runtime required by WebKitGTK.'
Assert-True ($workflow.Contains('openbox')) 'Linux smoke jobs must provide a window manager for the visibility proof.'
$appImageStart = $workflow.IndexOf('  smoke-appimage:')
$appImageEnd = $workflow.IndexOf('  smoke-deb:', $appImageStart)
$appImageJob = $workflow.Substring($appImageStart, $appImageEnd - $appImageStart)
Assert-True ($appImageJob.Contains('xdg-desktop-portal')) `
  'The clean AppImage runner must provide the desktop portal service used during WebKitGTK startup.'
Assert-True ($appImageJob.Contains('xdg-desktop-portal-gtk')) `
  'The clean AppImage runner must provide a desktop portal backend.'
$releaseWorkflow = Get-Content (Join-Path $repo '.github\workflows\release.yml') -Raw
Assert-True ($releaseWorkflow.Contains('libegl1')) 'Release AppImage smoke must install the EGL runtime.'
Assert-True ($releaseWorkflow.Contains('openbox')) 'Release Linux smoke jobs must provide a window manager.'
$releaseAppImageStart = $releaseWorkflow.IndexOf('  linux-appimage-smoke:')
$releaseAppImageEnd = $releaseWorkflow.IndexOf('  linux-deb-smoke:', $releaseAppImageStart)
$releaseAppImageJob = $releaseWorkflow.Substring(
  $releaseAppImageStart,
  $releaseAppImageEnd - $releaseAppImageStart
)
Assert-True ($releaseAppImageJob.Contains('xdg-desktop-portal')) `
  'Release AppImage smoke must provide the desktop portal service.'
Assert-True ($releaseAppImageJob.Contains('xdg-desktop-portal-gtk')) `
  'Release AppImage smoke must provide a desktop portal backend.'

$buildScript = Join-Path $repo 'scripts\build-linux-packages.sh'
$smokeScript = Join-Path $repo 'scripts\smoke-linux-package.sh'
$desktopSource = Join-Path $repo 'crates\app\src\desktop.rs'
Assert-True (Test-Path $buildScript) 'The Linux package build script is missing.'
Assert-True (Test-Path $smokeScript) 'The Linux package smoke script is missing.'
Assert-True (Test-Path $desktopSource) 'The desktop host source is missing.'

$config = Get-Content (Join-Path $repo 'crates\app\tauri.conf.json') -Raw | ConvertFrom-Json
$debDepends = $config.bundle.linux.deb.depends
Assert-True ($debDepends -contains 'libwebkit2gtk-4.1-0') 'The deb must declare its WebKitGTK runtime dependency.'
Assert-True ($debDepends -contains 'libgtk-3-0') 'The deb must declare its GTK runtime dependency.'
Assert-True ($debDepends -contains 'libayatana-appindicator3-1 | libappindicator3-1') `
  'The deb must declare a compatible tray-indicator runtime dependency.'

$defaultIcon = Join-Path $repo 'crates\app\icons\icon.png'
$defaultIconBytes = [System.IO.File]::ReadAllBytes($defaultIcon)
Assert-True ($defaultIconBytes[24] -eq 8) 'The default PNG must use 8-bit channels for the Linux tray backend.'
Assert-True ($defaultIconBytes[25] -eq 6) 'The default PNG must use RGBA color for the Linux tray backend.'

$build = Get-Content $buildScript -Raw
Assert-True ($build.Contains('--bundles appimage,deb')) 'The build must produce both AppImage and deb packages.'
Assert-True ($build.Contains('uname -m')) 'The build must reject non-x86_64 hosts.'

$smoke = Get-Content $smokeScript -Raw
$desktop = Get-Content $desktopSource -Raw
Assert-True ($smoke.Contains('STELLR_STARTUP_DIAGNOSTICS=1')) `
  'The smoke must enable opt-in desktop startup diagnostics.'
Assert-True ($desktop.Contains('STELLR_DESKTOP_STARTUP_STAGE=')) `
  'The desktop host must expose opt-in startup stage evidence.'
Assert-True ($smoke.Contains('xvfb-run')) 'The smoke must launch the native shell under a display server.'
Assert-True ($smoke.Contains('dbus-run-session')) 'The smoke must launch WebKitGTK inside a clean D-Bus session.'
Assert-True ($smoke.Contains('openbox')) 'The smoke must launch a window manager before asserting visibility.'
$xvfbIndex = $smoke.IndexOf('xvfb-run')
$dbusIndex = $smoke.IndexOf('dbus-run-session')
Assert-True ($xvfbIndex -lt $dbusIndex) 'Xvfb must set DISPLAY before the D-Bus activation environment is created.'
Assert-True ($smoke.Contains('xdotool search --onlyvisible')) 'The smoke must prove the native shell is visible.'
Assert-True ($smoke.Contains('ps -eo pid=,ppid=,stat=,comm=,args= --forest')) `
  'A failed smoke must report the application process tree.'
Assert-True ($smoke.Contains('xdotool getwindowname')) `
  'A failed smoke must report every visible window title.'
Assert-True ($smoke.Contains('xdotool getwindowpid')) `
  'A failed smoke must report every visible window owner.'
Assert-True ($smoke.Contains('ss -ltnp')) 'The smoke must inspect the application process listeners.'
Assert-True ($smoke.Contains('127.0.0.1:')) 'The smoke must permit IPv4 loopback listeners.'
Assert-True ($smoke.Contains('[::1]')) 'The smoke must permit IPv6 loopback listeners.'
Assert-True ($smoke.Contains('apt-get install')) 'The deb smoke must install the package on the clean runner.'
Assert-True ($smoke.Contains('apt-get remove')) 'The deb smoke must remove the installed package.'

Write-Output 'LINUX_PACKAGING_CONTRACT_PASSED=true'

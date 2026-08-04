$ErrorActionPreference = 'Stop'

$repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path

function Assert-True([bool]$Condition, [string]$Message) {
  if (-not $Condition) { throw $Message }
}

$config = Get-Content (Join-Path $repo 'crates\app\tauri.conf.json') -Raw | ConvertFrom-Json
Assert-True ($config.bundle.windows.webviewInstallMode.type -eq 'downloadBootstrapper') `
  'Windows bundles must explicitly provision WebView2 with the downloaded bootstrapper.'
Assert-True ($config.bundle.windows.nsis.installMode -eq 'currentUser') `
  'The NSIS installer must support non-elevated clean-runner install and uninstall.'

$windowsConfigPath = Join-Path $repo 'crates\app\tauri.windows.conf.json'
Assert-True (Test-Path $windowsConfigPath) 'The Windows-specific Tauri entry-point configuration is missing.'
$windowsConfig = Get-Content $windowsConfigPath -Raw | ConvertFrom-Json
Assert-True ($windowsConfig.mainBinaryName -eq 'stellr-desktop') `
  'Windows packages must use the console-free desktop entry point.'
$cargoManifest = Get-Content (Join-Path $repo 'crates\app\Cargo.toml') -Raw
Assert-True ($cargoManifest.Contains('default-run = "stellr-desktop"')) `
  'Tauri must select the dedicated desktop binary when the package has multiple entry points.'

$workflowPath = Join-Path $repo '.github\workflows\windows-bundle.yml'
Assert-True (Test-Path $workflowPath) 'The native Windows bundle workflow is missing.'
$workflow = Get-Content $workflowPath -Raw
Assert-True ($workflow.Contains('runs-on: windows-latest')) 'The bundle must build on a native Windows runner.'
Assert-True ($workflow.Contains('UNSIGNED-NOT-FOR-RELEASE')) 'Development artifacts must be unmistakably unsigned.'
Assert-True ($workflow.Contains('GITHUB_STEP_SUMMARY')) 'The workflow summary must warn that development artifacts are unsigned.'
Assert-True ($workflow.Contains('smoke-windows-nsis.ps1')) 'The clean-runner installer smoke test must gate artifacts.'
Assert-True ($workflow.Contains('smoke-windows-cli.ps1')) 'The release CLI shell matrix must gate artifacts.'
Assert-True ($workflow.Contains('target\release\stellr-desktop.exe')) `
  'The application-process smoke must launch the desktop entry point.'
$releaseWorkflow = Get-Content (Join-Path $repo '.github\workflows\release.yml') -Raw
Assert-True ($releaseWorkflow.Contains('WINDOWS_CERTIFICATE_BASE64')) 'Tagged builds must require the Windows certificate secret.'
Assert-True ($releaseWorkflow.Contains('WINDOWS_CERTIFICATE_PASSWORD')) 'Tagged builds must require the certificate password secret.'
Assert-True ((Get-Content (Join-Path $repo '.gitignore') -Raw).Contains('artifacts/')) `
  'Generated package artifacts must stay out of source control.'

$buildScript = Join-Path $repo 'scripts\build-windows-nsis.ps1'
$smokeScript = Join-Path $repo 'scripts\smoke-windows-nsis.ps1'
$cliSmokeScript = Join-Path $repo 'scripts\smoke-windows-cli.ps1'
$startupHelperPath = Join-Path $repo 'scripts\windows-startup-diagnostics.ps1'
$signingScript = Join-Path $repo 'scripts\assert-windows-signing.ps1'
$peSubsystemScript = Join-Path $repo 'scripts\assert-windows-pe-subsystem.ps1'
Assert-True (Test-Path $buildScript) 'The reproducible Windows NSIS build script is missing.'
Assert-True (Test-Path $smokeScript) 'The Windows install/launch/uninstall smoke script is missing.'
Assert-True (Test-Path $cliSmokeScript) 'The Windows release CLI smoke script is missing.'
Assert-True (Test-Path $startupHelperPath) 'The shared Windows startup diagnostics helper is missing.'
Assert-True (Test-Path $signingScript) 'The fail-closed Windows signing preflight is missing.'
Assert-True (Test-Path $peSubsystemScript) 'The Windows PE-subsystem assertion is missing.'
$buildContract = Get-Content $buildScript -Raw
Assert-True ($buildContract.Contains('Get-AuthenticodeSignature')) `
  'The build must verify whether the copied artifact is signed.'
Assert-True ($buildContract.Contains('PROCESSOR_ARCHITECTURE')) `
  'The supported build must reject non-x64 Windows hosts.'
Assert-True ($buildContract.Contains('WindowsGui')) 'The build must verify the desktop PE subsystem.'
Assert-True ($buildContract.Contains('WindowsCui')) 'The build must verify the CLI PE subsystem.'
Assert-True ($buildContract.Contains('externalBin')) 'The Windows package must configure a companion binary.'
Assert-True ($buildContract.Contains('binaries/stellr')) 'The Windows package must include the Stellr CLI.'

& $peSubsystemScript -ExecutablePath (Join-Path $env:WINDIR 'explorer.exe') -ExpectedSubsystem WindowsGui | Out-Null
& $peSubsystemScript -ExecutablePath $env:ComSpec -ExpectedSubsystem WindowsCui | Out-Null

$smokeContract = Get-Content $smokeScript -Raw
$startupHelperContract = Get-Content $startupHelperPath -Raw
Assert-True ($smokeContract.Contains('stellr-desktop.exe')) `
  'The installed smoke fallback must select the desktop entry point.'
Assert-True ($smokeContract.Contains('WINDOWS_COMPANION_CLI_INSTALLED=true')) `
  'The installed smoke must prove the companion CLI is present.'
Assert-True ($smokeContract.Contains('WINDOWS_START_MENU_TARGET=stellr-desktop.exe')) `
  'The installed smoke must prove the Start menu shortcut targets the desktop binary.'
Assert-True ($smokeContract.Contains('WINDOWS_PROTOCOL_TARGET=stellr-desktop.exe')) `
  'The installed smoke must prove the Stellr protocol targets the desktop binary.'
Assert-True ($smokeContract.Contains('WINDOWS_PROTOCOL_ACTIVATION_PASSED=true')) `
  'The installed smoke must invoke the registered Stellr protocol and observe its route.'
Assert-True ($smokeContract.Contains('WINDOWS_DISPLAY_ICON_TARGET=stellr-desktop.exe')) `
  'The installed smoke must prove uninstall metadata uses the desktop binary.'
Assert-True ($smokeContract.Contains('[int]$StartupTimeoutSeconds = 90')) `
  'The installed smoke must expose the approved 90-second startup budget.'
Assert-True ($smokeContract.Contains('windows-startup-diagnostics.ps1')) `
  'The installed smoke must use the shared startup diagnostics boundary.'
Assert-True ($startupHelperContract.Contains('[Diagnostics.Stopwatch]::StartNew()')) `
  'The installed smoke must measure a startup deadline instead of counting attempts.'
Assert-True ($startupHelperContract.Contains('STELLR_STARTUP_DIAGNOSTICS')) `
  'The installed smoke must enable native stage diagnostics for the child.'
Assert-True ($startupHelperContract.Contains('RedirectStandardError')) `
  'The installed smoke must capture startup diagnostics.'
Assert-True ($startupHelperContract.Contains('STELLR_DESKTOP_STARTUP_STAGE')) `
  'The installed smoke must report the last native startup stage.'

$cliSmokeContract = Get-Content $cliSmokeScript -Raw
Assert-True ($cliSmokeContract.Contains('$env:ComSpec')) 'The CLI smoke must exercise cmd.exe.'
Assert-True ($cliSmokeContract.Contains('powershell.exe')) 'The live CLI smoke must exercise PowerShell.'
Assert-True ($cliSmokeContract.Contains("@('--help')")) 'The CLI smoke must exercise help.'
Assert-True ($cliSmokeContract.Contains("@('--version')")) 'The CLI smoke must exercise version.'
Assert-True ($cliSmokeContract.Contains("@('not-a-command')")) 'The CLI smoke must exercise invalid-command behavior.'
Assert-True ($cliSmokeContract.Contains("@('serve'")) 'The CLI smoke must exercise and control serve mode.'
Assert-True ($cliSmokeContract.Contains('Test-StellrServeFromShell')) `
  'Live serve mode must run through both native shells.'

$signingRejected = $false
try {
  & $signingScript -CertificateBase64 '' -CertificatePassword ''
} catch {
  $signingRejected = $true
}
Assert-True $signingRejected 'The signing preflight must reject missing credentials.'

Write-Output 'WINDOWS_PACKAGING_CONTRACT_PASSED=true'

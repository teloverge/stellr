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

$workflowPath = Join-Path $repo '.github\workflows\windows-bundle.yml'
Assert-True (Test-Path $workflowPath) 'The native Windows bundle workflow is missing.'
$workflow = Get-Content $workflowPath -Raw
Assert-True ($workflow.Contains('runs-on: windows-latest')) 'The bundle must build on a native Windows runner.'
Assert-True ($workflow.Contains('UNSIGNED-NOT-FOR-RELEASE')) 'Development artifacts must be unmistakably unsigned.'
Assert-True ($workflow.Contains('GITHUB_STEP_SUMMARY')) 'The workflow summary must warn that development artifacts are unsigned.'
Assert-True ($workflow.Contains('smoke-windows-nsis.ps1')) 'The clean-runner installer smoke test must gate artifacts.'
$releaseWorkflow = Get-Content (Join-Path $repo '.github\workflows\release.yml') -Raw
Assert-True ($releaseWorkflow.Contains('WINDOWS_CERTIFICATE_BASE64')) 'Tagged builds must require the Windows certificate secret.'
Assert-True ($releaseWorkflow.Contains('WINDOWS_CERTIFICATE_PASSWORD')) 'Tagged builds must require the certificate password secret.'
Assert-True ((Get-Content (Join-Path $repo '.gitignore') -Raw).Contains('artifacts/')) `
  'Generated package artifacts must stay out of source control.'

$buildScript = Join-Path $repo 'scripts\build-windows-nsis.ps1'
$smokeScript = Join-Path $repo 'scripts\smoke-windows-nsis.ps1'
$signingScript = Join-Path $repo 'scripts\assert-windows-signing.ps1'
Assert-True (Test-Path $buildScript) 'The reproducible Windows NSIS build script is missing.'
Assert-True (Test-Path $smokeScript) 'The Windows install/launch/uninstall smoke script is missing.'
Assert-True (Test-Path $signingScript) 'The fail-closed Windows signing preflight is missing.'
$buildContract = Get-Content $buildScript -Raw
Assert-True ($buildContract.Contains('Get-AuthenticodeSignature')) `
  'The build must verify whether the copied artifact is signed.'
Assert-True ($buildContract.Contains('PROCESSOR_ARCHITECTURE')) `
  'The supported build must reject non-x64 Windows hosts.'

$signingRejected = $false
try {
  & $signingScript -CertificateBase64 '' -CertificatePassword ''
} catch {
  $signingRejected = $true
}
Assert-True $signingRejected 'The signing preflight must reject missing credentials.'

Write-Output 'WINDOWS_PACKAGING_CONTRACT_PASSED=true'

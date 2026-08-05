param(
  [ValidateSet('Development', 'Release')]
  [string]$Channel = 'Development',

  [string]$CertificateThumbprint = ''
)

$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path $PSScriptRoot\..).Path
$configPath = Join-Path $repo 'crates\app\tauri.conf.json'
$config = Get-Content $configPath -Raw | ConvertFrom-Json
$version = $config.version
$cli = Join-Path $repo 'web\node_modules\.bin\tauri.cmd'

if ($env:PROCESSOR_ARCHITECTURE -ne 'AMD64') {
  throw "The supported Windows installer must be built on x64 Windows; detected $env:PROCESSOR_ARCHITECTURE."
}
if (-not (Test-Path $cli)) {
  throw 'The pinned Tauri CLI is missing. Run npm --prefix web ci first.'
}
if ($Channel -eq 'Release' -and [string]::IsNullOrWhiteSpace($CertificateThumbprint)) {
  throw 'A certificate thumbprint is required for an official Windows release build.'
}

& npm --prefix (Join-Path $repo 'web') run build
if ($LASTEXITCODE -ne 0) { throw "Frontend build failed with exit code $LASTEXITCODE." }

$tauriArguments = @('build', '--bundles', 'nsis')
$temporaryBundleConfig = $null
$hostTuple = (& rustc --print host-tuple).Trim()
if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($hostTuple)) {
  throw 'Could not resolve the native Rust host tuple for the companion CLI.'
}
if ($hostTuple -ne 'x86_64-pc-windows-msvc') {
  throw "The supported Windows sidecar target is x86_64-pc-windows-msvc; detected $hostTuple."
}

& cargo build --package stellr-app --release --bin stellr
if ($LASTEXITCODE -ne 0) { throw "Companion Stellr CLI build failed with exit code $LASTEXITCODE." }

$cliBinary = Join-Path $repo 'target\release\stellr.exe'
$sidecarDirectory = Join-Path $repo 'crates\app\binaries'
$sidecarBinary = Join-Path $sidecarDirectory "stellr-$hostTuple.exe"
New-Item -ItemType Directory -Path $sidecarDirectory -Force | Out-Null
Copy-Item -LiteralPath $cliBinary -Destination $sidecarBinary -Force

$bundleConfig = @{
  externalBin = @('binaries/stellr')
}
if ($Channel -eq 'Release') {
  $bundleConfig.windows = @{
    certificateThumbprint = $CertificateThumbprint
    digestAlgorithm = 'sha256'
    timestampUrl = 'http://timestamp.digicert.com'
  }
}
$temporaryBundleConfig = Join-Path ([IO.Path]::GetTempPath()) "stellr-windows-bundle-$PID.json"
$configOverride = @{
  bundle = $bundleConfig
} | ConvertTo-Json -Depth 4
[IO.File]::WriteAllText($temporaryBundleConfig, $configOverride)
$tauriArguments += @('--config', $temporaryBundleConfig)

try {
  Push-Location (Join-Path $repo 'crates\app')
  & $cli @tauriArguments
  if ($LASTEXITCODE -ne 0) { throw "Tauri NSIS build failed with exit code $LASTEXITCODE." }
} finally {
  Pop-Location
  Remove-Item -LiteralPath $sidecarBinary -Force -ErrorAction SilentlyContinue
  if ($null -ne $temporaryBundleConfig) {
    Remove-Item -LiteralPath $temporaryBundleConfig -Force -ErrorAction SilentlyContinue
  }
}

$peAssertion = Join-Path $repo 'scripts\assert-windows-pe-subsystem.ps1'
& $peAssertion -ExecutablePath (Join-Path $repo 'target\release\stellr-desktop.exe') -ExpectedSubsystem WindowsGui
& $peAssertion -ExecutablePath $cliBinary -ExpectedSubsystem WindowsCui

$bundleDirectory = Join-Path $repo 'target\release\bundle\nsis'
$installer = Get-ChildItem -LiteralPath $bundleDirectory -Filter '*-setup.exe' |
  Sort-Object LastWriteTimeUtc -Descending |
  Select-Object -First 1
if ($null -eq $installer) { throw "No NSIS installer was produced in $bundleDirectory." }

$artifactDirectory = Join-Path $repo 'artifacts\windows-x64'
New-Item -ItemType Directory -Path $artifactDirectory -Force | Out-Null
$suffix = if ($Channel -eq 'Development') { '_UNSIGNED-NOT-FOR-RELEASE' } else { '' }
$artifact = Join-Path $artifactDirectory "Stellr_${version}_windows-x64_nsis${suffix}.exe"
Copy-Item -LiteralPath $installer.FullName -Destination $artifact -Force
$signature = Get-AuthenticodeSignature -FilePath $artifact
if ($Channel -eq 'Development' -and $signature.Status -ne 'NotSigned') {
  throw "Development artifact had unexpected Authenticode status: $($signature.Status)."
}
if ($Channel -eq 'Release' -and $signature.Status -ne 'Valid') {
  throw "Official Windows artifact is not validly signed: $($signature.Status)."
}
$hash = (Get-FileHash -LiteralPath $artifact -Algorithm SHA256).Hash.ToLowerInvariant()
[IO.File]::WriteAllText("$artifact.sha256", "$hash  $([IO.Path]::GetFileName($artifact))`n")

Write-Output "WINDOWS_NSIS_SIGNATURE=$($signature.Status)"
Write-Output "WINDOWS_NSIS_ARTIFACT=$artifact"

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
$temporarySigningConfig = $null
if ($Channel -eq 'Release') {
  $temporarySigningConfig = Join-Path ([IO.Path]::GetTempPath()) "stellr-windows-signing-$PID.json"
  $signingConfig = @{
    bundle = @{
      windows = @{
        certificateThumbprint = $CertificateThumbprint
        digestAlgorithm = 'sha256'
        timestampUrl = 'http://timestamp.digicert.com'
      }
    }
  } | ConvertTo-Json -Depth 4
  [IO.File]::WriteAllText($temporarySigningConfig, $signingConfig)
  $tauriArguments += @('--config', $temporarySigningConfig)
}

try {
  Push-Location (Join-Path $repo 'crates\app')
  & $cli @tauriArguments
  if ($LASTEXITCODE -ne 0) { throw "Tauri NSIS build failed with exit code $LASTEXITCODE." }
} finally {
  Pop-Location
  if ($null -ne $temporarySigningConfig) {
    Remove-Item -LiteralPath $temporarySigningConfig -Force -ErrorAction SilentlyContinue
  }
}

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

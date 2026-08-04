param(
  [Parameter(Mandatory = $true)]
  [AllowEmptyString()]
  [string]$CertificateBase64,

  [Parameter(Mandatory = $true)]
  [AllowEmptyString()]
  [string]$CertificatePassword
)

$ErrorActionPreference = 'Stop'

if ([string]::IsNullOrWhiteSpace($CertificateBase64)) {
  throw 'WINDOWS_CERTIFICATE_BASE64 is required for an official tagged release.'
}
if ([string]::IsNullOrWhiteSpace($CertificatePassword)) {
  throw 'WINDOWS_CERTIFICATE_PASSWORD is required for an official tagged release.'
}

try {
  [Convert]::FromBase64String($CertificateBase64) | Out-Null
} catch {
  throw 'WINDOWS_CERTIFICATE_BASE64 is not valid base64.'
}

Write-Output 'WINDOWS_SIGNING_PREFLIGHT_PASSED=true'

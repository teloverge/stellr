param(
  [string]$HostName = 'amd-halo'
)

$ErrorActionPreference = 'Stop'
$userName = if ([string]::IsNullOrWhiteSpace($env:STELLR_SSH_USER)) {
  'pfdev'
} else {
  $env:STELLR_SSH_USER
}
$remoteFile = '~/dev/stellr/target/stellr-tailnet-url.txt'

if ($null -eq (Get-Command ssh -ErrorAction SilentlyContinue)) {
  throw 'OpenSSH is required but ssh was not found on PATH.'
}

$url = (& ssh "$userName@$HostName" "cat $remoteFile" | Out-String).Trim()
if ($LASTEXITCODE -ne 0) {
  throw "Could not retrieve the Stellr URL from $HostName (ssh exit code $LASTEXITCODE)."
}

$parsed = $null
if (-not [Uri]::TryCreate($url, [UriKind]::Absolute, [ref]$parsed) -or
    ($parsed.Scheme -ne 'http' -and $parsed.Scheme -ne 'https')) {
  throw "The Stellr URL returned by $HostName is not an HTTP(S) URL."
}
if ($parsed.Query -notmatch '(?:^|[?&])token=') {
  throw "The Stellr URL returned by $HostName does not contain a session token."
}

Write-Output $url

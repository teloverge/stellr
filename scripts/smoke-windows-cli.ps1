param(
  [Parameter(Mandatory = $true)]
  [string]$ExecutablePath,
  [int]$StartupTimeoutSeconds = 30
)

$ErrorActionPreference = 'Stop'
if ($StartupTimeoutSeconds -le 0) { throw 'StartupTimeoutSeconds must be positive.' }
if ([string]::IsNullOrWhiteSpace($env:GITHUB_TOKEN)) {
  throw 'GITHUB_TOKEN is required to exercise serve mode.'
}

$executable = (Resolve-Path $ExecutablePath).Path

function Invoke-StellrFromPowerShell([string[]]$Arguments) {
  $previousPreference = $ErrorActionPreference
  try {
    $ErrorActionPreference = 'Continue'
    $output = & $executable @Arguments 2>&1
    $exitCode = $LASTEXITCODE
  } finally {
    $ErrorActionPreference = $previousPreference
  }
  [pscustomobject]@{
    ExitCode = $exitCode
    Output = $output -join [Environment]::NewLine
  }
}

function Invoke-StellrFromCmd([string[]]$Arguments) {
  $command = '"' + $executable + '" ' + ($Arguments -join ' ')
  $previousPreference = $ErrorActionPreference
  try {
    $ErrorActionPreference = 'Continue'
    $output = & $env:ComSpec /d /s /c $command 2>&1
    $exitCode = $LASTEXITCODE
  } finally {
    $ErrorActionPreference = $previousPreference
  }
  [pscustomobject]@{
    ExitCode = $exitCode
    Output = $output -join [Environment]::NewLine
  }
}

function Assert-StellrSuccess([pscustomobject]$Result, [string]$Pattern, [string]$Description) {
  if ($Result.ExitCode -ne 0) {
    throw "$Description exited with code $($Result.ExitCode).`n$($Result.Output)"
  }
  if ($Result.Output -notmatch $Pattern) {
    throw "$Description did not print the expected output '$Pattern'.`n$($Result.Output)"
  }
}

foreach ($shell in @(
  @{ Name = 'PowerShell'; Invoke = ${function:Invoke-StellrFromPowerShell} },
  @{ Name = 'cmd.exe'; Invoke = ${function:Invoke-StellrFromCmd} }
)) {
  $version = & $shell.Invoke @('--version')
  Assert-StellrSuccess $version '^stellr [0-9]+\.[0-9]+\.[0-9]+' "$($shell.Name) version"

  $help = & $shell.Invoke @('--help')
  Assert-StellrSuccess $help 'Usage: stellr' "$($shell.Name) help"

  $serveHelp = & $shell.Invoke @('serve', '--help')
  Assert-StellrSuccess $serveHelp 'Usage: stellr(\.exe)? serve' "$($shell.Name) serve help"

  $invalid = & $shell.Invoke @('not-a-command')
  if ($invalid.ExitCode -eq 0 -or $invalid.Output -notmatch 'error:') {
    throw "$($shell.Name) invalid-command behavior was not visible and non-zero.`n$($invalid.Output)"
  }
}

function Test-StellrServeFromShell(
  [string]$ShellName,
  [string]$FileName,
  [string]$Arguments
) {
  $start = New-Object System.Diagnostics.ProcessStartInfo
  $start.FileName = $FileName
  $start.Arguments = $Arguments
  $start.UseShellExecute = $false
  $start.RedirectStandardOutput = $true
  $start.RedirectStandardError = $true
  $start.CreateNoWindow = $true
  $shellProcess = New-Object System.Diagnostics.Process
  $shellProcess.StartInfo = $start

  try {
    if (-not $shellProcess.Start()) { throw "$ShellName serve mode could not be started." }
    $nextLine = $shellProcess.StandardOutput.ReadLineAsync()
    $startup = [Diagnostics.Stopwatch]::StartNew()
    $cockpitUrl = $null
    while ($startup.Elapsed.TotalSeconds -lt $StartupTimeoutSeconds) {
      Start-Sleep -Milliseconds 250
      if ($nextLine.IsCompleted) {
        $line = $nextLine.Result
        if ($line -match 'stellr cockpit: (?<url>http://127\.0\.0\.1:[0-9]+/(?:\?token=[0-9a-f]+)?)') {
          $cockpitUrl = $Matches.url
          break
        }
        if ($null -ne $line) { $nextLine = $shellProcess.StandardOutput.ReadLineAsync() }
      }
      if ($shellProcess.HasExited) {
        throw "$ShellName serve mode exited early with code $($shellProcess.ExitCode).`n$($shellProcess.StandardError.ReadToEnd())"
      }
    }
    if ([string]::IsNullOrWhiteSpace($cockpitUrl)) {
      throw "$ShellName serve mode did not print its cockpit URL within $StartupTimeoutSeconds seconds."
    }
    if ($shellProcess.HasExited) { throw "$ShellName did not wait for the console-subsystem CLI." }
    $response = Invoke-WebRequest -UseBasicParsing -Uri $cockpitUrl -TimeoutSec 10
    if ($response.StatusCode -ne 200) { throw "$ShellName serve mode returned HTTP $($response.StatusCode)." }
  } finally {
    if (-not $shellProcess.HasExited) {
      & (Join-Path $env:SystemRoot 'System32\taskkill.exe') /PID $shellProcess.Id /T /F | Out-Null
      $shellProcess.WaitForExit()
    }
    $shellProcess.Dispose()
  }
}

$escapedExecutable = $executable.Replace("'", "''")
$powerShellArguments = '-NoProfile -Command "& ''' + $escapedExecutable + ''' serve --addr 127.0.0.1:0 --no-token"'
Test-StellrServeFromShell 'PowerShell' (Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0\powershell.exe') $powerShellArguments
$cmdArguments = '/d /s /c ""' + $executable + '" serve --addr 127.0.0.1:0 --no-token"'
Test-StellrServeFromShell 'cmd.exe' $env:ComSpec $cmdArguments

Write-Output 'WINDOWS_CLI_POWERSHELL_PASSED=true'
Write-Output 'WINDOWS_CLI_CMD_PASSED=true'
Write-Output 'WINDOWS_CLI_SERVE_CONTROL_PASSED=true'

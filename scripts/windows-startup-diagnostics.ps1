if ($null -eq ('StellrNativeProcess' -as [type])) {
  Add-Type @'
using System;
using System.ComponentModel;
using System.Diagnostics;
using System.Runtime.InteropServices;

public static class StellrNativeProcess {
  [DllImport("kernel32.dll", SetLastError = true)]
  private static extern bool GetExitCodeProcess(IntPtr process, out uint exitCode);

  public static int ReadExitCode(Process process) {
    uint exitCode;
    if (!GetExitCodeProcess(process.Handle, out exitCode)) {
      throw new Win32Exception(Marshal.GetLastWin32Error());
    }
    return unchecked((int)exitCode);
  }
}
'@
}

function Get-StellrProcessExitCode([System.Diagnostics.Process]$Process) {
  if (-not $Process.HasExited) { throw "Process $($Process.Id) is still running." }
  [StellrNativeProcess]::ReadExitCode($Process)
}

function New-StellrStartupLogRoot([string]$Name) {
  $base = if ([string]::IsNullOrWhiteSpace($env:RUNNER_TEMP)) {
    [IO.Path]::GetTempPath()
  } else {
    $env:RUNNER_TEMP
  }
  Join-Path $base $Name
}

function New-StellrStartupLog([string]$Root, [string]$Name) {
  New-Item -ItemType Directory -Path $Root -Force | Out-Null
  [pscustomobject]@{
    StdoutPath = Join-Path $Root "$Name-stdout.log"
    StderrPath = Join-Path $Root "$Name-stderr.log"
  }
}

function Start-StellrProcessWithDiagnostics(
  [hashtable]$StartParameters,
  [pscustomobject]$StartupLog
) {
  $parameters = @{}
  foreach ($entry in $StartParameters.GetEnumerator()) {
    $parameters[$entry.Key] = $entry.Value
  }
  $parameters.PassThru = $true
  $parameters.RedirectStandardOutput = $StartupLog.StdoutPath
  $parameters.RedirectStandardError = $StartupLog.StderrPath

  $previousDiagnostics = [Environment]::GetEnvironmentVariable('STELLR_STARTUP_DIAGNOSTICS', 'Process')
  try {
    $env:STELLR_STARTUP_DIAGNOSTICS = '1'
    $process = Start-Process @parameters
    [void]$process.Handle
    $process
  } finally {
    if ($null -eq $previousDiagnostics) {
      Remove-Item Env:\STELLR_STARTUP_DIAGNOSTICS -ErrorAction SilentlyContinue
    } else {
      $env:STELLR_STARTUP_DIAGNOSTICS = $previousDiagnostics
    }
  }
}

function Get-StellrStartupFailure(
  [System.Diagnostics.Process]$Process,
  [pscustomobject]$StartupLog,
  [string]$Reason
) {
  $lines = @()
  foreach ($path in @($StartupLog.StdoutPath, $StartupLog.StderrPath)) {
    if (Test-Path -LiteralPath $path) {
      $lines += @(Get-Content -LiteralPath $path -ErrorAction SilentlyContinue)
    }
  }
  $marker = $lines |
    Where-Object { $_ -match '^STELLR_DESKTOP_STARTUP_(STAGE|ERROR)=' } |
    Select-Object -Last 1
  if ([string]::IsNullOrWhiteSpace($marker)) { $marker = 'STELLR_DESKTOP_STARTUP_STAGE=<none captured>' }
  $state = if ($Process.HasExited) {
    try {
      "exited with code $(Get-StellrProcessExitCode $Process)"
    } catch {
      "exited with an unreadable code: $($_.Exception.Message)"
    }
  } else {
    'remained running'
  }
  $diagnostics = if ($lines.Count -eq 0) { '<no startup output captured>' } else { $lines -join [Environment]::NewLine }
  "$Reason Process $($Process.Id) $state. Last startup marker: $marker`n$diagnostics"
}

function Wait-StellrDesktopWindow(
  [System.Diagnostics.Process]$Process,
  [pscustomobject]$StartupLog,
  [int]$TimeoutSeconds,
  [string]$ExitReason,
  [string]$TimeoutReason
) {
  $startup = [Diagnostics.Stopwatch]::StartNew()
  while ($startup.Elapsed.TotalSeconds -lt $TimeoutSeconds) {
    Start-Sleep -Milliseconds 500
    $Process.Refresh()
    if ($Process.HasExited) {
      throw (Get-StellrStartupFailure $Process $StartupLog $ExitReason)
    }
    if ($Process.MainWindowTitle -eq 'Stellr' -and $Process.MainWindowHandle -ne 0) {
      return $Process
    }
  }
  throw (Get-StellrStartupFailure $Process $StartupLog $TimeoutReason)
}

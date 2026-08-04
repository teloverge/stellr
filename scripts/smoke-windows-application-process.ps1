param(
  [Parameter(Mandatory = $true)]
  [string]$ExecutablePath,

  [string]$Repository = 'teloverge/stellr',
  [int]$InitialIssue = 70,
  [string]$InitialIssueTitle = 'M2: Prove and document the complete release boundary',
  [int]$ForwardedIssue = 66,
  [string]$ForwardedIssueTitle = 'M2: Ship the Polar Observatory shell and native actions'
)

$ErrorActionPreference = 'Stop'
if ($env:CI -ne 'true') {
  throw 'This smoke modifies the desktop profile and may run only on a disposable CI account.'
}
if ([string]::IsNullOrWhiteSpace($env:GITHUB_TOKEN)) {
  throw 'GITHUB_TOKEN is required to prove authenticated application startup.'
}

Add-Type -AssemblyName UIAutomationClient
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class StellrWindowControl {
  [DllImport("user32.dll")] public static extern bool ShowWindowAsync(IntPtr hWnd, int command);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
}
'@

$executable = (Resolve-Path $ExecutablePath).Path
$workingDirectory = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$routeFile = Join-Path ([Environment]::GetFolderPath('ApplicationData')) 'stellr\config\desktop-route.json'
$primary = $null

function Start-Stellr([string[]]$Arguments) {
  $start = @{
    FilePath = $executable
    WorkingDirectory = $workingDirectory
    PassThru = $true
  }
  if ($Arguments.Count -gt 0) { $start.ArgumentList = $Arguments }
  $process = Start-Process @start
  for ($attempt = 0; $attempt -lt 60; $attempt++) {
    Start-Sleep -Milliseconds 500
    $process.Refresh()
    if ($process.HasExited) { throw "Stellr exited during startup with code $($process.ExitCode)." }
    if ($process.MainWindowTitle -eq 'Stellr' -and $process.MainWindowHandle -ne 0) { return $process }
  }
  throw 'The real Stellr process did not create its desktop window.'
}

function Find-Element([System.Diagnostics.Process]$Process, [string]$Name) {
  for ($attempt = 0; $attempt -lt 80; $attempt++) {
    $Process.Refresh()
    $root = [System.Windows.Automation.AutomationElement]::FromHandle($Process.MainWindowHandle)
    $elements = $root.FindAll(
      [System.Windows.Automation.TreeScope]::Descendants,
      [System.Windows.Automation.Condition]::TrueCondition
    )
    for ($index = 0; $index -lt $elements.Count; $index++) {
      $element = $elements.Item($index)
      if ($element.Current.Name -eq $Name) { return $element }
    }
    Start-Sleep -Milliseconds 500
  }
  throw "The real Stellr UI did not expose '$Name'."
}

function Has-Element([System.Diagnostics.Process]$Process, [string]$Name) {
  $root = [System.Windows.Automation.AutomationElement]::FromHandle($Process.MainWindowHandle)
  $elements = $root.FindAll(
    [System.Windows.Automation.TreeScope]::Descendants,
    [System.Windows.Automation.Condition]::TrueCondition
  )
  for ($index = 0; $index -lt $elements.Count; $index++) {
    if ($elements.Item($index).Current.Name -eq $Name) { return $true }
  }
  return $false
}

function Close-Stellr([System.Diagnostics.Process]$Process) {
  if (-not $Process.CloseMainWindow()) { throw 'The real Stellr process rejected a clean window close.' }
  if (-not $Process.WaitForExit(15000)) { throw 'The real Stellr process did not exit after its window closed.' }
}

try {
  $existing = Get-Process -Name stellr -ErrorAction SilentlyContinue |
    Where-Object { $_.Path -eq $executable }
  if ($existing) { throw 'The application-process smoke requires no existing instance of this Stellr binary.' }

  $initialUrl = "https://github.com/$Repository/issues/$InitialIssue"
  $primary = Start-Stellr @('open', $initialUrl)
  Find-Element $primary $InitialIssueTitle | Out-Null
  if (Has-Element $primary 'Connect GitHub') {
    throw 'Authenticated application startup unexpectedly showed device authorization.'
  }

  $forwardedUrl = "https://github.com/$Repository/issues/$ForwardedIssue"
  $second = Start-Process -FilePath $executable -WorkingDirectory $workingDirectory -ArgumentList @('open', $forwardedUrl) -PassThru
  if (-not $second.WaitForExit(15000)) {
    Stop-Process -Id $second.Id -Force -ErrorAction SilentlyContinue
    throw 'The second instance did not forward its route and exit.'
  }
  if ($second.ExitCode -ne 0) { throw "The second instance exited with code $($second.ExitCode)." }
  Find-Element $primary $ForwardedIssueTitle | Out-Null

  [StellrWindowControl]::ShowWindowAsync($primary.MainWindowHandle, 6) | Out-Null
  Start-Sleep -Seconds 2
  [StellrWindowControl]::ShowWindowAsync($primary.MainWindowHandle, 9) | Out-Null
  [StellrWindowControl]::SetForegroundWindow($primary.MainWindowHandle) | Out-Null
  Start-Sleep -Seconds 2
  $primary.Refresh()
  if ($primary.HasExited) { throw 'Stellr exited while exercising focus cadence transitions.' }

  for ($attempt = 0; $attempt -lt 30; $attempt++) {
    if (Test-Path $routeFile) {
      $route = Get-Content $routeFile -Raw | ConvertFrom-Json
      if ($route.issue -eq $ForwardedIssue) { break }
    }
    Start-Sleep -Milliseconds 500
  }
  if ($route.issue -ne $ForwardedIssue) { throw 'The forwarded route was not persisted before clean exit.' }

  Close-Stellr $primary
  $primary = $null

  $primary = Start-Stellr @()
  Find-Element $primary $ForwardedIssueTitle | Out-Null
  Close-Stellr $primary
  $primary = $null

  Write-Output 'WINDOWS_DESKTOP_STARTUP_PASSED=true'
  Write-Output 'WINDOWS_AUTHENTICATED_SYNC_PASSED=true'
  Write-Output 'WINDOWS_SECOND_INSTANCE_ROUTING_PASSED=true'
  Write-Output 'WINDOWS_FOCUS_TRANSITION_PASSED=true'
  Write-Output 'WINDOWS_ROUTE_RESTORED_AFTER_RELAUNCH=true'
} finally {
  if ($null -ne $primary -and -not $primary.HasExited) {
    Stop-Process -Id $primary.Id -Force -ErrorAction SilentlyContinue
  }
}

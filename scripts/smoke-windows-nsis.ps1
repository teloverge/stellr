param(
  [Parameter(Mandatory = $true)]
  [string]$InstallerPath
)

$ErrorActionPreference = 'Stop'
$installer = (Resolve-Path $InstallerPath).Path
$appProcess = $null
$installedExecutable = $null
$uninstaller = $null

function Get-StellrUninstallRecord {
  $roots = @(
    'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\*',
    'HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall\*',
    'HKLM:\Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\*'
  )
  Get-ItemProperty $roots -ErrorAction SilentlyContinue |
    Where-Object { $_.DisplayName -eq 'Stellr' } |
    Select-Object -First 1
}

function Get-ExecutableFromCommand([string]$Command) {
  if ($Command -match '^\s*"([^"]+)"') { return $Matches[1] }
  if ($Command -match '^\s*([^\s]+\.exe)') { return $Matches[1] }
  throw "Could not parse executable path from command: $Command"
}

function Get-WebView2Version {
  $clientId = '{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}'
  $keys = @(
    "HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\$clientId",
    "HKLM:\SOFTWARE\Microsoft\EdgeUpdate\Clients\$clientId",
    "HKCU:\Software\Microsoft\EdgeUpdate\Clients\$clientId"
  )
  foreach ($key in $keys) {
    $version = (Get-ItemProperty -LiteralPath $key -Name pv -ErrorAction SilentlyContinue).pv
    if (-not [string]::IsNullOrWhiteSpace($version)) { return $version }
  }
  return $null
}

try {
  if ($null -ne (Get-StellrUninstallRecord)) {
    throw 'A Stellr installation already exists; refusing to overwrite it during a clean-runner smoke test.'
  }

  $install = Start-Process -FilePath $installer -ArgumentList '/S' -Wait -PassThru -WindowStyle Hidden
  if ($install.ExitCode -ne 0) { throw "NSIS installation failed with exit code $($install.ExitCode)." }

  $record = $null
  for ($attempt = 0; $attempt -lt 30; $attempt++) {
    $record = Get-StellrUninstallRecord
    if ($null -ne $record) { break }
    Start-Sleep -Milliseconds 500
  }
  if ($null -eq $record) { throw 'Stellr did not register an uninstaller.' }

  $uninstaller = Get-ExecutableFromCommand $record.UninstallString
  if (-not (Test-Path -LiteralPath $uninstaller)) { throw "Uninstaller is missing: $uninstaller" }

  if (-not [string]::IsNullOrWhiteSpace($record.DisplayIcon)) {
    $installedExecutable = Get-ExecutableFromCommand $record.DisplayIcon
  } elseif (-not [string]::IsNullOrWhiteSpace($record.InstallLocation)) {
    $installedExecutable = Join-Path $record.InstallLocation 'stellr.exe'
  }
  if ([string]::IsNullOrWhiteSpace($installedExecutable) -or -not (Test-Path -LiteralPath $installedExecutable)) {
    throw 'The installed Stellr executable could not be located.'
  }

  $webViewVersion = Get-WebView2Version
  if ([string]::IsNullOrWhiteSpace($webViewVersion)) {
    throw 'The Microsoft Edge WebView2 Runtime is not available after installation.'
  }

  $appProcess = Start-Process -FilePath $installedExecutable -PassThru
  $windowReady = $false
  for ($attempt = 0; $attempt -lt 60; $attempt++) {
    Start-Sleep -Milliseconds 500
    $appProcess.Refresh()
    if ($appProcess.HasExited) { throw "Installed Stellr exited early with code $($appProcess.ExitCode)." }
    if ($appProcess.MainWindowTitle -eq 'Stellr' -and $appProcess.MainWindowHandle -ne 0) {
      $windowReady = $true
      break
    }
  }
  if (-not $windowReady) { throw 'Installed Stellr did not show its native window.' }

  $webViewProcess = $null
  for ($attempt = 0; $attempt -lt 20; $attempt++) {
    $webViewProcess = Get-CimInstance Win32_Process -Filter "ParentProcessId = $($appProcess.Id)" |
      Where-Object { $_.Name -eq 'msedgewebview2.exe' } |
      Select-Object -First 1
    if ($null -ne $webViewProcess) { break }
    Start-Sleep -Milliseconds 250
  }
  if ($null -eq $webViewProcess) { throw 'The installed application did not launch a WebView2 child process.' }

  if (-not $appProcess.CloseMainWindow()) { throw 'Could not request a clean Stellr shutdown.' }
  if (-not $appProcess.WaitForExit(15000)) { throw 'Stellr did not exit after its window closed.' }
  $appProcess = $null

  $uninstall = Start-Process -FilePath $uninstaller -ArgumentList '/S' -Wait -PassThru -WindowStyle Hidden
  if ($uninstall.ExitCode -ne 0) { throw "NSIS uninstall failed with exit code $($uninstall.ExitCode)." }
  for ($attempt = 0; $attempt -lt 30; $attempt++) {
    if ($null -eq (Get-StellrUninstallRecord) -and -not (Test-Path -LiteralPath $installedExecutable)) { break }
    Start-Sleep -Milliseconds 500
  }
  if ($null -ne (Get-StellrUninstallRecord)) { throw 'Stellr uninstall registration remained after uninstall.' }
  if (Test-Path -LiteralPath $installedExecutable) { throw 'The Stellr executable remained after uninstall.' }
  $uninstaller = $null

  Write-Output "WEBVIEW2_VERSION=$webViewVersion"
  Write-Output 'WINDOWS_NSIS_SMOKE_PASSED=true'
} finally {
  if ($null -ne $appProcess -and -not $appProcess.HasExited) {
    Stop-Process -Id $appProcess.Id -Force -ErrorAction SilentlyContinue
  }
  if ($null -ne $uninstaller -and (Test-Path -LiteralPath $uninstaller)) {
    Start-Process -FilePath $uninstaller -ArgumentList '/S' -Wait -WindowStyle Hidden -ErrorAction SilentlyContinue
  }
}

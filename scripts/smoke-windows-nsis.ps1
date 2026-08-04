param(
  [Parameter(Mandatory = $true)]
  [string]$InstallerPath,
  [int]$StartupTimeoutSeconds = 90
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'windows-startup-diagnostics.ps1')
if ($StartupTimeoutSeconds -le 0) { throw 'StartupTimeoutSeconds must be positive.' }
$installer = (Resolve-Path $InstallerPath).Path
$appProcess = $null
$installedExecutable = $null
$uninstaller = $null
$routeFile = Join-Path ([Environment]::GetFolderPath('ApplicationData')) 'stellr\config\desktop-route.json'
$startupLogRoot = New-StellrStartupLogRoot "stellr-installed-startup-$PID"

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

function Get-StellrShortcutTarget {
  $programs = Join-Path ([Environment]::GetFolderPath('StartMenu')) 'Programs'
  $shortcut = Get-ChildItem -LiteralPath $programs -Filter 'Stellr.lnk' -File -Recurse |
    Select-Object -First 1
  if ($null -eq $shortcut) { throw 'The installed Stellr Start menu shortcut is missing.' }
  $shell = New-Object -ComObject WScript.Shell
  $shell.CreateShortcut($shortcut.FullName).TargetPath
}

try {
  Remove-Item -LiteralPath $routeFile -Force -ErrorAction SilentlyContinue
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
    $installedExecutable = Join-Path $record.InstallLocation 'stellr-desktop.exe'
  }
  if ([string]::IsNullOrWhiteSpace($installedExecutable) -or -not (Test-Path -LiteralPath $installedExecutable)) {
    throw 'The installed Stellr desktop executable could not be located.'
  }
  if ((Split-Path -Leaf $installedExecutable) -ne 'stellr-desktop.exe') {
    throw "The uninstall DisplayIcon does not target stellr-desktop.exe: $installedExecutable"
  }
  $installedCli = Join-Path (Split-Path -Parent $installedExecutable) 'stellr.exe'
  if (-not (Test-Path -LiteralPath $installedCli)) { throw 'The installed companion Stellr CLI is missing.' }

  $shortcutTarget = Get-StellrShortcutTarget
  if ((Resolve-Path $shortcutTarget).Path -ne (Resolve-Path $installedExecutable).Path) {
    throw "The Start menu shortcut does not target stellr-desktop.exe: $shortcutTarget"
  }
  $protocolKey = 'Registry::HKEY_CURRENT_USER\Software\Classes\stellr\shell\open\command'
  $protocolCommand = (Get-Item -LiteralPath $protocolKey -ErrorAction Stop).GetValue('')
  $protocolTarget = Get-ExecutableFromCommand $protocolCommand
  if ((Resolve-Path $protocolTarget).Path -ne (Resolve-Path $installedExecutable).Path) {
    throw "The stellr protocol does not target stellr-desktop.exe: $protocolCommand"
  }

  $webViewVersion = Get-WebView2Version
  if ([string]::IsNullOrWhiteSpace($webViewVersion)) {
    throw 'The Microsoft Edge WebView2 Runtime is not available after installation.'
  }

  $installDirectory = Split-Path -Parent $installedExecutable
  $startupLog = New-StellrStartupLog $startupLogRoot 'installed'
  $start = @{
    FilePath = $installedExecutable
    WorkingDirectory = $installDirectory
  }
  $appProcess = Start-StellrProcessWithDiagnostics $start $startupLog
  $appProcess = Wait-StellrDesktopWindow `
    $appProcess `
    $startupLog `
    $StartupTimeoutSeconds `
    'Installed Stellr exited during startup.' `
    "Installed Stellr did not show its native window within $StartupTimeoutSeconds seconds."

  $protocolUri = 'stellr://space?repo=teloverge%2Fstellr&issue=57'
  Start-Process -FilePath $protocolUri | Out-Null
  $protocolRoute = $null
  for ($attempt = 0; $attempt -lt 60; $attempt++) {
    if (Test-Path -LiteralPath $routeFile) {
      $protocolRoute = Get-Content -LiteralPath $routeFile -Raw | ConvertFrom-Json
      if ($protocolRoute.space -eq 'teloverge-stellr' -and $protocolRoute.issue -eq 57) { break }
    }
    Start-Sleep -Milliseconds 500
  }
  if ($null -eq $protocolRoute -or $protocolRoute.space -ne 'teloverge-stellr' -or $protocolRoute.issue -ne 57) {
    throw 'The installed stellr:// activation did not reach and persist the desktop route.'
  }
  $appProcess.Refresh()
  if ($appProcess.HasExited) { throw 'The installed desktop exited during protocol activation.' }

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
  Write-Output 'WINDOWS_DISPLAY_ICON_TARGET=stellr-desktop.exe'
  Write-Output 'WINDOWS_START_MENU_TARGET=stellr-desktop.exe'
  Write-Output 'WINDOWS_PROTOCOL_TARGET=stellr-desktop.exe'
  Write-Output 'WINDOWS_PROTOCOL_ACTIVATION_PASSED=true'
  Write-Output 'WINDOWS_COMPANION_CLI_INSTALLED=true'
  Write-Output 'WINDOWS_BARE_INSTALL_DIRECTORY_STARTUP_PASSED=true'
  Write-Output 'WINDOWS_NSIS_SMOKE_PASSED=true'
} finally {
  if ($null -ne $appProcess -and -not $appProcess.HasExited) {
    Stop-Process -Id $appProcess.Id -Force -ErrorAction SilentlyContinue
  }
  if ($null -ne $uninstaller -and (Test-Path -LiteralPath $uninstaller)) {
    Start-Process -FilePath $uninstaller -ArgumentList '/S' -Wait -WindowStyle Hidden -ErrorAction SilentlyContinue
  }
  Remove-Item -LiteralPath $routeFile -Force -ErrorAction SilentlyContinue
}

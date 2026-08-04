param(
  [Parameter(Mandatory = $true)]
  [string]$ExecutablePath,

  [Parameter(Mandatory = $true)]
  [ValidateSet('WindowsGui', 'WindowsCui')]
  [string]$ExpectedSubsystem
)

$ErrorActionPreference = 'Stop'
$executable = (Resolve-Path -LiteralPath $ExecutablePath).Path
$bytes = [IO.File]::ReadAllBytes($executable)

if ($bytes.Length -lt 0x40 -or $bytes[0] -ne 0x4d -or $bytes[1] -ne 0x5a) {
  throw "The file is not a valid DOS/PE image: $executable"
}

$peOffset = [BitConverter]::ToInt32($bytes, 0x3c)
if ($peOffset -lt 0 -or $peOffset + 24 + 0x46 -gt $bytes.Length) {
  throw "The PE header lies outside the file: $executable"
}
if ($bytes[$peOffset] -ne 0x50 -or $bytes[$peOffset + 1] -ne 0x45 -or
    $bytes[$peOffset + 2] -ne 0 -or $bytes[$peOffset + 3] -ne 0) {
  throw "The PE signature is invalid: $executable"
}

$optionalHeaderOffset = $peOffset + 24
$magic = [BitConverter]::ToUInt16($bytes, $optionalHeaderOffset)
if ($magic -notin @(0x10b, 0x20b)) {
  throw "Unsupported PE optional-header magic 0x$($magic.ToString('x4')): $executable"
}

$actual = [BitConverter]::ToUInt16($bytes, $optionalHeaderOffset + 0x44)
$expected = if ($ExpectedSubsystem -eq 'WindowsGui') { 2 } else { 3 }
if ($actual -ne $expected) {
  throw "Expected $ExpectedSubsystem ($expected), found subsystem $actual in $executable."
}

Write-Output "WINDOWS_PE_SUBSYSTEM=$ExpectedSubsystem"
Write-Output "WINDOWS_PE_EXECUTABLE=$executable"

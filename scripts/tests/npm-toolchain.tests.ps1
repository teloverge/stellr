$ErrorActionPreference = 'Stop'

$repo = Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
$package = Get-Content (Join-Path $repo 'web\package.json') -Raw | ConvertFrom-Json
$requiredVersion = $package.devEngines.packageManager.version

if ([string]::IsNullOrWhiteSpace($requiredVersion)) {
  throw 'web/package.json must declare devEngines.packageManager.version.'
}

$setupNeedle = '- uses: actions/setup-node@v6'
$installNeedle = "- run: npm install --global npm@$requiredVersion"
$ciNeedle = '- run: npm --prefix web ci'

function Test-NpmToolchainWorkflow {
  param(
    [Parameter(Mandatory)] [string] $Workflow,
    [Parameter(Mandatory)] [string] $Name
  )

  $jobMatches = [regex]::Matches($Workflow, '(?m)^  (?<name>[A-Za-z0-9_-]+):\r?$')
  $count = 0

  for ($jobIndex = 0; $jobIndex -lt $jobMatches.Count; $jobIndex++) {
    $jobMatch = $jobMatches[$jobIndex]
    $jobEnd = if ($jobIndex + 1 -lt $jobMatches.Count) {
      $jobMatches[$jobIndex + 1].Index
    } else {
      $Workflow.Length
    }
    $job = $Workflow.Substring($jobMatch.Index, $jobEnd - $jobMatch.Index)
    $jobName = $jobMatch.Groups['name'].Value
    $offset = 0

    while (($ciIndex = $job.IndexOf($ciNeedle, $offset, [System.StringComparison]::Ordinal)) -ge 0) {
      $count++
      $prefix = $job.Substring(0, $ciIndex)
      $setupIndex = $prefix.LastIndexOf($setupNeedle, [System.StringComparison]::Ordinal)
      $installIndex = $prefix.LastIndexOf($installNeedle, [System.StringComparison]::Ordinal)

      if ($setupIndex -lt 0) {
        throw "$Name job '$jobName' runs npm ci without actions/setup-node@v6."
      }
      if ($installIndex -lt $setupIndex) {
        throw "$Name job '$jobName' runs npm ci before activating npm $requiredVersion after its Node setup."
      }

      $offset = $ciIndex + $ciNeedle.Length
    }
  }

  return $count
}

$crossJobFixture = @"
jobs:
  configured:
    steps:
      $setupNeedle
      $installNeedle
      $ciNeedle
  missing-toolchain:
    steps:
      $ciNeedle
"@
$mutationRejected = $false
try {
  $null = Test-NpmToolchainWorkflow -Workflow $crossJobFixture -Name 'cross-job fixture'
} catch {
  if ($_.Exception.Message -like "*job 'missing-toolchain' runs npm ci*") {
    $mutationRejected = $true
  } else {
    throw
  }
}
if (-not $mutationRejected) {
  throw 'Contract self-test failed: a later job borrowed npm setup from an earlier job.'
}

$ciCount = 0
$workflowFiles = Get-ChildItem (Join-Path $repo '.github\workflows') -File |
  Where-Object Extension -In '.yml', '.yaml'
foreach ($workflowFile in $workflowFiles) {
  $workflow = Get-Content $workflowFile.FullName -Raw
  $ciCount += Test-NpmToolchainWorkflow -Workflow $workflow -Name $workflowFile.Name
}

$expectedCiCount = 9
if ($ciCount -ne $expectedCiCount) {
  throw "Expected $expectedCiCount GitHub workflow npm ci steps, found $ciCount."
}

Write-Output "NPM_TOOLCHAIN_CONTRACT_PASSED=true"
Write-Output "NPM_TOOLCHAIN_VERSION=$requiredVersion"
Write-Output "NPM_TOOLCHAIN_CI_STEPS=$ciCount"

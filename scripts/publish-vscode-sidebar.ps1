param(
  [string]$OutputPath = "pai-test.vsix",
  [switch]$SkipBuild,
  [switch]$SkipPackage,
  [switch]$SkipDuplicate,
  [switch]$PreRelease
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot
$extensionDir = Join-Path $repoRoot "src\features\sidebar\extension"
$packageScript = Join-Path $PSScriptRoot "package-vscode-sidebar.ps1"

if ([System.IO.Path]::IsPathRooted($OutputPath)) {
  $vsixPath = [System.IO.Path]::GetFullPath($OutputPath)
} else {
  $vsixPath = [System.IO.Path]::GetFullPath((Join-Path $extensionDir $OutputPath))
}

if (-not $SkipPackage) {
  $packageArgs = @("-OutputPath", $vsixPath)
  if ($SkipBuild) {
    $packageArgs += "-SkipBuild"
  }
  & $packageScript @packageArgs
}

if (-not (Test-Path -LiteralPath $vsixPath)) {
  throw "VSIX package not found: $vsixPath"
}

Write-Host "[vscode-sidebar] Publishing to Visual Studio Marketplace with the local vsce login..."

Push-Location $extensionDir
try {
  $vsceArgs = @(
    "dlx",
    "@vscode/vsce",
    "publish",
    "--packagePath",
    $vsixPath,
    "--allow-missing-repository",
    "--skip-license"
  )
  if ($SkipDuplicate) {
    $vsceArgs += "--skip-duplicate"
  }
  if ($PreRelease) {
    $vsceArgs += "--pre-release"
  }

  & pnpm @vsceArgs
  if ($LASTEXITCODE -ne 0) {
    throw "vsce publish failed with exit code $LASTEXITCODE."
  }
}
finally {
  Pop-Location
}

Write-Host "[vscode-sidebar] Published package: $vsixPath"

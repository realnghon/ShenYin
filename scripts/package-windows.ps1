param(
    [string]$Target = "x86_64-pc-windows-msvc",
    [string]$Profile = "release",
    [string]$OutputDir = "dist"
)

$ErrorActionPreference = "Stop"

$binaryPath = Join-Path -Path "target" -ChildPath "$Target\$Profile\shenyin.exe"
if (-not (Test-Path -LiteralPath $binaryPath)) {
    throw "Expected compiled Windows binary at '$binaryPath'."
}

New-Item -ItemType Directory -Force -Path $OutputDir | Out-Null

$artifactPath = Join-Path -Path $OutputDir -ChildPath "ShenYin.exe"
if (Test-Path -LiteralPath $artifactPath) {
    Remove-Item -LiteralPath $artifactPath -Force
}

Copy-Item -LiteralPath $binaryPath -Destination $artifactPath -Force
Write-Host "Packaged Windows artifact at $artifactPath"

param([string]$NodePath)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$appRoot = Join-Path $projectRoot 'app'
if (!$NodePath) { $NodePath = (Get-Command node.exe -ErrorAction Stop).Source }
Push-Location $appRoot
try {
    & $NodePath 'node_modules\typescript\bin\tsc' --noEmit
    if ($LASTEXITCODE -ne 0) { throw 'TypeScript check failed' }
    & $NodePath 'node_modules\vite\bin\vite.js' build
    if ($LASTEXITCODE -ne 0) { throw 'Frontend build failed' }
} finally { Pop-Location }
& (Join-Path $PSScriptRoot 'Invoke-Cargo.ps1') -CargoArguments @('build', '--manifest-path', (Join-Path $appRoot 'src-tauri\Cargo.toml'), '--features', 'tauri/custom-protocol')
if ($LASTEXITCODE -ne 0) { throw 'Desktop build failed' }
$output = Join-Path $projectRoot 'artifacts'
New-Item -ItemType Directory -Force $output | Out-Null
Copy-Item -LiteralPath (Join-Path $appRoot 'src-tauri\target\debug\ag-switch-pro.exe') -Destination $output -Force
& (Join-Path $PSScriptRoot 'Copy-WebViewRuntime.ps1') -Destination $output
$strip = Join-Path $PSScriptRoot 'mingw-compatible\mingw64\bin\strip.exe'
& $strip '--strip-debug' (Join-Path $output 'ag-switch-pro.exe')
if ($LASTEXITCODE -ne 0) { throw 'Removing debug symbols from the portable copy failed.' }
Set-Content -LiteralPath (Join-Path $output 'portable.ini') -Value '[portable]' -Encoding ascii
Write-Output "Built portable executable: $output\ag-switch-pro.exe"

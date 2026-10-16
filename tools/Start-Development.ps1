param([string]$NodePath)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$appRoot = Join-Path $projectRoot 'app'
if (!$NodePath) { $NodePath = (Get-Command node.exe -ErrorAction Stop).Source }
$vitePath = Join-Path $appRoot 'node_modules\vite\bin\vite.js'
$frontend = Start-Process -FilePath $NodePath -ArgumentList ('"' + $vitePath + '"') -WorkingDirectory $appRoot -WindowStyle Hidden -PassThru
try {
    & (Join-Path $PSScriptRoot 'Invoke-Cargo.ps1') -CargoArguments @('build', '--manifest-path', (Join-Path $appRoot 'src-tauri\Cargo.toml'))
    if ($LASTEXITCODE -ne 0) { throw 'Development build failed.' }
    $backendDirectory = Join-Path $appRoot 'src-tauri\target\debug'
    & (Join-Path $PSScriptRoot 'Copy-WebViewRuntime.ps1') -Destination $backendDirectory
    & (Join-Path $backendDirectory 'ag-switch-pro.exe')
    if ($LASTEXITCODE -ne 0) { throw 'Development process failed.' }
} finally {
    if (!$frontend.HasExited) { Stop-Process -Id $frontend.Id }
}

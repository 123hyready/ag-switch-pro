param([Parameter(Mandatory = $true)][string]$Destination)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$buildRoot = Join-Path $projectRoot 'app\src-tauri\target\debug\build'
$loader = Get-ChildItem -Path (Join-Path $buildRoot 'webview2-com-sys-*\out\x64\WebView2Loader.dll') |
    Sort-Object LastWriteTime -Descending | Select-Object -First 1
if (!$loader) { throw 'Build the backend first to obtain its x64 WebView2 loader.' }
New-Item -ItemType Directory -Path $Destination -Force | Out-Null
Copy-Item -LiteralPath $loader.FullName -Destination $Destination -Force

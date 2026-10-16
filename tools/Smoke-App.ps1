$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$artifactRoot = Join-Path $projectRoot 'artifacts'
$executable = Join-Path $artifactRoot 'ag-switch-pro.exe'
if (!(Test-Path -LiteralPath $executable)) { throw 'Build the desktop application first.' }
if (Get-Process -Name 'ag-switch-pro' -ErrorAction SilentlyContinue) {
    throw 'Close the existing AG Switch Pro process before the isolated smoke test.'
}
$testRoot = Join-Path $artifactRoot ('smoke-' + [guid]::NewGuid().ToString('N'))
$configRoot = Join-Path $testRoot '.ag-switch-pro'
New-Item -ItemType Directory -Force $configRoot | Out-Null
Set-Content -LiteralPath (Join-Path $configRoot 'settings.json') -Value '{"silentStartup":true,"showInTray":false}' -Encoding utf8
$savedTestHome = $env:CC_SWITCH_TEST_HOME
$savedWebViewData = $env:WEBVIEW2_USER_DATA_FOLDER
$process = $null
try {
    $env:CC_SWITCH_TEST_HOME = $testRoot
    $env:WEBVIEW2_USER_DATA_FOLDER = Join-Path $testRoot 'webview-data'
    $process = Start-Process -FilePath $executable -WorkingDirectory $artifactRoot -WindowStyle Hidden -PassThru
    Start-Sleep -Seconds 8
    $process.Refresh()
    if ($process.HasExited) { throw "Desktop process exited during startup: $($process.ExitCode)" }
    $database = Join-Path $configRoot 'ag-switch-pro.db'
    if (!(Test-Path -LiteralPath $database)) { throw 'Desktop startup did not initialize the isolated database.' }
    Write-Output "Desktop process remained alive and initialized its database: $database"
    Write-Output 'This check does not verify rendered UI or real model calls.'
} finally {
    if ($process -and !$process.HasExited) { Stop-Process -Id $process.Id }
    $env:CC_SWITCH_TEST_HOME = $savedTestHome
    $env:WEBVIEW2_USER_DATA_FOLDER = $savedWebViewData
}

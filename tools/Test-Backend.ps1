param([string[]]$Filters = @('proxy::intent', 'mode::foreign_proxy'))
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
& (Join-Path $PSScriptRoot 'Invoke-Cargo.ps1') -CargoArguments @(
    'test', '--manifest-path', (Join-Path $projectRoot 'app\src-tauri\Cargo.toml'), '--lib', '--no-run'
)
if ($LASTEXITCODE -ne 0) { throw 'Backend test compilation failed.' }
$testBinary = Get-ChildItem (Join-Path $projectRoot 'app\src-tauri\target\debug\deps') -Filter 'cc_switch_lib-*.exe' |
    Sort-Object LastWriteTime -Descending | Select-Object -First 1
if (!$testBinary) { throw 'Backend test executable was not generated.' }
# GNU unit tests do not inherit Tauri's main executable manifest.
& (Join-Path $PSScriptRoot 'Set-TestManifest.ps1') -Executable $testBinary.FullName
& (Join-Path $PSScriptRoot 'Copy-WebViewRuntime.ps1') -Destination $testBinary.DirectoryName
foreach ($filter in $Filters) {
    & $testBinary.FullName $filter '--test-threads=1'
    if ($LASTEXITCODE -ne 0) { throw "Backend tests failed: $filter (exit $LASTEXITCODE)" }
}

$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$coverageDir = Join-Path $projectRoot 'artifacts\coverage-core'
New-Item -ItemType Directory -Force $coverageDir | Out-Null
$env:AG_EXTRA_RUSTFLAGS = '-C instrument-coverage'
$env:LLVM_PROFILE_FILE = Join-Path $coverageDir '%p-%m.profraw'
& (Join-Path $PSScriptRoot 'Invoke-Cargo.ps1') -CargoArguments @('test', '--manifest-path', (Join-Path $projectRoot 'app\crates\intent-router\Cargo.toml'))
if ($LASTEXITCODE -ne 0) { throw 'Core coverage tests failed' }
$llvmBin = Join-Path $PSScriptRoot 'rust\lib\rustlib\x86_64-pc-windows-gnu\bin'
$profilePath = Join-Path $coverageDir 'core.profdata'
$profiles = @(Get-ChildItem -LiteralPath $coverageDir -Filter '*.profraw' | ForEach-Object FullName)
& (Join-Path $llvmBin 'llvm-profdata.exe') merge -sparse @profiles -o $profilePath
if ($LASTEXITCODE -ne 0) { throw 'Profile merge failed' }
$testBinary = Get-ChildItem (Join-Path $projectRoot 'app\crates\intent-router\target\debug\deps') -Filter 'routing-*.exe' |
    Sort-Object LastWriteTime -Descending | Select-Object -First 1
$coreSource = Join-Path $projectRoot 'app\crates\intent-router\src\lib.rs'
& (Join-Path $llvmBin 'llvm-cov.exe') report $testBinary.FullName "-instr-profile=$profilePath" $coreSource |
    Tee-Object -FilePath (Join-Path $coverageDir 'report.txt')
& (Join-Path $llvmBin 'llvm-cov.exe') export $testBinary.FullName "-instr-profile=$profilePath" $coreSource |
    Set-Content (Join-Path $coverageDir 'coverage.json') -Encoding utf8NoBOM

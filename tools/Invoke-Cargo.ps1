param([Parameter(ValueFromRemainingArguments = $true)][string[]]$CargoArguments)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$rustBin = Join-Path $PSScriptRoot 'rust\bin'
$mingwBin = Join-Path $PSScriptRoot 'mingw-compatible\mingw64\bin'
if (!(Test-Path (Join-Path $mingwBin 'gcc.exe'))) {
    throw 'Run tools/Prepare-Toolchain.ps1 first; this Rust target requires the matching MSVCRT compiler.'
}
$env:CARGO_HOME = Join-Path $PSScriptRoot 'cargo-home'
$env:RUSTC = Join-Path $rustBin 'rustc.exe'
$env:RUSTDOC = Join-Path $rustBin 'rustdoc.exe'
$env:RUSTFMT = Join-Path $rustBin 'rustfmt.exe'
$env:CC = Join-Path $mingwBin 'gcc.exe'
$env:CXX = Join-Path $mingwBin 'g++.exe'
$env:AR = Join-Path $mingwBin 'ar.exe'
$env:RANLIB = Join-Path $mingwBin 'ranlib.exe'
$env:RC = Join-Path $mingwBin 'windres.exe'
$env:CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER = $env:CC
$env:CFLAGS = "-B$($mingwBin.Replace('\', '/'))/"
$env:CXXFLAGS = $env:CFLAGS
$rustRuntime = Join-Path $PSScriptRoot 'rust\lib\rustlib\x86_64-pc-windows-gnu\lib\self-contained'
$env:RUSTFLAGS = "-C link-self-contained=yes -C link-arg=-B$($mingwBin.Replace('\', '/'))/ -L native=$($rustRuntime.Replace('\', '/'))"
if ($env:AG_EXTRA_RUSTFLAGS) { $env:RUSTFLAGS += " $env:AG_EXTRA_RUSTFLAGS" }
if (!(Test-Path -LiteralPath $env:RUSTC)) { throw 'Run tools/Prepare-Toolchain.ps1 first.' }
& (Join-Path $rustBin 'cargo.exe') @CargoArguments
exit $LASTEXITCODE

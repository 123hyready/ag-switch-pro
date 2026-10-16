param([string]$RustMirror = 'https://rsproxy.cn', [string]$CompilerMirror = 'https://ghfast.top')
$ErrorActionPreference = 'Stop'
$rustVersion = '1.95.0'
$target = 'x86_64-pc-windows-gnu'
$downloads = Join-Path $PSScriptRoot 'downloads'
New-Item -ItemType Directory -Force $downloads | Out-Null
$manifestPath = Join-Path $downloads "channel-rust-$rustVersion.toml"
if (!(Test-Path -LiteralPath $manifestPath)) {
    Invoke-WebRequest "https://static.rust-lang.org/dist/channel-rust-$rustVersion.toml" -OutFile $manifestPath
}
$manifestText = Get-Content -LiteralPath $manifestPath -Raw
$sectionPattern = '(?ms)^\[pkg\.rust\.target\.' + [regex]::Escape($target) + '\]\r?\n(.*?)(?=^\[)'
$targetSection = [regex]::Match($manifestText, $sectionPattern).Groups[1].Value
$downloadUrl = [regex]::Match($targetSection, 'xz_url = "([^"]+)"').Groups[1].Value
$expectedHash = [regex]::Match($targetSection, 'xz_hash = "([^"]+)"').Groups[1].Value
if (!$downloadUrl -or !$expectedHash) { throw 'Official manifest has no matching archive.' }
$archivePath = Join-Path $downloads "rust-$rustVersion-$target.tar.xz"
if (!(Test-Path -LiteralPath $archivePath)) {
    $mirrorUrl = $RustMirror.TrimEnd('/') + ([uri]$downloadUrl).PathAndQuery
    & "$env:SystemRoot\System32\curl.exe" -L --fail --retry 2 -o $archivePath $mirrorUrl
    if ($LASTEXITCODE -ne 0) { throw 'Rust archive download failed.' }
}
if ((Get-FileHash -LiteralPath $archivePath -Algorithm SHA256).Hash.ToLowerInvariant() -ne $expectedHash) {
    throw 'Rust archive checksum mismatch.'
}
$extractRoot = Join-Path $PSScriptRoot 'rust-dist'
$rustRoot = Join-Path $PSScriptRoot 'rust'
New-Item -ItemType Directory -Force $extractRoot, $rustRoot | Out-Null
& "$env:SystemRoot\System32\tar.exe" -xf $archivePath -C $extractRoot --exclude '*/rust-docs*'
if ($LASTEXITCODE -ne 0) { throw 'Rust archive extraction failed.' }
$distribution = Join-Path $extractRoot "rust-$rustVersion-$target"
foreach ($component in @('rustc', 'cargo', "rust-std-$target", 'rustfmt-preview', 'clippy-preview', 'rust-mingw', 'llvm-tools-preview')) {
    $componentDir = Join-Path $distribution $component
    if (Test-Path -LiteralPath $componentDir) {
        Get-ChildItem -LiteralPath $componentDir -Directory | ForEach-Object {
            Copy-Item -LiteralPath $_.FullName -Destination $rustRoot -Recurse -Force
        }
    }
}
$mingwRoot = Join-Path $PSScriptRoot 'mingw-compatible'
$compilerName = 'winlibs-x86_64-posix-seh-gcc-13.3.0-mingw-w64msvcrt-11.0.1-r1.7z'
$compilerUrl = "https://github.com/brechtsanders/winlibs_mingw/releases/download/13.3.0posix-11.0.1-msvcrt-r1/$compilerName"
$compilerArchive = Join-Path $downloads 'mingw-msvcrt.7z'
$compilerHashPath = Join-Path $downloads 'mingw-msvcrt.sha256'
if (!(Test-Path -LiteralPath $compilerHashPath)) {
    & "$env:SystemRoot\System32\curl.exe" -L --fail -o $compilerHashPath ($compilerUrl + '.sha256')
    if ($LASTEXITCODE -ne 0) { throw 'Official compiler checksum download failed.' }
}
$compilerHash = [regex]::Match((Get-Content -LiteralPath $compilerHashPath -Raw), '[a-fA-F0-9]{64}').Value.ToLowerInvariant()
if (!(Test-Path -LiteralPath $compilerArchive)) {
    & "$env:SystemRoot\System32\curl.exe" -L --fail --retry 2 -o $compilerArchive ($CompilerMirror.TrimEnd('/') + '/' + $compilerUrl)
    if ($LASTEXITCODE -ne 0) { throw 'Compiler archive download failed.' }
}
if ((Get-FileHash -LiteralPath $compilerArchive -Algorithm SHA256).Hash.ToLowerInvariant() -ne $compilerHash) {
    throw 'Compiler archive checksum mismatch.'
}
New-Item -ItemType Directory -Force $mingwRoot | Out-Null
& "$env:SystemRoot\System32\tar.exe" -xf $compilerArchive -C $mingwRoot
if ($LASTEXITCODE -ne 0) { throw 'Compiler archive extraction failed.' }
New-Item -ItemType Directory -Force (Join-Path $PSScriptRoot 'cargo-home') | Out-Null
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'cargo-config.toml') -Destination (Join-Path $PSScriptRoot 'cargo-home\config.toml') -Force
& (Join-Path $rustRoot 'bin\rustc.exe') --version
& (Join-Path $mingwRoot 'mingw64\bin\gcc.exe') --version | Select-Object -First 1
Write-Output 'Portable toolchain ready. No PATH was changed.'

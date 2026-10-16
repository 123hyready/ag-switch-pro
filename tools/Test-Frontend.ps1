param([string]$NodePath, [switch]$All)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
if (!$NodePath) { $NodePath = (Get-Command node.exe -ErrorAction Stop).Source }
$artifactRoot = Join-Path $projectRoot 'artifacts'
New-Item -ItemType Directory -Force $artifactRoot | Out-Null
$arguments = @('node_modules/vitest/vitest.mjs', 'run', '--pool=forks', '--maxWorkers=1', '--minWorkers=1')
if ($All) {
    $arguments += @('--silent', '--reporter=json', "--outputFile=$artifactRoot/frontend-all.json")
} else {
    $arguments += @('src/lib/intent.test.ts', 'src/components/intent/IntentRoutingPage.test.tsx',
        '--coverage', '--coverage.include=src/lib/intent.ts',
        '--coverage.include=src/components/intent/IntentRoutingPage.tsx')
}
Push-Location (Join-Path $projectRoot 'app')
try {
    & $NodePath @arguments
    if ($LASTEXITCODE -ne 0) { throw 'Frontend tests failed.' }
} finally { Pop-Location }

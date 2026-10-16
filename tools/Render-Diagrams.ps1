param([string]$JavaPath, [ValidateSet('svg','png')][string]$Format = 'svg')
$ErrorActionPreference = 'Stop'
if (!$JavaPath) { $JavaPath = (Get-Command java.exe -ErrorAction Stop).Source }
$jar = Join-Path $PSScriptRoot 'plantuml\plantuml.jar'
if (!(Test-Path -LiteralPath $jar)) { throw 'Place the official PlantUML JAR in tools/plantuml/plantuml.jar.' }
$projectRoot = Split-Path -Parent $PSScriptRoot
$arguments = @('-Djava.awt.headless=true', '-jar', $jar, '-charset', 'UTF-8', "-t$Format",
    (Join-Path $projectRoot 'docs\progress.puml'), (Join-Path $projectRoot 'docs\intent-routing-flow.puml'))
& $JavaPath @arguments
if ($LASTEXITCODE -ne 0) { throw 'PlantUML rendering failed.' }

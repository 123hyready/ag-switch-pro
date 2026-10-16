param([string]$SourceExecutable)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$artifactRoot = Join-Path $projectRoot 'artifacts'
$packageRoot = Join-Path $artifactRoot 'ag-switch-pro-0.1.0-windows-x64-preview'
New-Item -ItemType Directory -Force $packageRoot | Out-Null
foreach ($filename in @('ag-switch-pro.exe', 'WebView2Loader.dll', 'portable.ini')) {
    $source = Join-Path $artifactRoot $filename
    if ($filename -eq 'ag-switch-pro.exe' -and $SourceExecutable) { $source = $SourceExecutable }
    if (!(Test-Path -LiteralPath $source)) { throw "Missing build output: $filename" }
    Copy-Item -LiteralPath $source -Destination (Join-Path $packageRoot $filename) -Force
}
Copy-Item -LiteralPath (Join-Path $projectRoot 'app\LICENSE') -Destination (Join-Path $packageRoot 'LICENSE-cc-switch.txt') -Force
Copy-Item -LiteralPath (Join-Path $projectRoot 'third_party\LICENSE-magpie.txt') -Destination (Join-Path $packageRoot 'LICENSE-magpie.txt') -Force
$readme = @'
AG Switch Pro 0.1.0 — Windows x64 开发预览

解压后运行 ag-switch-pro.exe，保持 WebView2Loader.dll 与程序处于同一目录。需要本机已有 Microsoft WebView2 Runtime。此版本没有安装器、签名或自动更新渠道。

先添加 Claude Code 或 Codex 供应商，再在「意图路由」中配置一个判断模型和至少一个工作模型。保存后点「启动客户端路由」，在客户端的模型选择菜单中选择路由组。Codex 模型目录变化后重启客户端。

Codex 支持 OpenAI Official 使用现有 Codex 登录：未绑定托管账号时从当前 Codex 配置目录的 auth.json 读取并刷新 ChatGPT 登录，判断与工作请求共用该认证。选择供应商后自动获取模型列表，工作模型可应用供应商返回的上下文及图片能力。没有落盘 auth.json 的系统钥匙串登录暂不支持。

「试判断」会真实调用判断模型并计费，但不执行工作任务。节省比例取决于判断模型和工作模型的费用及任务分布。切换发生在模型请求边界；完整历史重放会去掉不可跨模型复用的加密推理记录，保留消息和工具结果。服务端增量或压缩状态仍需保持原模型。旧会话若只有压缩状态，请新建会话使用路由组。

应用数据目录：~/.ag-switch-pro；代理默认端口：15722。portable.ini 识别便携分发，不会把应用数据自动移入解压目录。如 CC Switch 或 magpie 已接管客户端，先在对应应用恢复直连。

基于 cc-switch 96d636ca；意图路由借鉴 magpie e21ea07e。MIT 许可证见同目录文件。项目源码、完整验证记录和 PlantUML 源文件位于源项目目录 D:/ag-switch-pro。
'@
Set-Content -LiteralPath (Join-Path $packageRoot 'README.txt') -Value $readme -Encoding utf8
$checksums = foreach ($filename in @('ag-switch-pro.exe', 'WebView2Loader.dll')) {
    $hash = (Get-FileHash -LiteralPath (Join-Path $packageRoot $filename) -Algorithm SHA256).Hash.ToLowerInvariant()
    "$hash  $filename"
}
Set-Content -LiteralPath (Join-Path $packageRoot 'SHA256SUMS.txt') -Value $checksums -Encoding ascii
$archive = $packageRoot + '.zip'
Compress-Archive -Path (Join-Path $packageRoot '*') -DestinationPath $archive -Force
$archiveHash = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
Set-Content -LiteralPath ($archive + '.sha256') -Value "$archiveHash  $([IO.Path]::GetFileName($archive))" -Encoding ascii
Write-Output "Portable archive: $archive"

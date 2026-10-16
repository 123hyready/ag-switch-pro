# AG Switch Pro

基于 cc-switch 的桌面应用，增加由一个判断模型和一个或多个工作模型组成的意图路由组。客户端选择路由组后，模型请求可以按执行阶段分流，连续请求复用判断，判断调用费用进入同一用量账本。

## 目录

- `app/`：派生应用，Tauri 2 + React + Rust。
- `app/crates/intent-router/`：可独立测试的路由状态机。
- `tools/`：项目内便携编译器、依赖缓存和启动脚本。
- `docs/ag-switch-pro-plan.md`：已确认的设计方案。
- `cc-switch/`、`magpie/`：本地参考仓库，未修改。

## 环境

Rust 使用官方 1.95.0 Windows GNU 压缩包。编译器与缓存位于项目目录，通过绝对路径调用，不修改 PATH。Node.js 使用本机已有环境；所有前端依赖在项目内。便携 MinGW 必须与 Rust 的 MSVCRT 运行库匹配。

```powershell
# 独立检查与测试核心
./tools/Invoke-Cargo.ps1 -CargoArguments @('test', '--manifest-path', 'app/crates/intent-router/Cargo.toml')

# 原生代理与冲突保护测试（自动补齐测试清单和 WebView2 DLL）
./tools/Test-Backend.ps1

# 新功能前端测试与覆盖率；加 -All 运行完整回归
./tools/Test-Frontend.ps1

# 构建桌面可执行文件，输出 artifacts/ag-switch-pro.exe
./tools/Build-App.ps1

# 在项目内独立数据目录静默启动，验证进程与数据库后关闭测试程序
./tools/Smoke-App.ps1

# 压缩便携程序和运行库
./tools/Package-App.ps1

# 开发运行：启动前端服务和桌面程序
./tools/Start-Development.ps1
```

带 `--` 的 Cargo 参数使用数组，以避免 PowerShell 吞掉分隔符：

```powershell
./tools/Test-Backend.ps1 -Filters @('proxy::intent', 'mode::foreign_proxy', 'database::schema', 'proxy::forwarder::tests')
```

## 使用

1. 添加 Claude Code 或 Codex 的供应商。
2. 在「意图路由」中配置一个判断模型、工作模型、能力说明、上下文容量、默认模型与回退模型。
3. 保存路由组，再点「启动客户端路由」。首次启用会接管客户端连接配置；「恢复直连」撤回接管。
4. 在客户端 `/model` 中选择该路由组。Codex 模型目录变化后需要重启客户端。
5. 查看路由记录与用量统计。试判断会调用真实判断模型并计费，但不会执行任务。

阶段模式可以在一条用户任务内重新判断；每轮模式在用户新消息到来时判断；观察模式执行默认模型并记录建议，仍有判断费用。手动固定优先于自动判断。

Codex 路由组支持选择 OpenAI Official 并使用现有 Codex 登录。未绑定托管账号时，判断模型与工作模型共同读取当前 Codex 配置目录的 `auth.json`，自动刷新到期令牌；无需在 AG Switch Pro 重新登录或填写 API Key。已绑定托管账号时使用绑定账号。模型字段须填写实际模型 ID；仅存于系统钥匙串、未落盘的凭据暂不支持。详见[认证约定](docs/intent-routing-contract.md)。

程序身份为 `com.agswitchpro.desktop`，默认代理端口 `15722`，数据目录 `~/.ag-switch-pro`。未配置本项目的签名发布源，因此禁用继承的应用自更新。检测到 CC Switch 或 magpie 已接管时，先让对应应用恢复直连。

## 已知边界

- 路由发生在模型请求边界，无法切换一次响应内部的推理步骤。
- 阶段由公开对话与工具结果推断，不保证每次选择都是最优模型。
- 未知会话或子 Agent 分支身份不共享阶段状态。
- 不能移交的加密、压缩或增量状态保持原目标；未知来源拒绝自动转移。
- 输出已经开始后不重放请求到另一个模型。
- 上下文检查采用保守估算；实际费用来自供应商用量与本地定价，缺失价格显示未知。
- 验证记录将模拟上游测试与真实模型联调分开，不能据模拟结果宣称真实节省率。

## 汇报与验证

- 回退冷却修复已通过；已按后续要求重新构建最新源码并更新 `artifacts` 便携程序与 ZIP。Windows EXE 为图形界面程序，前端已内置，双击运行不会额外打开控制台。后续运行与真实模型验证交由用户执行。
- [阶段费曼汇报](docs/phase-report-2026-10-14.md)与 [PlantUML 进度图](docs/progress.puml)。
- [接口约定](docs/intent-routing-contract.md)、[工具来源](docs/toolchain-provenance.md)与 [验证记录](docs/validation.md)。
- 便携程序是包含前端的开发预览，需同目录的 `WebView2Loader.dll` 和机器已有的 WebView2 Runtime。`portable.ini` 用于识别便携分发，应用数据仍使用上述独立用户目录。

## 来源与许可

基于 [cc-switch](https://github.com/farion1231/cc-switch) 本地提交 `96d636ca`；意图路由借鉴 [magpie](https://github.com/yetone/magpie) 本地提交 `e21ea07e` 的设计。保留 MIT 许可证及原作者版权；本项目未修改两个参考仓库。

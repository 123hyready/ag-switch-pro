# 验证与交接记录：2026-10-14

完成回退冷却回归后，按用户后续要求重新构建最新源码。当前桌面可执行文件与便携 ZIP 均包含最新修复，Windows EXE 为图形界面程序，前端已内置。后续运行与真实模型验证交由用户执行。

## 最终测试结果

| 范围 | 结果 | 证据 |
|---|---|---|
| 独立路由核心 | 24 通过，0 失败 | `artifacts/core-tests.log` |
| 代理接入（本地模拟上游） | 6 通过，0 失败 | `artifacts/backend-final.log` |
| 外部代理接管检测 | 1 通过，0 失败 | 同上 |
| 数据库迁移回归 | 7 通过，0 失败 | 同上 |
| 既有转发器回归 | 128 通过，0 失败 | 同上 |
| 完整前端 | 2525 通过，0 失败 | `artifacts/frontend-all.json` |
| 新增前端覆盖率 | 10 项通过；行/语句 95.76%，分支 91.30%，函数 82.92% | `artifacts/frontend-intent-coverage-final.log`、`app/coverage/lcov.info` |

相关后端合计 142 项通过。没有运行完整的全部 Rust 后端套件，也没有获得 Rust 覆盖率：GNU 标准库缺少 `profiler_builtins`。以上数据不代表外部供应商兼容性或真实节省率。

## 回退冷却回归

新增 `execution_fallback_does_not_revisit_a_cooling_worker`：先把配置的回退模型置于失败冷却，再解析下一次路由请求。修复前目标名单仍包含该模型，断言预期 1 个目标、实际 2 个，测试失败，记录于 `artifacts/backend-cooldown-red.log`。

修复后执行回退名单与判断候选共用相同的冷却状态。该测试与相关后端回归全部通过，记录于 `artifacts/backend-final.log`。

## 前端回归中的超时处理

首次完整串行回归为 2522/2525：App 集成文件的模块冷启动占用 10 秒操作时限，随后未完成的异步测试产生重复 DOM。该文件单独复查为 20/20。把模块加载移到 `beforeAll`，保持操作原有的 10 秒时限，完整重跑后 2525/2525。

先前失败的 JSON 保留为 `artifacts/frontend-all-before-harness-fix.json`。新增页面测试的异步状态更新也已使用 `act` 等待。

## 桌面预览构建

前端 TypeScript 与 Vite 生产构建通过；Windows GNU 桌面构建通过。输出携带 x64 WebView2Loader.dll，便携副本移除了调试符号。

隔离启动检查静默运行 8 秒，进程存活并创建项目内测试数据库。没有验证桌面页面视觉效果或真实模型调用。证据：`artifacts/desktop-build-final.log`、`artifacts/desktop-smoke.log`。

上述隔离启动结果早于最后的回退冷却修复。随后已完成最新源码的 TypeScript 检查、前端生产构建及桌面重建，并更新便携 ZIP。检查 PE 头确认为 x64 Windows GUI 子系统；ZIP 内 EXE 的 SHA256 与最新构建一致，且包含 WebView2Loader.dll。新增证据：`artifacts/desktop-rebuild.log`、`artifacts/desktop-repackage.log`、`artifacts/desktop-exe-check.json`。

本次重建后没有再进行桌面运行或真实模型联调，后续由用户验证。双击 `artifacts/ag-switch-pro.exe`，或者解压 `artifacts/ag-switch-pro-0.1.0-windows-x64-preview.zip` 后双击其中的 EXE；保持 DLL 与 EXE 在同一目录。

后续再次修改源码时可以执行：

```powershell
& 'D:\ag-switch-pro\tools\Build-App.ps1' -NodePath 'E:\nodejs\node.exe'
```

构建后如需重新压缩：

```powershell
& 'D:\ag-switch-pro\tools\Package-App.ps1'
```

真实判断模型、工作模型、流式输出和实际费用对比由用户继续验证。保存路由组不会自动接管客户端；启用路由需在页面启动客户端路由并选择虚拟模型。

## 后续功能：现有 Codex 登录接入（2026-10-14）

## 后续修复与模型发现（2026-10-14）

修复空基础目录生成虚拟模型时缺少 `support_verbosity` 等必填字段的问题，改用完整原生 Responses 模板并保留基础目录自定义内容。已备份并修复当前托管目录，Codex CLI 配置读取对比：旧目录退出码 1、报相同缺失字段；修复后退出码 0。证据：`artifacts/catalog-parser-check.json` 及前后解析日志。将“智能编码”路由组内三个显示名称改成已由本机缓存确认的实际 ID，推进修订号；备份为 `artifacts/coding-route-before-model-id-fix.json`。

增加路由页面自动模型发现、下拉选择、搜索建议、刷新及错误提示。支持 Codex 原生/托管登录与 API Key 供应商；其他托管认证类型明确提示尚未接入。后端意图路由 10、Codex 模型解析 5、通用模型发现 29，共 44 项通过；前端 12 项通过。TypeScript、生产前端和桌面构建通过。新程序为 `artifacts/ag-switch-pro-models.exe`，ZIP 内以 `ag-switch-pro.exe` 分发，SHA256 一致。旧程序仍运行，故另存新版，没有再次进行桌面启动或真实供应商请求。证据采用 `artifacts/model-discovery-*` 前缀。

## 原生登录接入验证记录

按用户新的授权对齐 Magpie 的文件登录接入。Codex OpenAI Official 未绑定托管账号时，路由组允许保存，判断与工作请求读取现有 `auth.json`，提前刷新令牌并保持 workspace 与 bearer 一致；绑定托管账号时使用指定账号。Codex User-Agent、版本、实验协议头及会话缓存头同时对齐。判断正文使用 ChatGPT 订阅后端支持的 SSE 和存储选项。

后端相关回归 **153 个不同测试通过、0 失败**：原生登录认证 14、意图路由 9、转发器 129（包含新增真实转发器边界测试 1）、外部代理冲突保护 1。边界测试单独执行后又随转发器回归执行，因此日志中该测试出现两次，不重复计数。证据：`artifacts/codex-native-auth-tests-final.log`。

原生登录测试使用项目内临时文件及 Axum 本地模拟 OAuth 服务，覆盖读取、提前刷新、并发合并、刷新令牌轮换、未知字段保留、CLI 更新优先、退出登录不复活、账号切换、错误脱敏、刷新身份不一致拒绝、客户端版本与会话缓存头。真实转发器测试在隔离 HOME 下验证无 bearer 的判断请求和携带旧 bearer 的工作请求实际发出的认证。既有第三方头剥离回归通过；没有读取或刷新用户真实凭据。

前端相关 10 项测试通过，TypeScript 检查、Vite 生产构建和 Windows 桌面构建通过。最新 EXE 与 ZIP 已更新；静默隔离启动存活 8 秒并创建数据库，PE 检查为 x64 Windows GUI，ZIP 内 EXE SHA256 与直接分发的 EXE 一致，携带 WebView2Loader.dll。证据：`artifacts/codex-native-auth-frontend.json`、`artifacts/codex-native-auth-build.log`、`artifacts/codex-native-auth-package.log`、`artifacts/codex-native-auth-smoke.log`、`artifacts/codex-native-auth-package-check.json`。以上证明本地实现，不证明外部账号权限、具体模型可用性或真实模型节省率。

## 2026-10-15 Codex 完整历史路由接入

- 完整 input 历史且有用户消息、没有 previous_response_id 或 compaction 时，删除不可跨工作模型复用的 encrypted reasoning 项；保留消息、工具调用和结果。增量与压缩状态继续要求归属，不擅自迁移。
- 本地 proxy::intent 回归 11 项通过，1 项真实账号测试默认忽略。
- 显式执行 live_native_codex_full_history_route：使用当前原生 Codex 登录、隔离内存数据库；真实判断模型和工作模型 gpt-6-luna，source=judge，输出 ROUTE_OK，收到 response.completed，12.46 秒通过。消耗真实额度。
- 此结果验证完整历史接入及原生认证、判断和执行链路，不代表已验证用户当前组的复杂任务选模质量、跨模型加密状态兼容或节省比例。
- 证据：artifacts/codex-replay-tests.log、codex-replay-live.log。测试时桌面代理未运行、客户端为 direct；未改用户路由组或客户端配置。

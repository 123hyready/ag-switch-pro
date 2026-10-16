<div align="center">

<img src="app/src/assets/icons/ag-switch-pro-icon-source.png" alt="AG Switch Pro" width="112" />

# AG Switch Pro

### 面向 Claude Code 与 Codex 的模型管理和意图路由工具

**一个模型判断任务，多个模型分工执行，让模型能力与任务难度相匹配。**

[![Version](https://img.shields.io/badge/version-0.1.0-blue)](https://github.com/123hyready/ag-switch-pro/releases/tag/0.1.0)
[![Platform](https://img.shields.io/badge/platform-Windows%20x64-lightgrey)](https://github.com/123hyready/ag-switch-pro/releases)
[![Tauri](https://img.shields.io/badge/built%20with-Tauri%202-orange)](https://tauri.app/)
[![License](https://img.shields.io/badge/license-MIT-green)](LICENSE)

[下载安装](#下载安装) · [快速开始](#快速开始) · [功能特性](#功能特性) · [常见问题](#常见问题) · [开发与构建](#开发与构建)

</div>

## 为什么选择 AG Switch Pro？

复杂设计需要更强的推理能力，读取文件、整理信息和明确的机械修改则可以交给轻量模型。AG Switch Pro 基于 [CC Switch](https://github.com/farion1231/cc-switch)，借鉴 [Magpie](https://github.com/yetone/magpie) 的意图路由设计，在原有桌面模型管理基础上加入判断模型与工作模型的分工。

- **按阶段分配模型**：由判断模型分析下一阶段任务，从配置的工作模型中选择执行者。
- **连续请求复用判断**：同一阶段的连续请求可以沿用已有选择，减少重复判断。
- **看清路由过程**：展示判断模型、选择理由、执行尝试和失败回退。
- **沿用 Codex 登录**：支持读取现有 Codex 登录凭据，无需另填 API Key。
- **自动发现模型**：从供应商获取模型列表，并应用返回的上下文及图片能力。

> 当前版本为 **0.1.0 开发预览**。实际节省效果取决于模型价格、任务分布和判断频率；判断调用本身也会消耗额度或产生费用。

## 下载安装

### 系统要求

- Windows x64。
- 已安装 Microsoft WebView2 Runtime。
- 使用意图路由时，需要可用的 Claude Code 或 Codex 客户端，以及对应供应商或登录账号。

### Windows 便携版

前往 [Release 0.1.0](https://github.com/123hyready/ag-switch-pro/releases/tag/0.1.0)，下载：

| 文件 | 用途 |
| --- | --- |
| `ag-switch-pro-0.1.0-windows-x64-preview.zip` | Windows x64 便携程序 |
| `ag-switch-pro-0.1.0-windows-x64-preview.zip.sha256` | 压缩包 SHA-256 校验值 |

解压后，保持 `WebView2Loader.dll` 与 `ag-switch-pro.exe` 在同一目录，双击 EXE 启动。运行便携版无需安装 Node.js 或 Rust。

当前仅提供 Windows x64 构建；macOS、Linux 暂未提供本项目的验证包。

## 快速开始

### 1. 准备供应商

在应用中添加 Claude Code 或 Codex 的供应商。使用 API 接口时，配置对应的地址与凭据。

对于 Codex 的 **OpenAI Official** 供应商：未绑定托管账号时，使用当前 Codex 配置目录中的 `auth.json`；请先在 Codex 中完成登录。已绑定托管账号时使用绑定账号。仅存于系统钥匙串、未落盘的凭据暂不支持。

### 2. 创建意图路由组

进入「意图路由」，选择客户端并配置：

| 配置 | 作用 |
| --- | --- |
| 判断模型 | 分析下一阶段任务，选择工作模型 |
| 工作模型 | 实际处理客户端请求，可配置一个或多个 |
| 擅长任务与成本说明 | 帮助判断模型理解各工作模型的分工 |
| 上下文、工具与图片能力 | 用于筛选能接收当前请求的模型 |
| 默认工作模型 | 暂停自动路由等情况下使用的模型 |
| 异常回退模型 | 判断失败、次数达上限或允许执行回退时使用的模型 |

选择供应商后可从自动获取的列表中选择模型。已有配置可点击「应用模型能力」更新为供应商返回的容量和图片能力；未知能力需要自行确认。

### 3. 启动客户端路由

1. 点击「保存路由组」，再点击「启动客户端路由」。
2. 在客户端的 `/model` 中选择该路由组。**Codex 模型目录变化后需重启客户端。**
3. 正常提交任务，在「最近路由记录」查看过程，在用量统计中查看调用记录。

「试判断」只调用判断模型并显示选择结果，不执行工作任务。需要撤回客户端接管时，点击「恢复直连」。

若 CC Switch 或 Magpie 已接管同一客户端，请先在对应应用恢复直连，再启用本项目的路由。

## 功能特性

### 意图路由与模型分工

| 模式 | 行为 |
| --- | --- |
| 阶段判断 | 一轮任务内可以随执行阶段重新选择模型 |
| 每轮判断 | 在用户新消息到来时进行判断 |
| 观察模式 | 执行默认工作模型，同时记录判断建议，便于评估策略 |
| 手动固定 | 优先使用指定的工作模型 |

模型切换发生在请求边界。已经开始输出的请求不会重新投递给其他模型。

### 判断到执行的过程记录

每条记录展示 **判断模型 → 工作模型选择及原因 → 执行尝试 → 失败回退**，并区分本次判断、阶段保持、重复请求和判断次数达上限。页面每 5 秒刷新记录，可展开查看会话与请求标识。

“上游已接收”表示代理已拿到上游响应，不代表流式输出已结束或编码任务已验证成功。旧记录缺失的判断模型和执行信息会明确标注，不使用当前配置补造历史。

详见[路由过程记录说明](docs/intent-route-trace.md)。

### 用量、上下文与回退

- 判断调用计入用量统计；未知费用不代表免费。
- 根据工具、图片和上下文需求筛选工作模型，避免发送不兼容请求。
- 允许在输出开始前回退到配置的模型，失败模型进入冷却。
- Codex 路由组的上下文容量取工作模型配置的最小值，自动压缩阈值设为该容量的 90%。
- 压缩由客户端发起；完整长会话自动压缩仍需进一步实际验证。

### 桌面与客户端接管

应用基于 Tauri 2、React 和 Rust，保留 CC Switch 的供应商管理等基础能力。新增意图路由面向 Claude Code 与 Codex；托盘提供打开主窗口、客户端入口、轻量模式和退出功能。

应用数据目录为 `~/.ag-switch-pro`，默认代理端口为 `15722`。便携分发仍使用独立用户数据目录，不会自动将数据写入解压目录。当前版本未启用自动更新，请从 Releases 下载新包。

## 常见问题

### “判断次数已达上限”是什么意思？

原提示为“预算耗尽”，指当前会话用完了允许的判断次数，**不代表账户余额或 Token 额度耗尽**。默认最多判断 20 次，达到上限后使用配置的回退工作模型继续执行。可调整「每个会话最多判断次数」；设为 0 表示不调用判断模型。

### 为什么路由组的上下文与某个工作模型不一样？

路由组对客户端公布所有工作模型中最小的配置容量，便于客户端按共同容量管理历史。各工作模型仍有自己的能力限制。旧配置需点击「应用模型能力」更新，不能通过手工填大容量扩展模型的实际上限。

### 为什么有时没有调用判断模型？

阶段保持、重复请求、手动固定、暂停路由、状态连续性限制或判断次数达上限，都可能跳过判断。查看记录的来源及过程说明即可区分。

### 旧会话能否直接跨模型继续？

完整历史可重放的请求会保留消息与工具结果，并去掉不可跨模型复用的加密推理记录。服务端增量、压缩或其他不能移交的状态仍需保持原模型；未知来源的状态会拒绝自动转移。遇到这类情况，请回到原模型或新建会话。

### 使用前需要注意哪些边界？

判断结果不保证每次都是最优模型；上下文采用估算检查。当前提供的是开发预览，应结合自己的任务验证。有关客户端配置、环境影响和验证范围，见[环境影响审计](docs/environment-impact-audit.md)与[验收记录](docs/intent-routing-acceptance-20261015.md)。

## 开发与构建

### 项目结构

```text
app/                         Tauri 2 + React + Rust 应用
app/crates/intent-router/     可独立测试的路由状态机
tools/                       便携工具链准备、构建与验证脚本
docs/                        设计、接口约定与验证说明
third_party/                 第三方许可证
```

编译器、依赖缓存、构建产物及本地参考仓库不随源码提交。

### 构建环境

Windows 脚本使用项目内的 Rust 1.95.0 GNU 工具链与匹配 MSVCRT 的 MinGW，通过绝对路径调用，不修改 PATH。先运行 `tools/Prepare-Toolchain.ps1` 准备工具链；下载来源与校验方式见[工具链说明](docs/toolchain-provenance.md)。

前端需准备 Node.js 与 `app/package.json` 指定的 pnpm，并在 `app/` 安装依赖。以下命令在仓库根目录执行；`-NodePath` 可传入已有 `node.exe` 的绝对路径，省略时查找已有 Node.js。

```powershell
# 测试独立路由核心
./tools/Invoke-Cargo.ps1 -CargoArguments @('test', '--manifest-path', 'app/crates/intent-router/Cargo.toml')

# 后端与前端测试
./tools/Test-Backend.ps1
./tools/Test-Frontend.ps1 -NodePath 'E:/nodejs/node.exe'

# 构建、启动检查与便携打包
./tools/Build-App.ps1 -NodePath 'E:/nodejs/node.exe'
./tools/Smoke-App.ps1
./tools/Package-App.ps1

# 开发运行
./tools/Start-Development.ps1 -NodePath 'E:/nodejs/node.exe'
```

请将示例中的 Node.js 路径替换为本机路径。`Smoke-App.ps1` 检查独立进程及数据库初始化，不验证界面渲染或真实模型调用。

## 文档与贡献

- [设计方案](docs/ag-switch-pro-plan.md) · [接口与认证约定](docs/intent-routing-contract.md)
- [上下文与判断次数](docs/context-and-judge-limits.md) · [路由过程记录](docs/intent-route-trace.md)
- [验证记录](docs/validation.md) · [真实调用验收](docs/intent-routing-acceptance-20261015.md)
- [问题反馈](https://github.com/123hyready/ag-switch-pro/issues) · [提交 Pull Request](https://github.com/123hyready/ag-switch-pro/pulls)

反馈问题时，请提供版本、客户端、复现步骤与脱敏日志；请勿提交 API Key、`auth.json` 或完整登录令牌。

## 致谢与许可

基于 [CC Switch](https://github.com/farion1231/cc-switch) 提交 `96d636ca`，意图路由借鉴 [Magpie](https://github.com/yetone/magpie) 提交 `e21ea07e` 的设计。感谢原项目作者与贡献者。

本项目遵循 [MIT License](LICENSE)，保留原作者版权声明；Magpie 许可证见 [third_party/LICENSE-magpie.txt](third_party/LICENSE-magpie.txt)。

# Open Computer Use 插件安装记录

2026-10-14 已安装并启用 `open-computer-use@open-computer-use-local`，版本 `1.0.0`。

来源：[iFurySt/open-codex-computer-use](https://github.com/iFurySt/open-codex-computer-use)，使用该仓库发布的 npm 压缩包 `open-computer-use-1.0.0.tgz`。归档通过 npm 元数据的 SHA512 integrity 校验，没有运行安装包或修改 PATH。

运行文件和压缩包保存在 `D:/ag-switch-pro/tools/open-computer-use/`。Codex 已注册本地 marketplace，并将插件清单、脚本与配套 Skill 安装到用户插件缓存。

- 插件缓存：`C:/Users/HY/.codex/plugins/cache/open-computer-use-local/open-computer-use/1.0.0/`
- Windows x64 运行程序：`tools/open-computer-use/package/dist/windows/amd64/open-computer-use.exe`
- 已有 Node：`E:/nodejs/node.exe`
- 验证结果：`tools/open-computer-use/verification.json`

为适配 Windows 和绝对路径调用，安装源的 MCP 配置使用已有 Node 直接启动随包 REPL，并传入项目内 Windows 程序路径，不依赖 Bash 或 PATH。插件清单同时加入了随包 Skill。上游原始清单和 MCP 文件保留为 `.upstream` 副本。

Codex CLI 已确认 `installed: true`、`enabled: true`。MCP 握手、工具目录 `js` / `js_reset` 与 REPL 计算检查均通过；本次没有执行桌面点击、输入或截图操作，也没有修改 AG Switch Pro 路由配置。

重启 Codex 以加载插件，然后可以继续配置路由。此插件的启动路径指向项目目录，因此保留 `tools/open-computer-use/` 运行文件。

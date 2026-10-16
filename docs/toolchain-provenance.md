# 本地工具来源

所有新增编译器、归档、工具和依赖缓存都在 `D:/ag-switch-pro/tools/`；脚本通过绝对路径调用，没有修改 PATH。

| 工具 | 来源与校验 |
|---|---|
| Rust 1.95.0 Windows GNU | 官方 `static.rust-lang.org/dist/channel-rust-1.95.0.toml` 中的 x86_64 GNU 归档；镜像下载后与官方 SHA256 比对 |
| GCC 13.3.0 MSVCRT POSIX SEH | WinLibs 官方 GitHub 发布；7z 归档与发布方提供的 `.sha256` 比对 |
| PlantUML 1.2026.8 | [官方发布](https://github.com/plantuml/plantuml/releases/tag/v1.2026.8)，下载 JAR 到 `tools/plantuml/`，使用本机既有 Java 离线渲染 |
| Node.js / Java | 使用本机已有可执行文件，不另装环境；也可通过脚本参数传入绝对路径 |
| WebView2Loader.dll | Cargo 下载的 WebView2 SDK 构建产物，选择 x64 DLL 随便携程序携带 |

本次归档 SHA256：

```text
Rust:     c510ce34221cc4e8eff701001a991c78b53e2ea00e5cb0d77546b12af5fb3a03
WinLibs:  08c979b75d193c4fbb8d7eb13ccb0e20fcda52e36738c7deffddc732ec732319
PlantUML: 5e1ecfa8ecd32c90b03bbf3b1eb6f020943f98ab0fcf4032be31a0002ee2c462
```

PlantUML 哈希是本次从官方地址下载后的本地留档值。Rust 与 WinLibs 则另外比对了发布方校验值。

Windows GNU 单元测试需要 Common Controls v6 清单和 WebView2Loader.dll。`Test-Backend.ps1` 只对项目生成的测试二进制嵌入已有清单并复制 DLL；主桌面程序的资源由 Tauri 构建流程生成。

便携输出采用包含前端的 debug 构建，发行副本移除了调试符号。这是可运行的开发预览，不包含安装器、签名或自动更新发布渠道。WebView2Loader.dll 不是浏览器运行时；机器需要已有 WebView2 Runtime。

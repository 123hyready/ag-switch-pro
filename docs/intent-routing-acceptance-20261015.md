# 意图路由验收（2026-10-15）

使用已保存的“智能编码”配置副本、当前 Codex 原生登录和真实上游模型测试，执行数据库隔离在内存。没有改 Node/npm/Codex 安装，也没有修改客户端 config.toml。测试经过产品 Responses 处理器、判断模型调用、路由选择、原生认证及流式响应处理；不是只测试判断接口，也不是 GUI 自动化验收。

## 发现与修复

首次真实测试失败：gpt-6-astra 判断超过原 8000 ms，简单任务回退至 gpt-6.1-sol。官方 Codex 判断请求改用 low 推理强度；测试超时改为 30000 ms。通过后仅将用户“智能编码”组的 judgeTimeoutMs 更新为 30000，并递增 revision；旧配置备份在 artifacts/acceptance-route-before-timeout.json。

修复业务 schema/工具输出中的 signature 等字段被误识别为模型加密状态。实际 reasoning、thinking、compaction 状态仍被识别。增加 session-id/thread-id 会话标识兼容。

完整历史输入会移除不宜跨模型复用的加密 reasoning 项，保留消息和工具调用/结果；previous_response_id 或 compaction 等增量/压缩状态不剥离，未知归属仍拒绝，需在原模型继续或新建路由会话。

## 真实账号结果

| 用例 | 决策来源 | 工作模型 | 判断耗时 | 完成 |
|---|---|---|---|---|
| 带旧加密推理的简单排序 | judge | gpt-6-luna | 6740 ms | 是，排序正确 |
| 完全重复请求 | duplicate | gpt-6-luna | 0 ms | 是，未重复判断 |
| 要求读取固定文本的工具调用 | judge | gpt-6-luna | 6066 ms | 是，返回 function_call |
| 工具结果续接 | judge | gpt-6-luna | 7097 ms | 是，返回固定文本 |
| 并发一致性复杂设计 | judge | gpt-6.1-sol | 9527 ms | 是，流完整结束 |

所有请求收到 response.completed。工具函数是测试固定文本桩，不执行用户文件操作。复杂输出仅验证流完成和选模，没有形式化审核设计正确性。此组判断模型是 gpt-6-astra，判断本身有费用，结果不能证明总费用一定降低。

## 回归

路由核心 30 项；桌面相关后端 42 项；前端 13 项通过。后端 42 = intent 11 + session 11 + native_auth 14 + oauth_models 6。两个真实账号测试默认 ignored，本次显式执行 saved_coding_group_live_acceptance（5 请求）通过，71.55 秒。核心覆盖阶段复用、并发隔离、失败回退和冷却；失败注入使用本地桩，没有故意破坏真实供应商。

日志：artifacts/acceptance-live.log、acceptance-live-results.json、acceptance-live-before-optimization.json、acceptance-core.log、acceptance-backend.log、acceptance-frontend.log。构建及启动证据见 acceptance-build.log、acceptance-package.log、acceptance-smoke.log。

最终产物：EXE 构建成功，ZIP 内 EXE 的 SHA256 与输出文件一致，隔离启动 8 秒成功并初始化数据库。artifacts/ag-switch-pro-models.exe 已同步更新。

环境对比：本轮前后 node.exe、npm-cli.js、codex.js、Codex config.toml 的 SHA256 均相同；用户和系统 PATH 均相同（artifacts/acceptance-environment-check.json）。此结果只说明本轮操作，不消除独立环境影响报告中的历史风险。

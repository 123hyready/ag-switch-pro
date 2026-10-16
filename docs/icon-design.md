# AG Switch Pro 图标：万象生长

用户要求：简洁大气，体现包罗万象、勃勃生机。

设计方向：以包容的圆环和向上展开的三片叶形构成统一符号，使用深青、玉绿和少量薄荷绿；保留负空间，不添加文字与细碎装饰。采用透明背景，兼顾 Windows 桌面、任务栏和应用内的小尺寸显示。

使用内置 imagegen 生成，原始 PNG 已保存在 `app/src/assets/icons/ag-switch-pro-icon-source.png`。应用内图标由同一源图缩放，桌面多尺寸 PNG、ICO、ICNS 由 Tauri CLI 导出。正式采用圆环环抱三片舒展叶形的生成图；图像服务等待期间绘制的扁平矢量备选保存在 `app/src/assets/icons/ag-switch-pro-icon-alternative.svg`，未用于正式程序。

已同步 Windows EXE、窗口、托盘、侧栏、“关于”页和网页页签图标。16、32、64、256 像素导出预览位于 `artifacts/icon-preview/`；32 像素下仍能辨认圆环与叶形。生成请求耗时较长，但最终成功返回。

## 验证与交付

TypeScript 检查、前端生产构建与桌面构建通过。Windows 资源编译增加 `icons/icon.ico` 文件依赖，防止换图标后继续使用旧缓存。EXE 内 16、24、32、48、64、256 像素图标资源均与新 ICO 相同；便携 ZIP 内 EXE 与直接分发版本的 SHA256 一致。隔离静默启动存活 8 秒并初始化测试数据库。

证据：`artifacts/icon-export.log`、`artifacts/icon-build.log`、`artifacts/icon-embedded-check.json`、`artifacts/icon-package-check.json`、`artifacts/icon-smoke.log`。最新程序为 `artifacts/ag-switch-pro.exe`，ZIP 为 `artifacts/ag-switch-pro-0.1.0-windows-x64-preview.zip`。

## 生成提示词

```text
Use case: logo-brand. Create one finished desktop application icon for AG Switch Pro. User intent: 简洁大气，体现包罗万象、勃勃生机. A premium minimalist, confident, original emblem: a thick near-circular embracing orbit containing three broad upward-growing leaf/flame forms unified as one harmonious symbol, suggesting an inclusive universe and flourishing life. Use deep midnight teal, jade green, and a restrained luminous mint accent. Flat graphic design, exceptionally clean silhouette and precise smooth curves; at most 3 or 4 broad shapes, generous negative space, bold enough to remain unmistakable at 16 and 32 pixels. Center the compact emblem with 12% safe padding on all sides in a square image, actual transparent background. No words, letters, numbers, framing mockup, gradients, texture, shadows, tiny details, or photorealism. The icon is the asset itself, not a presentation sheet. Balanced circular composition, serene but energetic, timeless professional software identity.
```

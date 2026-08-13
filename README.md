# NovaHub

> 星汇：一个本地优先、低资源占用、可安全扩展的桌面生产力入口。

NovaHub 面向 Windows 与 macOS，使用 Rust 构建宿主、系统能力和插件运行时。它通过全局快捷键提供应用启动、计算器、剪贴板历史、文件搜索、系统命令和 Rust/WASM 插件，并由宿主统一渲染官方声明式 UI。

## 当前阶段

项目目前处于设计与实施准备阶段。总体架构和 V1 产品范围已经固定，V1 采用“能力对标 + 迁移辅助”：Windows 优先完整覆盖 14 项热门能力，macOS 首发覆盖核心能力，不直接运行 uTools/Raycast 插件。实现代码将按已落盘的独立实施计划推进。

## 核心选择

- Windows + macOS 双平台
- Rust + Slint 桌面宿主
- Wasmtime Component Model + WIT 插件协议
- Rust/WASM 首版插件 SDK
- SQLite 本地数据与 `nucleo` 模糊匹配
- 主程序与按需启动的插件宿主双进程
- 官方声明式组件，不允许插件注入任意 HTML
- 第三方插件仅在用户调用时运行
- 插件卸载默认删除其本地数据与权限

## 文档

完整方案从 [docs/INDEX.md](docs/INDEX.md) 开始阅读。

V1 热门能力、现代化体验与迁移边界见 [docs/11-v1-popular-tools-and-modern-experience.md](docs/11-v1-popular-tools-and-modern-experience.md)，工程执行顺序见 [V1 设计原型与实施计划](docs/aegis/plans/2026-08-13-v1-design-and-implementation.md)。

品牌图标与平台导出规范见 [assets/brand/README.md](assets/brand/README.md)。

可运行交互原型位于 [prototype/index.html](prototype/index.html)，官方组件验收页位于 [prototype/components.html](prototype/components.html)。

## 方案状态

这些文档是待评审的设计基线，不代表功能已经实现或达到发布状态。

# NovaHub

> 星汇：一个本地优先、低资源占用、可安全扩展的桌面生产力入口。

NovaHub 面向 Windows 与 macOS，使用 Rust 构建宿主、系统能力和插件运行时。它通过全局快捷键提供应用启动、计算器、剪贴板历史、文件搜索、系统命令和 Rust/WASM 插件，并由宿主统一渲染官方声明式 UI。

## 当前阶段

项目已进入可运行 MVP 实施阶段。当前主线是 Rust + Slint Shell、按需 sibling Plugin Host、WIT/WASM 插件、SQLite 本地数据和 Windows/macOS capability adapter；V1 的 14 项能力仍按“能力对标 + 迁移辅助”逐步扩展，不直接运行 uTools/Raycast 插件。

本地验证入口：

```text
cargo test --workspace --offline -- --test-threads=1
cargo run -p xtask --offline -- release-check
cargo run -p xtask --offline -- build-examples
NOVAHUB_HEADLESS=1 cargo run -p novahub-app --offline
cargo run -p xtask --offline -- resource-report --pid <novahub-root-pid>
```

MVP 已覆盖宿主搜索、计算器、剪贴板策略、文件搜索及打开/定位/复制路径动作、系统命令确认、插件安装/更新/回滚/卸载、结构化权限与安装 diff、`view`/`one-shot` 命令路由、迁移 dry-run/事务导入、声明式 View、可切换的内置/自定义桌面宠物和真实 Wasmtime Component 往返。跨平台原生无障碍、签名安装器、权限 broker 实机矩阵和参考机资源指标仍由发布 CI/实机验收闭合。

Shell 中可直接使用 `files <root> <query>` 搜索，使用 `files open <path>`、`files reveal <path>` 或 `files copy <path>` 处理结果；`copy <text>` 用于复制受限文本。桌面宠物可用 `pet list` 查看，使用 `pet use <id>` 激活内置或已安装的声明式宠物。

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
- 插件 SDK/CLI 提供 `view` 与 `one-shot` 两类模板；生产环境不保留 watcher 或 WebView

## 文档

完整方案从 [docs/INDEX.md](docs/INDEX.md) 开始阅读。

V1 热门能力、现代化体验与迁移边界见 [docs/11-v1-popular-tools-and-modern-experience.md](docs/11-v1-popular-tools-and-modern-experience.md)，工程执行顺序见 [V1 设计原型与实施计划](docs/aegis/plans/2026-08-13-v1-design-and-implementation.md)。

品牌图标与平台导出规范见 [assets/brand/README.md](assets/brand/README.md)。

可运行交互原型位于 [prototype/index.html](prototype/index.html)，官方组件验收页位于 [prototype/components.html](prototype/components.html)。

## 方案状态

文档仍是架构与验收基线；代码已形成可运行 MVP，但不等同于跨平台 Release Candidate。发布前必须补齐平台实机、无障碍、签名安装器和完整进程树资源证据。

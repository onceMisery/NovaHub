# MVP 实施反思

代码级 MVP 已形成可运行闭环：Rust + Slint 主 Shell 保持轻量，WIT 是唯一插件 ABI，Wasmtime 只在按需 sibling Host 中运行；插件安装、签名校验、版本指针、更新/回滚、启停和 pending-delete 均在宿主边界内完成。应用 active pointer、官方 View 摘要、有界应用搜索、平台化快捷键标签、DPI 感知宠物定位、文件结果动作、剪贴板复制和自定义宠物持久化也已接入宿主，未引入 Tauri/WebView 或额外常驻运行时。

当前不能把实现验证等同于发布验收。Windows/macOS 实机上的 LaunchServices/Windows Search 质量、全局快捷键冲突、原生宠物窗口定位/DPI、Narrator/VoiceOver、签名安装器以及 RSS/Private Working Set/GPU/冷启动指标仍需外部参考机证据；这些是发布门槛和平台适配工作，不构成降级为 Tauri 的理由。

本轮验证覆盖 `cargo fmt`、workspace Clippy、workspace 单线程测试、`release-check`、headless Component 探针、五个示例包构建、带 `--source` 的迁移 dry-run、递归进程树资源报告、差异空白和 UTF-8 BOM 扫描；插件命令索引通过 enabled active pointer 进入宿主搜索，插件执行仍只发生在 sibling Host，插件 ID 也统一经过路径安全校验。仓库 CI 已声明 Windows x86_64、macOS x86_64 和 macOS arm64 矩阵，签名与 Notarization 仍隔离在受保护发布环境。Aegis workspace bundle/check 仍受仓库既有治理目录、索引和 sidecar 缺失阻塞；这不影响上述代码级验证结果。

本轮新增的固定搜索结果行、`Cmd/Ctrl+K` ActionPanel、`clipboard copy <序号>` 文本恢复和 `sync_channel(1)` 响应通道继续遵守同一边界：Slint 只渲染宿主提供的有界数据，插件只通过 WIT View/Action 交互，UI 线程不等待 IPC。对应的 workspace 与 Release 验证已重新执行并通过；图片剪贴板恢复明确保持未支持，避免伪造平台能力。

Kunkun 借鉴项已进一步落到可执行契约：manifest 的 `view`/`one-shot` 分类贯通搜索、宠物动作、Host 生命周期和 CLI scaffold；one-shot 不进入持久 View session，结果和 UI 状态都由宿主有界处理。该实现继续以 Rust + Slint 主 Shell、WIT 唯一 ABI 和按需 sibling Host 为前提，未引入 Tauri/WebView 或常驻 JavaScript。MVP 必须项已经明确为权限与 diff、交互分类、官方 View、stable-ID/虚拟列表、诊断、双模板与 golden/真实 Host 验证；watcher、多语言 SDK 和扩展 provenance 保持 Phase 2。当前 P0 capability 主调用链、持久化 user grant/撤销、卸载清理、typed KV、session 失效、opaque token 拒绝和 broker 级逐跳 redirect 已贯通并有回归证据；剩余风险是有界 HTTP、有效文件 token、完整 Slint/View 语义与平台实测，不应通过架构降级解决。

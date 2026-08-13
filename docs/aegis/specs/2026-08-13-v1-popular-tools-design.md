# NovaHub V1 热门能力设计规格

## 状态

已由用户确认：兼容策略 B（能力对标 + 迁移辅助）、首期平衡范围 B（14 项）、Windows 完整优先且 macOS 保持核心能力与架构兼容。本文是实施计划的权威输入，不表示功能已经实现。

## Goal

在既有 Rust + Slint + Wasmtime/WIT 架构上，将 NovaHub 从 6 项 MVP 基线扩展为可日常替代 uTools/Raycast 高频工作流的 V1，并通过迁移中心降低用户和插件作者切换成本。

## Approved Scope

- 8 个内置能力族：应用/命令、文件、剪贴板、Snippets、Quicklinks、窗口、系统命令、计算/换算。
- 5 个官方工具插件族：翻译、JSON、UUID/Hash/编码、颜色、二维码。
- 插件管理与迁移中心。
- Windows 14 项主流程；macOS 核心能力与相同协议/体验结构。
- 扩展本地确定性原型与 Figma 同步清单。

## Authority Refs

- `docs/03-system-architecture.md`：进程、模块、平台和 IPC 所有权。
- `docs/04-plugin-platform.md`：WIT、权限、安装/卸载与 SDK。
- `docs/05-ui-ux-design.md`：官方组件、主题、焦点与无障碍。
- `docs/07-security-privacy.md`：数据分类、凭据与权限。
- `docs/08-quality-release.md`：性能、测试与发布门槛。
- `docs/11-v1-popular-tools-and-modern-experience.md`：V1 产品、原型与验收范围。

## Architecture

`novahub-app` 继续拥有 Shell、搜索协调、内置 Provider、官方 View 渲染、权限裁决与本地数据。`novahub-plugin-host` 继续按需运行官方和第三方 WASM Component。V1 不增加兼容运行时或第三进程类型。

搜索注册表统一容纳 `BuiltInCommandDescriptor` 与 `PluginCommandDescriptor`，但只有用户执行插件命令时才创建插件会话。迁移中心是宿主内置、离线、事务化的导入管线，其解析器输出 NovaHub 中立的 `MigrationItem`，不能直接写业务表或执行源内容。

## Ownership Map

| 契约/数据 | 唯一所有者 | 下游消费者 |
|---|---|---|
| Query/Result/Action | `core-domain` | search、providers、UI、plugin adapter |
| 官方 View/Event | `ui-protocol` | ui-slint、WIT bindings、SDK |
| Provider 生命周期 | `search` | app coordinator、platform adapters |
| 剪贴板/Quicklinks/Snippets schema | `storage` | built-in providers、migration |
| OS 能力 | `platform/*` | providers、capability broker |
| 插件 ABI | `wit/novahub-plugin` | runtime、SDK、official plugins |
| 导入中立模型与事务 | `migration` | uTools/Raycast parser、storage |
| 视觉令牌和组件 | `ui-slint` | Shell 与官方 View renderer |

## Compatibility Boundary

- 不执行、翻译或打包 uTools/Raycast 插件代码。
- 不读取未公开数据库，不迁移账号令牌、凭据、剪贴板历史或云端数据。
- 导入源必须是用户显式选择的导出文件；所有解析本地完成。
- WIT 是插件契约唯一来源，迁移适配器不能向 WIT 增加竞品特有类型。
- Windows/macOS 共享领域测试和 UI 状态；平台专用能力通过 capability 探测表达。
- 原有 10 个确定性原型 URL 保持可用；新增状态只扩展清单。

## UI State Model

所有工作流复用 Shell 状态：`idle`、`loading`、`ready`、`partial`、`empty`、`error`、`confirming`。工具 View 使用宿主 Navigation 管理层级，插件只能返回允许的 List/Grid/Detail/Form/ActionPanel 模型。Window Manager 的布局格、颜色取色和文件预览可由宿主专用组件实现，但必须通过稳定 UI 协议暴露状态，不允许插件绘制任意画布。

## Data And Privacy

- 剪贴板正文加密存储，默认 7 天/500 项；敏感来源排除规则先于持久化。
- Quicklinks 与 Snippets 可导出，但字段变量经过白名单解析，不执行脚本。
- 汇率缓存含来源与时间戳；离线时明确展示陈旧状态。
- 翻译网络域名按插件权限授权，历史默认本地且可关闭。
- 迁移先解析到临时数据库/内存模型，预览确认后单事务写入；失败回滚。

## Performance Budgets

- 宿主无 Plugin Host 闲置 RSS P95 ≤ 70 MiB。
- 热唤起到首帧 P95 ≤ 100 ms；输入到首批本地结果 P95 ≤ 50 ms。
- 每个 Provider 有独立 deadline、结果上限与取消；慢源只产生 Partial 状态。
- 剪贴板缩略图、文件预览和二维码图片都有字节预算并延迟解码。
- Plugin Host 冷启动 P95 ≤ 500 ms，单插件默认线性内存 64 MiB。

## Platform Rollout

Windows 是功能完整验收平台。macOS 从第一阶段即实现快捷键、窗口定位、应用发现、Spotlight、Keychain、剪贴板与核心窗口操作，避免后置移植。平台高级差异以能力标记进入发布清单，不以空操作或通用失败掩盖。

## Verification

- 领域与存储：Rust 单元/属性/迁移测试。
- Provider：固定语料的排序、取消、超时和平台契约测试。
- 插件：WIT conformance、资源限制、权限拒绝、Host 崩溃恢复。
- 迁移：公开样例与恶意/损坏输入语料、事务回滚、禁止执行测试。
- UI：Slint 组件快照、键盘路径、主题、高对比度、200% 文本、减少动画。
- E2E：Windows 全矩阵；macOS 核心矩阵；性能在固定参考机采样 P50/P95。

## Risks And Falsification

| 假设 | 反证信号 | 处置 |
|---|---|---|
| Slint 可承载紧凑桌面 UI 与无障碍 | 核心流程无法通过 Narrator/VoiceOver | 先补原生语义适配；失败后评审局部原生控件 |
| 系统文件索引足够 | 固定语料召回率或 P95 不达标 | 评审受控增量索引，不默认全盘扫描 |
| 共享 Host 隔离足够 | 可复现跨插件干扰 | 高风险插件迁移独立进程并量化 RSS |
| 迁移辅助能覆盖切换成本 | 20 个真实样本多数只能标记重写且无指导价值 | 增加静态迁移分析，不运行源插件 |

## Acceptance

1. `docs/11` 的 14 项能力均映射到唯一所有者、原型流程、实现任务和验收测试。
2. Windows 完整、macOS 核心范围均无“占位成功”或平台分支泄漏到 WIT。
3. 迁移中心能安全分类和事务导入允许的数据，且明确拒绝代码、凭据和隐私数据。
4. 插件依旧可随包安装/卸载前后端能力，宿主官方组件和权限裁决不被绕过。
5. 性能、安全、无障碍和默认删除语义达到既有基线。

## Non-goals

直接兼容竞品插件、JavaScript/Node/WebView 运行时、常驻第三方后台、Linux、移动端、账号同步、AI 代理、任意脚本型 Snippets 或工作流。

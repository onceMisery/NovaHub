# NovaHub 总体设计规格

## 状态

待用户评审。本文是实现计划的输入，不表示代码已经实现。

## TaskIntentDraft

### 目标

构建 Windows + macOS 的 NovaHub 桌面生产力入口：Rust 宿主、Slint 官方声明式 UI、按需 Rust/WASM 插件、稳定 WIT 协议和低内存运行模型。

### 范围

MVP 包括应用启动、计算器、剪贴板历史、基础文件搜索、系统命令、插件安装/禁用/更新/卸载、Rust SDK、CLI 和示例插件；方案同时固定长期生态、工作流、云服务边界与最终形态。

### 主要风险

跨 Windows/macOS 的窗口/快捷键/无障碍差异；Slint 的桌面能力成熟度；WIT/Wasmtime 版本演进；插件权限和卸载删除边界；低内存目标与冷启动延迟之间的冲突。

## BaselineReadSetHint

- 仓库基线：当前只存在 README 和初始提交，无既有实现约束。
- 已确认用户决策：双平台、Rust/WASM、官方声明式 UI、按需插件、默认删除、MVP 选项 B、共享 Plugin Host。
- 权威设计文档：`docs/01` 至 `docs/09`、`CONTEXT.md`、本文件和 `BASELINE-GOVERNANCE.md`。

## ImpactStatementDraft

- 受影响层：产品范围、宿主架构、插件 ABI、UI 协议、平台适配、数据安全、发布流程和 Figma 设计资产。
- 唯一所有者：WIT 拥有插件契约；宿主拥有 UI 渲染与权限裁决；Plugin Host 拥有第三方执行；storage 拥有 schema；平台 crate 拥有 OS 差异。
- 兼容边界：MVP 仅 Rust/WASM；无任意 HTML、WebView、原生动态库、常驻插件、账号和云同步；未来扩展必须适配现有 WIT 语义。
- 不变量：插件崩溃不影响宿主；UI 线程不执行阻塞任务；卸载默认删除插件本地数据；外部文件/远端数据不属于自动删除范围。

## 方案摘要

### 宿主

`novahub-app` 使用 Rust + Slint，负责窗口、渲染、搜索协调、内置提供器、SQLite 和平台能力；第三方代码只通过本地认证 IPC 进入 `novahub-plugin-host`。

### 插件

插件是 `.novahub-plugin` 归档，包含 `novahub.toml`、签名的 `plugin.wasm` 和资源。WIT Component 导出 `initialize/open/update/close`，返回固定的官方 View；所有 storage/http/clipboard/files 等能力默认拒绝且受权限清单约束。

### 生命周期

按需启动、前台会话、取消与超时、空闲 60 秒回收。禁用保留数据；卸载删除代码、缓存、插件 namespace、凭据、权限和索引，删除失败进入 `pending_delete`。

### UI

启动器默认 720 × 560 px 上限，主搜索第一视觉焦点；语义令牌、4/8 间距、8 px 最大圆角、键盘优先、完整无障碍和深浅色模式为宿主规范。

## 备选方案与决策

| 方案 | 优点 | 代价 | 决策 |
|---|---|---|---|
| 单进程 UI + Wasmtime | 进程少、理论内存最低 | 插件崩溃影响 UI，权限边界弱 | 拒绝 |
| **主程序 + 共享按需 Plugin Host** | 低闲置内存与足够隔离的平衡 | 需要 IPC 与 Host 生命周期 | **采用** |
| 每插件独立进程 | 隔离最强 | 内存、启动和运维成本高 | 仅作为风险触发后的演进 |

| UI 方案 | 结论 |
|---|---|
| Slint + 官方声明式 View | MVP 采用，最符合内存、稳定和统一交互目标 |
| Tauri/WebView | 仅当任意 HTML 是硬需求时重新评审 |
| 原生双前端 | 仅在无障碍或平台体验证据否定 Slint 时评审 |

## 验收标准

- Windows/macOS 能完成快捷键、搜索、内置命令和插件调用主流程
- 无插件活动时主进程 RSS P95 ≤ 70 MiB；插件 Host 按需启动并空闲回收
- 插件超时、超限、Host 崩溃不拖垮主窗口
- 安装、更新、禁用、卸载语义符合 `docs/04-plugin-platform.md`
- 官方 UI 组件与 `docs/05-ui-ux-design.md` 的令牌、键盘和无障碍要求一致
- 插件卸载默认删除插件本地数据并验证 `pending_delete` 重试路径
- 文档、协议和原型统一使用 NovaHub 品牌与术语

## 非目标

Linux、移动端、TypeScript MVP、任意 HTML/WebView、原生动态库、常驻插件、插件市场、账号、同步、AI 代理和远程执行。

## 自检结论

- 未留下未决占位或模糊的关键决策。
- UI、插件协议、进程模型没有互相复制所有权。
- 路线图对 TypeScript、WebView、独立进程和云服务设置了明确触发条件。
- 设计规格仍需用户评审后，才能进入实施计划阶段。

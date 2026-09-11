# Kunkun 可借鉴设计与 NovaHub 落地建议

## 1. 评审范围

本文分析只读参考项目 `C:\user-data\code\github\kunkun`，评审版本为
`cb8af4930dfbd1ca9f252e73475faa6416e20428`。结论基于 2026-08-15 的静态源码审阅，
没有把 Kunkun 的宣传材料、安装包大小或单进程截图当作内存证据，也没有完成两项目在同一参考机上的运行时基准测试。

本文是外部项目借鉴记录，不是新的架构权威。NovaHub 的约束仍由以下文档定义：

- `docs/03-system-architecture.md`
- `docs/04-plugin-platform.md`
- `docs/07-security-privacy.md`
- `docs/08-quality-release.md`
- `docs/aegis/baseline/2026-08-14-architecture-hardening-baseline.md`

评审问题不是“是否改用 Kunkun 的技术栈”，而是：哪些已经被真实产品验证过的契约、工具和交互思想，
可以在不引入 WebView、JavaScript 运行时或常驻辅助进程的前提下，提高 NovaHub 的完整度。

## 2. 结论摘要

NovaHub 不需要替换核心架构，需要做局部增强。

必须保留的主干是：Rust + Slint 主 Shell、WIT 唯一插件 ABI、宿主拥有声明式渲染与能力裁决、
Wasmtime 仅存在于按需启动的 sibling Plugin Host、统一 Query Coordinator/Ranker，以及按版本目录和
原子 active pointer 管理插件。

Kunkun 最值得借鉴的不是 Tauri/Svelte/Web Worker 这些技术载体，而是四类平台思想：

1. 把命令交互形态、权限范围和兼容版本写成可验证契约。
2. 让宿主拥有常用 UI 语义，同时提供稳定 ID、局部更新和大列表窗口化。
3. 把脚手架、校验、开发重载、故障诊断做成插件平台的一部分。
4. 在安装和商店界面解释权限、来源、构建记录与版本兼容性。

因此，建议按 P0/P1/P2 增强 NovaHub，而不是增加 Tauri/WebView fallback。P0 解决权限契约尚未闭环的问题；
P1 补齐命令形态、官方 View 和诊断能力；P2 改善开发模式与供应链可解释性。

### 2.1 采纳状态（2026-08-15）

本分析中的设计建议已经写入 NovaHub 权威文档：`docs/02` 明确 MVP 范围，`docs/03` 固定进程与所有权，
`docs/04` 固定 manifest/WIT/权限/SDK 契约，`docs/05` 固定安装、信任与诊断交互，`docs/07` 固定安全边界，
`docs/08` 固定验证矩阵，`docs/09` 固定路线图。对应设计决策记录为
`docs/aegis/specs/2026-08-15-kunkun-design-adoption.md`。

采纳采用“契约关键能力进入 MVP，生态增强延后”的边界：

- MVP：强类型权限、权限 diff、`view`/`one-shot`、官方 View 最小完整语义、stable-ID 差量、虚拟列表、
  宿主原生诊断、两类 SDK 模板、golden fixtures、真实 Host E2E、精确版本矩阵和本地信任事实。
- Phase 2：显式 `plugin dev --watch`、多语言 SDK/模板，以及 source/commit/builder/workflow/SBOM/
  transparency log 的扩展 provenance 与可视化。
- 持续拒绝：Tauri/WebView fallback、任意 Web UI、JS 生产运行时、宽系统能力、后台插件和覆盖式升级。

## 3. 第一性原理判断

| 问题 | 判断 |
|---|---|
| NovaHub 的最终目标是什么 | 在 Windows/macOS 上提供低空闲内存、低交互延迟、可隔离插件和一致原生交互 |
| 达成目标不可缺少什么 | 原生轻量 Shell、单一协议所有者、按需运行时、能力最小化、完整进程树度量 |
| Kunkun 哪些价值不依赖 Web 技术 | 显式命令类型、权限裁剪、声明式 UI、SDK/CLI、诊断、权限与来源说明 |
| Kunkun 哪些价值与其载体绑定 | 任意 iframe UI、每扩展 WebViewWindow、Web Worker 执行、Deno/Node/shell 能力 |
| 最小充分调整是什么 | 保留 NovaHub 运行时边界，只扩展 WIT/manifest/宿主模型、测试和开发工具 |

这里的关键区分是“产品契约”和“实现载体”。NovaHub 可以采用 Kunkun 的契约完整度，
但不能为了复用其前端生态而同时引入其常驻内存、隔离和供应链成本。

## 4. 架构对照

### 4.1 Kunkun 当前结构

Kunkun 是 pnpm/Turbo 与 Cargo 组成的 monorepo，桌面端使用 Tauri 2、SvelteKit/Svelte 5、
TypeScript 和 Rust。主界面运行在系统 WebView 中；模板 UI 与无界面命令使用 Web Worker，
自定义 UI 使用 iframe 或额外 WebviewWindow，扩展通过 kkrpc 调用宿主构造的 API。

```text
Tauri 主程序
├── SvelteKit/Svelte 主 WebView
│   ├── 多类 Fuse 搜索源
│   ├── 宿主模板 UI
│   └── Web Worker 扩展
├── 自定义扩展 iframe / WebviewWindow
├── kkrpc
└── Rust/Tauri plugins
    ├── 文件、剪贴板、网络和系统能力
    └── Deno、shell 与子进程能力
```

代码依据包括根 `package.json`、`Cargo.toml`、`apps/desktop/package.json`，以及
`apps/desktop/src/lib/cmds/ext.ts:81-142` 的 Headless Worker 启动路径。

### 4.2 NovaHub 当前结构

```text
novahub-app（常驻）
├── Slint Shell / Official View Renderer
├── Query Coordinator + 单一 Ranker
├── Built-in Providers
├── Plugin Manager / Capability Broker
└── SQLite / Credential Store
        │ 本地认证 IPC，用户动作期间存在
        ▼
novahub-plugin-host
├── Wasmtime Component Model
├── 每插件 Store / Limiter / Session
└── WIT Host API Adapter
        │
        ▼
WebAssembly Component
```

NovaHub 的主进程不链接 Wasmtime，不加载第三方原生库，也不运行 JavaScript。插件关闭后回收 Host，
无插件闲置时不为扩展能力保留 WebView、JS heap、Wasmtime Engine 或 watcher。

### 4.3 核心差异

| 维度 | Kunkun | NovaHub | 判断 |
|---|---|---|---|
| 主渲染层 | 系统 WebView + Svelte | Slint 原生声明式 UI | 保留 Slint；不能仅凭框架名宣称更省内存，仍需参考机实测 |
| 空闲运行时 | 主 WebView、JS runtime 和前端状态常驻 | 主进程不含浏览器或 Wasmtime | NovaHub 的低内存路径更直接、预算更可控 |
| 插件执行 | iframe/Web Worker + kkrpc，可调用较宽宿主能力 | sibling Host + Wasmtime Component + WIT | NovaHub 的进程与能力边界更强 |
| UI 自由度 | 自定义 iframe 可使用多个 Web 框架 | 插件只能返回宿主官方 View | Kunkun 开发自由度更高；NovaHub 一致性、安全性和资源可预测性更高 |
| 搜索 | 多个 Fuse 实例分别过滤/排序 | 单一 matcher 和 Ranker 所有者 | 保留 NovaHub，避免多排序所有者 |
| 安装更新 | 解压、覆盖目录；升级先卸载再安装 | staging、版本目录、active/previous pointer、回滚 | NovaHub 已更稳健，不应回退 |
| 开发体验 | 多框架模板、快速 HMR、TypeScript 生态 | Rust/WIT SDK、构建链更严格 | 借鉴工具化，不引入生产 Web runtime |
| 兼容性 | package/API SemVer 与 RPC 序列化兼容分支 | WIT package SemVer + manifest host range | 保留 WIT 权威，补齐 golden contract matrix |

Tauri 使用系统 WebView，通常比捆绑 Chromium 的 Electron 更轻，但这不等于没有 WebView/JS 的常驻成本。
Kunkun 还会按扩展打开额外 WebviewWindow 或 Worker，因此不能用“Tauri 安装包较小”推导“完整进程树内存较低”。
在缺少同机数据时，本文不提供虚构的 MiB 对比；NovaHub 应继续以完整进程树 RSS、Private Working Set、
GPU/纹理内存、首帧和 P95 延迟作为裁决依据。

## 5. 值得借鉴的设计

### 5.1 显式区分命令交互形态

Kunkun 在 manifest 中区分 `UiIframe`、`UiWorker` 和 `HeadlessWorker`：

- `packages/api/src/models/extension.ts:28-35`
- `packages/api/src/models/manifest.ts:81-100`

这个设计的价值在于，宿主在执行前就知道命令是否需要 UI、由谁渲染、是否有后台逻辑，
可以选择窗口、生命周期和资源预算，而不是加载代码后再猜测。

NovaHub 不应照搬这三个类型。建议在每个 `[[commands]]` 增加与技术栈无关的交互字段：

```toml
[[commands]]
id = "json.format"
interaction = "view" # view | one-shot
```

- `view` 使用现有 `open/update/close` 与宿主官方 View。
- `one-shot` 只执行一次有 deadline 的动作，返回标准结果、toast 或宿主导航指令，然后回收会话。
- 不提供生产 `background`、任意窗口或插件自渲染类型。

这会减少一次性命令创建完整 View 的成本，也让 Host 生命周期和超时策略更清楚。

### 5.2 依据权限构造能力表面

Kunkun 的 `constructJarvisServerAPIWithPermissions` 按 manifest 权限分别构造 clipboard、fetch、fs、
open、shell 等 API，而不是先暴露完整 API 再依赖调用方自律：

- `packages/api/src/api/server/index.ts:91-97`
- `packages/api/src/api/server/index.ts:127-213`

它还区分字符串权限与 scoped permission，并为文件、URL、命令提供 allow/deny 范围：

- `packages/api/src/permissions/schema.ts:116-151`
- `packages/api/src/utils/path.ts:30-102`

NovaHub 应借鉴“能力表面由权限生成”，但使用更小、更强类型的模型：

- 有效权限等于 `插件声明 ∩ 用户授予 ∩ 宿主策略`，任一层缺失即拒绝。
- HTTP 使用规范化 HTTPS Origin，不使用通配 URL 字符串。
- manifest 只声明用户选择文件的读写意图；运行时使用宿主签发、绑定插件身份的文件/目录句柄，
  不向插件暴露绝对路径别名。
- 剪贴板读写分权；进程、环境变量、原始 Socket 和任意 shell 继续不提供。
- 如果未来存在显式 deny overlay，deny 必须在所有 allow 之前裁决并有组合测试。

这个方案比复制 `$DESKTOP/**` 或 `$HOME/**` 更符合 NovaHub 的句柄式文件模型，也减少跨平台路径语义。

### 5.3 宿主拥有声明式 UI

Kunkun 的 Template UI 已覆盖 List、Form、Markdown、ActionPanel、Detail metadata、accessory、
dropdown、loading/progress 等常用交互，扩展只发送模型和事件，宿主负责渲染：

- `packages/api/src/ui/template/schema/`
- `packages/api/src/ui/template/components/`
- `packages/ui/src/components/extension/templates/`

这与 NovaHub 的方向一致，是最值得继续吸收的部分。NovaHub 当前 `ui-protocol` 已有 List、Grid、Detail、
Form、ActionPanel 和稳定 ID 校验，但模型仍偏薄：`FormField` 只有 id、label、required，List/Grid item
也只有标题与副标题，见 `crates/ui-protocol/src/lib.rs:3-107`。

建议补充以下宿主语义，而不是开放颜色、CSS、坐标或任意组件：

- List/Grid accessory：状态文本、图标资源句柄、badge、快捷键提示。
- Detail metadata：纯文本、link、tag 三种受限值类型。
- Form field：text、password、select、checkbox、switch，并明确初始值与校验错误。
- View state：loading、determinate progress、empty、recoverable error。
- ActionPanel：默认动作、次要动作、破坏性确认。
- 分页：宿主管理 cursor 和 `load-more` 事件，单页上限保持 100。

所有新增类型必须先进入 WIT，再生成 Rust SDK 绑定；不能让 SDK 或 Slint 组件反向成为协议来源。

### 5.4 局部更新与列表窗口化

Kunkun 的 List 模型支持 `inherits`，可以继承 items、detail、filter、sections、actions 或 defaultAction，
避免更新详情时重复发送整个列表：

- `packages/api/src/ui/template/schema/list.ts:167-184`
- `packages/api/src/ui/template/components/list-view.ts:361-405`

其 Svelte 列表使用 `@tanstack/svelte-virtual` 和 `overscan: 5`，把渲染节点数与数据量解耦：

- `packages/ui/src/components/extension/templates/list-view.svelte:160-176`
- `packages/ui/src/components/extension/templates/virtual-command-group.svelte:39-54`

NovaHub 不应把 `inherits` 变成第二套补丁协议。现有架构已经规定插件返回完整 View，宿主用稳定 ID
做内部差量；`StableList` 也已能在替换行时保留选择，见 `crates/ui-slint/src/lib.rs:771-848`。

正确落点是：

- 保持 WIT View 是唯一状态快照。
- 宿主按 stable ID 计算增删、移动和字段更新。
- Slint 使用虚拟列表/窗口化模型，只实例化可见行与小幅 overscan。
- 详情变化不重建未变化的列表行，不改变键盘选择和滚动锚点。
- 用 100、1,000、10,000 个宿主候选分别测量模型更新、帧时间和内存；插件单页仍受 100 项限制。

### 5.5 开发者工具是一等平台能力

Kunkun 提供 `create-kunkun`、`kksh verify`、worker/headless 模板以及 React、Vue、Svelte、Nuxt、
SvelteKit、Next 模板。相关入口包括：

- `apps/create-kunkun/index.ts:29-49`
- `apps/create-kunkun/index.ts:92-160`
- `apps/cli/cli.ts:29-40`
- `apps/cli/src/commands/verify.ts:9-100`

NovaHub 已实现 `plugin new/dev/check/test/pack/sign/install/update/rollback` 等命令，方向正确，
不需要复制多 Web 框架模板。值得继续补的是：

- 一份最小 View 插件和一份 one-shot 插件模板。
- manifest、WIT、生成绑定和 Host 行为共用的 golden contract fixtures。
- SDK/WIT/Wasmtime 精确版本兼容矩阵。
- 仅开发构建可用的显式 `plugin dev --watch`；默认仍为一次性构建和校验。
- 重载必须重新校验包、权限和资源上限，不能直接把开发目录挂入生产 Host。

开发 watcher 是开发者主动启动的短期工具，不进入 NovaHub 发布包的常驻进程树。

### 5.6 权限、来源和构建信息可解释

Kunkun 的权限检查器同时显示权限名、解释和原始 scoped JSON：

- `packages/ui/src/components/extension/PermissionInspector.svelte:16-39`

扩展详情还展示 GitHub Actions、source commit、workflow、Rekor/Sigstore transparency log 和镜像地址：

- `packages/ui/src/components/extension/StoreExtDetail.svelte:199-228`
- `packages/ui/src/components/extension/GitHubProvenanceCard.svelte:25-68`

NovaHub 已强制生产插件签名并保存包哈希，但用户侧的来源解释还不完整。可借鉴的原则是：

- 安装和更新前展示新增、收窄、扩大和移除的权限范围。
- 展示 publisher key ID、归档 SHA-256、来源仓库、source commit、builder/workflow、SBOM digest。
- 只有校验链真正通过时才显示“已验证”；缺失信息显示“未提供”，不能用绿色徽章暗示安全。
- provenance 字段必须在签名范围内；外部链接只用于解释，不参与本地信任裁决。

### 5.7 生命周期诊断产品化

Kunkun 会记录扩展产生的进程，并尝试在扩展退出时清理：

- `packages/api/src/events.ts:10-30`
- `packages/api/src/api/shell.ts:138-139`

它还提供 extension loading、extension window、mDNS 和 ORM troubleshooter：

- `apps/desktop/src/routes/app/troubleshooters/sidebar.svelte:14-34`
- `apps/desktop/src/routes/app/troubleshooters/extension-loading/+page.svelte:73-109`

NovaHub 已有 IPC request ID、Host timeout、结构化日志和进程树资源采集，但还缺少面向用户的统一诊断面。
建议增加宿主原生诊断页，读取一个有界环形事件缓冲区，展示：

- 插件安装、启用、加载、open/update/close、回收各阶段状态与耗时。
- Host PID、退出码、超时、崩溃、有限重试和 pending-delete 状态。
- 权限拒绝的能力名称和范围，不记录用户输入、文件内容或完整 URL。
- 当前版本、active/previous pointer、包哈希和来源验证状态。
- 一键生成可预览、可删减的本地诊断包。

该诊断面不需要后台服务、云遥测或新的 watcher；事件缓冲区必须设置条数和字符串长度上限。

## 6. 不应照搬的实现

| Kunkun 实现 | 风险或不适配点 | NovaHub 决策 |
|---|---|---|
| Tauri/WebView 主 Shell | 常驻 WebView 和 JS 状态扩大空闲内存及依赖面 | 拒绝；主 Shell 保持 Rust + Slint |
| 任意 iframe UI、每扩展 WebviewWindow | UI 一致性、资源预算、无障碍和隔离难以统一 | 拒绝；只允许宿主官方 View |
| Headless 命令运行在 Web Worker | Worker 不是进程、系统能力或内存隔离 | 拒绝；one-shot 仍在 Wasmtime sibling Host |
| Deno、Node、任意 shell/FFI/环境变量 | 能力面过宽，难以形成最小权限和确定性卸载 | 拒绝；不进入 WIT Host API |
| NPM/JSR 包直接成为运行时插件 | 依赖树、安装脚本和供应链范围明显扩大 | 拒绝；保留签名 `.novahub-plugin` Component 包 |
| 多个 Fuse store 各自排序 | 多个 rank owner 会产生跨来源不可比较的分数 | 拒绝；继续由统一 Ranker 合并 |
| 覆盖安装先删除旧目录 | 失败时旧版本不可用，难以回滚 | 拒绝；保留 staging + immutable versions + pointer |
| 生产常驻 HMR/watcher | 增加空闲进程、文件句柄和唤醒 | 拒绝；watch 仅显式开发模式可用 |

### 6.1 Kunkun 权限实现的警示

Kunkun 的路径专用校验 `verifyGeneralPathScopedPermission` 在
`packages/api/src/utils/path.ts:135-149` 做到了 deny-first；但通用 `verifyScopedPermission` 在
`packages/api/src/utils/path.ts:177-194` 先匹配 allow，并在命中后提前退出，因此后续 deny 可能无法覆盖。
注释与实现存在不一致风险。

同时，`packages/api/src/utils/__tests__/path.test.ts:4-7` 的测试体已被注释，无法为路径别名、
glob、平台分隔符和 allow/deny 组合提供充分证据。

NovaHub 的教训不是“复制后修掉循环顺序”，而是尽量避免通用字符串策略语言。优先使用类型化 Origin、
宿主句柄和集合交集；若确实引入 deny overlay，必须用真值表和跨平台边界测试固定优先级。

### 6.2 Kunkun 安装与升级的警示

Kunkun 安装时会在复制新版本前递归删除现有扩展目录：

- `packages/extension/src/install.ts:49-86`

其升级路径是先卸载再安装：

- `apps/desktop/src/lib/stores/extensions.ts:198-207`

这比 NovaHub 当前 `staging -> immutable version directory -> active.json.next -> atomic activate -> previous.json`
更弱。NovaHub 的实现位于 `crates/plugin-manager/src/lib.rs:426-605` 和
`crates/plugin-manager/src/lib.rs:1043-1072`，应作为不可回退的架构优势。

### 6.3 供应链展示不等于强制校验

Kunkun 的详情页能展示 Sigstore/Rekor 元数据，但在本次静态审阅覆盖的安装路径中，没有找到与展示字段
一一对应的强制 provenance 校验闭环。因此只能把它评价为优秀的信息展示，不能据此断言安装包已可信。

NovaHub 必须保持“先验证、后展示”：签名、哈希和 provenance 校验结果由宿主生成，UI 只渲染结果，
不根据包内自报字段自行授予“已验证”状态。

## 7. NovaHub 当前能力对照

| 能力 | 当前状态 | 证据或缺口 | 结论 |
|---|---|---|---|
| Rust + Slint 主 Shell | 已实现 | `apps/novahub-app`、`crates/ui-slint` | 保持 |
| Wasmtime 仅在 sibling Host | 已实现 | `apps/novahub-plugin-host`、发布依赖检查 | 保持 |
| WIT 唯一 ABI | 已实现基础链路 | `wit/novahub-plugin/plugin.wit` | 保持并扩展 |
| 统一搜索排序 | 已实现 | `crates/search/src/lib.rs:36-128` 复用单个 matcher | 优于 Kunkun |
| 官方 View 与稳定 ID | 宿主模型已实现 | `ui-protocol` 校验 ID；Slint `StableList` 保留选择、focus、scroll anchor，并提供有界窗口 | 继续接入真实 Slint 控件并完成虚拟列表实测 |
| View 控件表达力 | 已实现 MVP 代码级最小集 | WIT/SDK 覆盖 List/Grid/Detail/Form/ActionPanel/Loading/Progress/Error、附件、游标分页和默认动作 | 继续补齐无障碍与资源实机证据 |
| 插件命令交互类型 | 已实现 | manifest、应用搜索/宠物动作路由、Host `run` 和 CLI 双模板 | 保持 `view/one-shot`，禁止后台类型 |
| 权限作用域 | 运行时闭环已实现 | `DeclaredPermission`/`EffectiveGrant`、逐次 broker、持久化撤销、typed KV、HTTP/Files Component E2E | MVP 继续完成真实平台 smoke 与发布矩阵 |
| 安装前权限摘要 | 已实现基础模型 | CLI 使用规范化声明计算 added/expanded/narrowed/removed diff | 补充 scope、reason 和用户拒绝交互 |
| 原子安装、回滚 | 已实现 | staging、active/previous pointer、pending-delete | 保持，禁止回退 |
| 签名与哈希 | 已实现 | Ed25519、SHA-256、生产安装强制签名 | MVP 展示本地信任事实；Phase 2 补扩展 provenance |
| SDK/CLI | 已实现 MVP 闭环基础 | new 支持 `--interaction view|one-shot`，另有 dev/check/test/pack/sign/install/update/rollback | MVP 补齐 golden matrix；Phase 2 补可选 dev watch |
| 生命周期诊断 | 有界导出模型已实现 | 有界事件环、Host PID、阶段、错误分类、本地摘要、删减和 JSON 导出已有 | MVP 补齐原生诊断页交互 |
| 完整进程树内存证据 | 采集能力已实现 | reference-machine P95 仍是发布环境证据缺口 | 继续测量，不能用静态推断替代 |

权限模型的结构化重构已经落地：`PluginManifest` 保留 `DeclaredPermission` 的 scope/reason，
安装预览可计算权限 diff，未知能力和字段默认拒绝。`EffectiveGrant` 已接入真实 Component、sibling Host、
认证 IPC、App broker 与 host-owned adapter 调用链，HTTP/Files 也已有成功/拒绝 E2E。尚未完成的是
Windows/macOS 参考机上的真实 picker/TLS smoke 与发布证据；这不需要改变 Rust + Slint 与 sibling Host
的进程架构。

## 8. 分阶段落地方案

### MVP P0：权限契约与安装预览闭环（代码闭环已落地，真实平台证据待验收）

#### P0.1 强类型 PermissionSpec（已完成）

已新增共享领域模型，明确区分 manifest 中的 `DeclaredPermission` 和宿主运行时的 `EffectiveGrant`。
WIT 只定义 capability 调用及其参数，manifest 只声明所需能力，至少覆盖：

- `http.request { origins: [...] }`
- `files.read { source: "user-selected" }`
- `files.write { source: "user-selected" }`
- `clipboard.read`
- `clipboard.write`
- `notification.show`
- `storage { quota_bytes }`

实现要求：

- manifest parser 不得丢弃 scope value，未知能力或未知字段默认拒绝。
- 安装预览使用规范化的 `DeclaredPermission`；授权存储、Plugin Host linker 和 capability broker
  使用由声明、用户选择与宿主策略计算出的 `EffectiveGrant`，两者不得混为一个字符串集合。
- broker 每次调用按插件身份和参数重新校验；SDK 的本地 `require` 只能改善错误信息，不能成为安全边界。
- 文件仍使用宿主句柄，网络仍按规范化 Origin，不能引入 shell 或路径 glob API。

内存兼顾：权限在安装时规范化为有界、小型、可共享的不可变结构；只为活跃会话创建调用上下文，
不增加常驻运行时或后台线程。

#### P0.2 权限 diff 与解释 UI

安装/更新预览必须区分新增、扩大、收窄、移除和未变化权限，并显示能力用途与精确范围。
用户拒绝新增或扩大权限时保留旧 active pointer。

验收：同一组 contract fixtures 必须贯穿 parser、preview、grant store、broker、SDK 和 UI snapshot；
覆盖未知权限、Origin 规范化、重定向逃逸、失效文件句柄、用户撤销和 deny overlay（若存在）。

### MVP P1：命令、View、渲染和诊断

#### P1.1 命令交互类型（已完成）

command manifest 和 WIT lifecycle 已增加 `view | one-shot`。`one-shot` 沿用相同 Component、权限、
fuel/epoch、IPC 认证和 deadline，不获得后台常驻能力。

验收：one-shot 不创建插件 View 模型；执行完成、取消或超时后 Host 被回收；无插件闲置时进程树不出现 Host。

#### P1.2 扩充官方 View 语义

按 5.3 的最小集合扩展 accessory、typed metadata、form controls、loading/progress、default action 和分页。
每种节点拥有稳定 ID、长度/数量限制、键盘行为和无障碍名称。

内存兼顾：所有集合有上限，图片只传资源句柄，Markdown 不自动联网，Slint 只实现固定语义组件。

#### P1.3 完成 stable-ID 差量与虚拟列表

差量算法保留在宿主内部，不发布第二套 patch DSL。Slint 模型按可见窗口渲染，选择和滚动锚点绑定 stable ID。

验收：只更新详情时列表行实例不整体重建；插入、删除、移动和异步返回不改变当前键盘选择；
在固定数据集和 DPI 下记录帧时间、RSS 与 GPU/纹理内存。

#### P1.4 宿主原生 troubleshooter

增加有界生命周期事件模型和 Slint 诊断页，覆盖安装、权限、Host、会话、超时、崩溃、回收、回滚和待删除。

内存兼顾：默认只保留固定条数的结构化摘要；不保留用户内容，不启动额外进程，不上传遥测。

#### P1.5 SDK golden matrix

MVP 为最小 `view` 与 `one-shot` 模板建立
`new -> check -> test -> pack -> sign -> install -> real Host round-trip` 的 golden 测试矩阵，并锁定
SDK/WIT/Wasmtime 精确版本兼容关系。首个稳定 manifest/WIT 发布前必须完成，避免把契约兼容债带入 Phase 2。

### Phase 2：开发体验与供应链解释

#### P2.1 可选开发重载

在 MVP golden matrix 基础上增加显式、仅开发模式可用的 `plugin dev --watch`。

内存兼顾：watcher 由开发者主动运行，生产构建不包含自动 watcher，NovaHub 主程序不因开发能力增加常驻线程。

#### P2.2 签名范围内的 provenance

为包元数据增加可选 source repo、commit、builder identity、workflow、SBOM digest、transparency log reference，
并与 publisher key ID、包哈希、验证时间一起显示。

验收：篡改任一受签字段必须使安装失败；离线时仍能基于本地签名完成信任裁决；外部服务不可用只影响链接查看，
不能影响已完成的本地验证。

## 9. 明确不进入路线图

以下内容不因参考 Kunkun 而重新打开：

- Tauri/WebView 主 Shell 或无障碍 fallback。
- iframe、自定义 HTML/CSS 或插件自行创建窗口。
- JavaScript、Deno、Node、NPM/JSR 生产插件运行时。
- 任意 shell、环境变量、原始 Socket、后台定时器。
- 每个来源独立搜索排序器。
- 安装前删除当前版本或升级先卸载。
- 为 HMR、诊断或更新保留生产常驻辅助进程。

复杂编辑器未来若达到现有架构文档规定的量化触发器，只能单独评审独立、按需、可回收的 Host；
它不能替换核心 Shell、改变 WIT View 所有权或成为普通插件默认能力。

## 10. 验收门槛

本建议只有同时满足下列条件才算兼顾了 Kunkun 的优点和 NovaHub 的性能目标：

1. `novahub-app` 依赖树继续不含 Tauri、WebView、Wasmtime、JS runtime 和插件 SDK。
2. 无插件闲置时不启动 Plugin Host；完整进程树闲置 RSS P95 继续以 70 MiB 为硬门槛。
3. 一个和四个会话分别记录主进程、Host、完整进程树、Private Working Set 与可获取的 GPU/纹理内存。
4. 权限 contract matrix 覆盖 parser、preview、persist、broker、SDK 与 UI，默认拒绝行为一致。
5. View 的节点、文本、页大小、资源和 IPC 包大小保持有界，stable-ID 差量不引入第二协议所有者。
6. one-shot、view、超时、取消、崩溃和 close 都能确定性结束会话并回收 Host。
7. provenance 展示严格区分“已验证”“未提供”“校验失败”，UI 不自行推断信任。
8. 开发 watcher 和诊断页不改变生产空闲进程拓扑。

## 11. 最终判断

Kunkun 证明了插件型效率工具需要完整的命令分类、声明式组件、权限解释、开发工具和故障诊断；
它也展示了任意 Web UI、宽系统能力和覆盖式安装会带来的边界成本。

NovaHub 当前架构方向不需要改变。需要调整的是协议和产品完整度，而不是渲染技术和进程主干：
MVP 先完成强类型权限与安装预览闭环，再完成命令交互、官方 View、虚拟列表、诊断和 SDK golden matrix；
Phase 2 再补开发重载、多语言 SDK 与扩展 provenance。这样可以吸收 Kunkun 的成熟思想，同时继续把
WebView、JS runtime、Wasmtime 常驻和多排序所有者排除在低内存基线之外。

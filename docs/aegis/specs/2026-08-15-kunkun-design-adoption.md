# NovaHub MVP Kunkun 优点采纳设计规格

## 状态

本规格记录 2026-08-15 已确认的设计调整，是后续实施计划和验收的权威输入。命令分类、强类型权限、
Plugin Host → App capability broker 主路径、持久化授权/撤销、卸载清理、typed KV、session 失效、
opaque token 拒绝、有界 HTTP、有效文件 token、显式 session 清理和逐跳 redirect 矩阵已在 MVP 代码与
真实 Component E2E 中落地；完整官方 View、真实 Slint 窗口化、平台 smoke、实机资源和发布证据仍未宣称完成。

## TaskIntentDraft

### 目标

在不引入 Tauri/WebView、JavaScript 生产运行时或常驻插件运行时的前提下，把 Kunkun 中成熟的命令分类、权限解释、声明式 UI、开发工具和生命周期诊断思想吸收到 NovaHub，并在首个稳定 manifest/WIT 前完成影响长期兼容性的 MVP 契约。

### 范围

本次设计覆盖插件 manifest、WIT 生命周期、权限领域模型、官方 View、Slint 更新策略、安装/更新预览、信任摘要、诊断页、SDK/CLI 测试闭环和阶段路线图；代码实现以权威架构文档和本规格的契约边界为准。

### 主要风险

- 结构化权限若在 parser、授权存储、broker、SDK 和 UI 中形成不同模型，会产生授权漂移。
- `one-shot` 若建立独立运行时或后台生命周期，会破坏按需 Host 和低内存边界。
- View 差量若成为公开 Patch DSL，会产生第二个 UI 状态所有者。
- 诊断与开发 watcher 若常驻，会增加空闲内存、线程、文件句柄和隐私面。
- provenance 若只展示包内自报字段，会把信息展示误当成信任校验。

## BaselineReadSetHint

- 权威产品与架构：`docs/01-product-and-principles.md`、`docs/02-mvp-requirements.md`、`docs/03-system-architecture.md`。
- 插件与体验：`docs/04-plugin-platform.md`、`docs/05-ui-ux-design.md`。
- 安全与交付：`docs/07-security-privacy.md`、`docs/08-quality-release.md`、`docs/09-roadmap-and-final-form.md`。
- 参考分析：`docs/14-kunkun-reference-analysis.md`，评审 Kunkun 版本 `cb8af4930dfbd1ca9f252e73475faa6416e20428`。
- 低内存基线：`docs/aegis/baseline/2026-08-14-architecture-hardening-baseline.md`。

## ImpactStatementDraft

- 契约影响：manifest 新增结构化权限与 command `interaction`；WIT 新增 `run` 和 `command-result`，扩展官方 View 语义。
- 所有权影响：`plugin-manager` 唯一拥有 `DeclaredPermission`、`EffectiveGrant` 和 `PermissionDiff`；WIT 拥有 ABI；宿主拥有差量、渲染、信任状态和权限裁决；Plugin Host 只拥有 Wasmtime 执行。
- 进程影响：保持 `novahub-app` + 用户动作期间按需 sibling Plugin Host；不新增常驻服务、watcher、WebView 或 JS runtime。
- 数据影响：新增有界授权记录、权限 diff 与默认 256 条诊断事件；支持用户选择最近 N 条/仅失败的本地 JSON 导出；不记录用户输入、文件内容、完整 URL 或插件表单值。
- 兼容影响：`interaction` 缺省为 `view`；未知权限和字段默认拒绝；首个稳定契约前完成破坏性调整，不为错误的字符串权限模型保留双轨。

## 方案比较

| 方案 | 收益 | 代价 | 决策 |
|---|---|---|---|
| 所有 Kunkun 能力进入 MVP | 一次覆盖最广 | watcher、多语言和扩展 provenance 放大交付面 | 拒绝 |
| 所有增强延后到 Phase 2 | MVP 范围最小 | 首个稳定 manifest/WIT 产生兼容债，权限闭环不足 | 拒绝 |
| 契约关键能力进 MVP，生态增强延后 | 先固定安全、ABI、渲染和测试边界，保持交付可控 | MVP 仍需跨模块实施 | 采用 |

## MVP 决策

### 强类型权限和权限 diff

- `EffectiveGrant = DeclaredPermission ∩ UserGrant ∩ HostPolicy`。
- HTTP 使用规范化 HTTPS Origin；文件使用绑定插件身份的用户选择 handle；Clipboard 读写分权。
- 未知能力/字段默认拒绝，发布者 reason 不参与裁决，broker 每次调用重新验证。
- 安装/更新展示新增、扩大、收窄、移除和未变化；拒绝新增或扩大时保留旧 active pointer。

### 平台 Capability 适配器

- 调用链固定为 `WIT → sibling Host → 认证 IPC → App-owned broker → host-owned adapter`；Host 不打开
  SQLite、不持有真实文件路径或网络客户端。
- MVP HTTP 仅支持有界 HTTPS `GET`：最多 5 次重定向，每跳重新授权，连接/读取/总时限为 3/5/10 秒，
  UTF-8 响应上限 512 KiB。
- MVP Files 只读取系统选择器签发 token 对应的 UTF-8 文本，单次上限 512 KiB；token 绑定 plugin/session，
  在 close、Host crash、撤销、禁用和卸载后失效。
- adapter 使用现有有界 worker，不引入 Tokio runtime、常驻线程池、后台服务或第二套 ABI。HTTP 库/平台
  实现必须先比较二进制增量、冷启动、完整进程树 RSS、后台线程和 TLS 后端。

### 命令和 View

- command `interaction` 为 `view | one-shot`，缺省 `view`。
- `one-shot` 与 `view` 共用 Component、权限、IPC、fuel/epoch、deadline 和 Host 回收边界；不创建完整 View 或后台任务。
- 官方 View 增加受限 accessory、typed metadata、Form 控件/状态、ViewState、ActionPanel 和 cursor pagination。
- 插件返回完整 View；宿主按 stable ID 差量并窗口化渲染，不发布 Patch DSL。

### 诊断和开发交付

- Slint 原生诊断页读取默认 256 条的有界事件缓冲，支持筛选、恢复动作和本地诊断包预览/删减；宿主导出接口已实现，页面绑定仍为 MVP P1。
- MVP 交付最小 `view`/`one-shot` 模板、golden contract fixtures、真实 Host E2E 和 SDK/WIT/Wasmtime 精确版本矩阵。
- MVP 信任摘要只展示本地已验证的 publisher key ID、归档 SHA-256、签名状态、API 兼容性和开发者模式状态。

## Phase 2

- 显式、仅开发模式的 `plugin dev --watch`，每次重载重新校验包、权限和资源上限。
- 多语言 SDK 与模板，但必须通过同一 WIT 的跨语言一致性测试。
- 签名范围内的 source repo、source commit、builder identity、workflow、SBOM digest 和 transparency log reference，以及明确区分已验证/未提供/失败的可视化。

## Non-goals

- Tauri/WebView 主 Shell、局部 fallback、iframe、任意 HTML/CSS 或插件自建窗口。
- JavaScript、Deno、Node、NPM/JSR 生产插件运行时。
- 任意 shell、环境变量、原始 Socket、后台插件、定时任务或全局监听。
- 每来源独立 Ranker、覆盖安装、升级先卸载、生产 watcher/HMR。
- 在 MVP 把包内自报 provenance 显示为已验证。

## Ownership Map

| 对象 | 唯一所有者 | 不得拥有 |
|---|---|---|
| manifest/WIT 契约 | `plugin-manager` parser + WIT package | SDK 或 Slint 私有类型 |
| 运行时授权 | Plugin Manager / Capability Broker | 插件 SDK、本地 Mock 或 UI |
| View 状态 | 插件完整快照 | 宿主公开 Patch DSL |
| 差量、虚拟化和渲染 | `ui-slint` | 插件 CSS、坐标或窗口控制 |
| Wasmtime 生命周期 | sibling Plugin Host | `novahub-app` 主进程 |
| 诊断事件与信任事实 | 宿主 | 包内自报状态或外部网络服务 |
| 搜索合并和排序 | 单一 Query Coordinator / Ranker | 插件或来源私有排序器 |

## 验收标准

1. 权限 fixtures 贯穿 parser、preview、grant store、broker、SDK 和 UI，未知输入一致拒绝。
2. `view`/`one-shot` 在成功、取消、超时、崩溃后确定性回收，空闲进程树不保留 Host 或 Wasmtime。
3. View 节点、文本、集合、资源、页大小和 command-result 均有上限；差量不形成第二协议。
4. 100/1,000/10,000 项测试证明选择和滚动锚点稳定，详情更新不整体重建列表。
5. 诊断缓冲容量、字段长度、敏感信息禁录和诊断包删减通过测试，不新增常驻进程。
6. `view`/`one-shot` 模板完成 `new -> check -> test -> pack -> sign -> install -> real Host round-trip`。
7. `novahub-app` 依赖树继续不含 Tauri、WebView、Wasmtime、JS runtime 和插件 SDK。
8. 无插件、无更新任务时完整进程树闲置 RSS P95 继续以 70 MiB 为硬门槛。

## 修复轨道

P0 代码闭环已经覆盖有界 HTTP、有效文件 token、宿主 adapter 与真实 Component E2E，下一步只补
Windows/macOS 真实 picker/TLS smoke。P1 完成官方 View 完整语义、真实 Slint 差量/虚拟化、诊断包交互和
参考机验收。每一步先更新唯一契约与 fixtures，再让宿主、Host、SDK 和 UI 对齐；P0/P1 都属于 MVP，
Phase 2 不得成为遗留安全或发布门槛的收容区。

## 退役轨道

- 退役只保留 capability 名称、丢弃 scope value 的字符串权限模型，不维持新旧授权双写。
- 不引入通用 UI Patch DSL、生产 watcher、包内自报“已验证”路径或 `one-shot` 独立执行管线。
- 已有 staging、immutable versions、active/previous pointer 和单一 Ranker 保持不变，不因采纳 Kunkun 思想而退化。

## 残余风险

- 结构化权限、交互分类、Host one-shot 路由、双模板、运行时 broker、持久化 user grant/撤销、typed KV、
  redirect-per-hop、session 失效、HTTP 和有效文件 token 成功/拒绝路径已进入代码；真实平台 smoke 仍待参考机。
- Slint 的虚拟化、无障碍和不同渲染后端仍需参考机探针决定实现细节。
- 一个与四个插件会话的完整进程树资源上限仍需在 M1 前冻结。
- 扩展 provenance 的签名格式与离线验证策略留待 Phase 2 独立设计。

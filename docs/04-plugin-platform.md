# NovaHub 插件平台与 SDK

## 1. 插件模型

MVP 的插件是一个版本化 Rust/WASM 包。UI 状态和业务逻辑共同编译成单个 WebAssembly Component，由同一清单、版本和生命周期管理。这里的“前后端均可插拔”指插件视图和业务能力随包安装或卸载；Slint 渲染器与官方组件库属于宿主平台，不随插件替换。

## 2. 包格式

扩展名为 `.novahub-plugin`，内容是可重复构建的归档：

```text
com.example.translate-1.2.0.novahub-plugin
├── novahub.toml
├── plugin.wasm
├── assets/
├── LICENSE
└── signature.ed25519
```

清单示例：

```toml
manifest_version = 1
id = "com.example.translate"
name = "Translate"
version = "1.2.0"
plugin_api = ">=1.1, <2.0"
entry = "plugin.wasm"
platforms = ["windows", "macos"]
kind = "command"

[[commands]]
id = "translate"
title = "翻译"
subtitle = "翻译文本"
keywords = ["translate", "翻译"]
interaction = "view"

[permissions.http]
origins = ["https://api.example.com"]
reason = "翻译用户提交的文本"

[permissions.files]
read = "user-selected"
write = "user-selected"
reason = "打开和保存用户选择的文件"

[permissions.clipboard]
access = ["read", "write"]
reason = "读取选中文本并复制翻译结果"
```

插件 ID 使用反向域名格式，安装后不可更改。命令 ID 只需在插件内唯一；宿主 canonical ID 为 `plugin:<plugin_id>:<command_id>`，用于搜索、选择和动作路由。

`interaction` 取值为 `view | one-shot`，缺省为 `view` 以兼容已有清单。它描述用户交互形态，不改变插件技术栈：两种命令都运行同一个 WebAssembly Component，并受同一权限、IPC 和资源限制约束。MVP 不接受 `background` 或插件自建窗口类型。

`kind` 缺省为 `command`，保持旧包兼容。桌面宠物使用 `kind = "desktop-pet"`，可以是只包含 `pet.json` 与受限资源的声明式包并省略 `entry`；若同时提供普通命令，仍通过同一 WIT、权限和按需 Plugin Host 运行。宠物浮层、显示/隐藏、位置、全局快捷键和快捷动作栏由宿主拥有，完整契约见 `docs/aegis/specs/2026-08-14-desktop-pet-plugin-design.md`。

安装后的桌面宠物由宿主通过 active pointer 重新校验 `novahub.toml`、`pet.json` 和 SVG 帧，再由 `pet use <id>` 切换并持久化选择。单帧最多 2 MiB、全部活动帧最多 8 MiB；宿主向 Slint 传递受限字节，不把安装路径、窗口句柄或渲染 API 暴露给插件。

## 3. WIT 世界

WIT 是协议唯一权威，建议拆分为稳定接口：

当前 `plugin` world 还包含一个可选使用的 `capabilities` import，提供
`read-clipboard`、`write-clipboard`、`http-get` 与 `storage-write`。组件只有在
实际导入这些函数时才会触发宿主实现；每次调用都经过
`CapabilityBroker`，按插件身份、`EffectiveGrant`、HTTPS Origin、宿主文件句柄和
Storage quota 重新裁决。Plugin Host 默认上下文使用空 grant，未完成用户授权或
原生 adapter 接线时明确返回拒绝，不把 SDK 的本地 `require` 当作安全边界。

用户授权以规范化 `DeclaredPermissions` JSON 存入宿主 `SQLite` 的
`plugin_user_grants` 表。首次安装审批只初始化一次；后续版本新增或扩大声明时，
旧 grant 与新声明求交集，不会静默获得新能力。显式空 grant 表示全部撤销，不能
通过删除记录表示；Shell 提供 `plugin permissions <id>`、
`plugin revoke <id> [capability|all]` 和 `plugin approve <id>` 作为 MVP 管理入口。
授权表只由主进程读写，Plugin Host 不得直接访问数据库。

```text
world plugin {
  import storage;
  import http;
  import clipboard;
  import files;
  import notification;
  import logging;

  export lifecycle;
  export commands;
}
```

插件导出的核心操作：

- `initialize(context) -> result<metadata, plugin-error>`：加载时协议握手，不执行长期任务
- `open(command-id, context) -> result<session-view, plugin-error>`：创建短生命周期会话（WIT 导出名为 `open-view`，避免 WASI libc 符号冲突）
- `update(session-id, event) -> result<view-update, plugin-error>`：处理输入或动作并返回新视图
- `close(session-id)`：释放插件内部会话状态（WIT 导出名为 `close-plugin`）
- `run(command-id, context) -> result<command-result, plugin-error>`：执行 `one-shot` 命令并返回有界文本、复制值、Toast 或宿主导航意图；不能返回任意 UI

MVP 的 `view-update` 返回完整官方 View。宿主使用稳定节点 ID 对 Slint 模型进行差量更新；同一 ID 的排序、增删和字段更新不得无条件重建整棵组件树。协议层暂不发布通用 UI Patch DSL，差量算法是宿主内部实现。

`one-shot` 不调用 `open/update/close`，但仍执行 initialize、Component 校验、能力链接、fuel/epoch、IPC 认证和 deadline。完成、取消或超时后销毁 Store 并回收 Host；它不能创建 Session View、后台任务或长期资源。需要富内容、表单或导航栈的命令必须使用 `view`。

## 4. 官方 UI 协议

插件只能返回以下顶层视图：

- `list-view`：分区、列表项、状态文本、图标资源、badge、快捷键提示和分页游标
- `grid-view`：固定列策略、网格项、受限附件和主动作
- `detail-view`：安全 Markdown、text/link/tag 元数据、资源引用
- `form-view`：text、password、select、checkbox、switch、初始值、helper、校验错误和提交
- `empty-view`：标题、说明和最多一个恢复动作

公共结构包括 `navigation-title`、`search-bar`、`view-state`、`action-panel`、`toast` 和 `resource-ref`。`view-state` 只包含 loading、determinate progress、empty 和 recoverable error；`action-panel` 明确默认动作、次要动作和需要宿主确认的破坏性动作。插件不能设置任意坐标、字体、颜色、阴影、动画或窗口尺寸。

协议限制：

- 单次 View 编码后不超过 1 MiB
- 单页列表默认最多 100 项，使用游标加载更多
- 所有交互节点必须有当前会话内稳定 ID
- 所有集合、字符串、metadata、附件和 `command-result` 都有协议级数量或字节上限
- Markdown 禁止原始 HTML、脚本、外部 iframe 和自动网络加载
- 图片必须是包内资源或经宿主 HTTP API 获取的缓存资源句柄
- 宿主校验深度、字符串长度、项数和资源尺寸后才渲染

## 5. 事件与会话

```text
用户打开命令 → initialize（必要时）→ open → View
用户输入/点击 → Event → update → View
返回/关闭/卸载 → close → 销毁 Store 或会话
用户执行 one-shot → initialize（必要时）→ run → command-result → 销毁 Store 并回收 Host
```

事件携带 `session_id`、`view_revision` 和 `event_id`。宿主丢弃过期 revision 返回，避免慢请求覆盖新状态；用户离开视图即取消未完成调用。插件不能依赖进程常驻保存重要状态，需持久化的数据必须写入宿主 storage API。

超时分为两种不可混用的预算：

- 纯 WASM 计算由 Store fuel、epoch interruption 和默认 2 秒硬 deadline 共同限制
- Component 首次加载/编译使用独立的 30 秒 IPC 预算，以覆盖未优化开发构建的冷启动；`open/update/close` 仍使用 2 秒响应预算，超时只回收当前 sibling Host 并允许一次有限重试
- 插件等待宿主 HTTP、文件等异步能力时不消耗计算 fuel；每项 IO 使用 capability broker 配置的独立 deadline、取消和响应大小上限

计算预算耗尽只销毁对应 Store；IO 超时取消对应宿主操作并返回可重试或不可重试的确定性错误，不能把慢网络误判为计算死循环。

## 6. Capability 权限

权限使用两个不可混用的强类型：

- `DeclaredPermission`：清单声明的能力需求及规范化范围，用于安装预览和授权请求。
- `EffectiveGrant`：当前调用可使用的运行时授权，由声明、用户授权和宿主策略共同计算。

```text
EffectiveGrant = DeclaredPermission ∩ UserGrant ∩ HostPolicy
```

清单解析器必须保留 scope value，不能把结构化权限降为 capability 名称集合。未知能力、未知字段、非法 Origin、超限集合或不支持的 scope 默认拒绝。发布者提供的 `reason` 只用于解释，明确标注为“发布者说明”，不参与授权裁决。

| 能力 | MVP 规则 |
|---|---|
| Storage | 仅插件命名空间，配额默认 10 MiB |
| HTTP | 仅有界 HTTPS `GET`；只访问清单 Origin，禁止裸 IP、本地网段和凭据注入，每次重定向重新授权 |
| Clipboard | 读写分别声明；不开放剪贴板历史 |
| Files | 系统文件选择器返回会话 token；MVP 只读有界 UTF-8 文本，不接受或返回任意绝对路径 |
| Notification | 用户动作触发的有限通知，不得用于后台推送 |
| Logging | 自动标记插件 ID，发布构建做敏感字段过滤 |

MVP 明确不提供进程启动、环境变量、原始 Socket、全局快捷键、系统事件订阅、无界目录扫描和后台定时器。

HTTP scope 是规范化 HTTPS Origin；重定向后的每一跳都重新授权。文件清单只声明 `user-selected` 读写意图，实际访问必须使用宿主签发并绑定插件身份、权限和生命周期的 handle。Clipboard 读写分权。SDK 的本地 `require` 只负责提前返回友好错误，Capability Broker 在每次调用时仍须校验插件身份、`EffectiveGrant` 和具体参数。

MVP 的 HTTP 与 Files 不是通用客户端：HTTP 限定 `GET`、5 次重定向、3 秒连接、5 秒读取、10 秒总时限和
512 KiB UTF-8 响应；Files 限定系统选择器签发的会话 token 和 512 KiB UTF-8 文本。App 在有界 worker 中
执行 adapter，Plugin Host 只转发 typed WIT 请求。任何实现都不得带来常驻网络 runtime、后台下载、路径泄漏
或新的 ABI；具体平台/库选型必须通过依赖体积、冷启动和完整进程树内存比较后确定。

桌面宠物不会放宽以上限制。声明式动画由主程序渲染，宠物插件不能自行注册全局快捷键、监听桌面输入或通过可选 WASM 命令建立后台循环。

## 7. 安装与更新事务

```text
读取归档
→ 防 Zip Slip/炸弹校验
→ 校验清单、哈希、签名和 API 范围
→ 计算并展示权限 diff 与本地信任事实
→ 解压到 staging
→ Wasmtime 预编译与实例化检查
→ 原子切换 active.json
→ 更新命令索引
→ 异步清理旧版本
```

插件目录按版本保存，更新失败时 active 指针仍指向旧版本。权限 diff 必须区分新增、扩大、收窄、移除和未变化，并逐项展示 capability、宿主解释、精确范围、发布者说明和拒绝后的影响；新增或扩大权限必须再次确认，拒绝后保留旧 active pointer。

MVP 安装预览只把 publisher key ID、归档 SHA-256、签名状态、插件 API 兼容性和开发者模式状态作为本地信任事实。UI 的“已验证”只能来自宿主校验结果；包内自报的 source、commit、builder、workflow、SBOM 或 transparency log 字段在扩展 provenance 闭环完成前不得提升信任状态。

## 8. 禁用与卸载

禁用停止新会话并关闭活跃会话，但保留代码、设置和权限。卸载由宿主执行：

1. 将状态置为 `unloading` 并拒绝新请求
2. 从搜索索引移除命令，退出当前插件视图
3. 取消请求、关闭会话、撤销资源句柄
4. 销毁插件 Store；必要时终止 Plugin Host
5. 删除安装版本、编译缓存、存储命名空间、资源缓存
6. 删除系统凭据、授权记录和使用历史
7. Windows 文件占用导致删除失败时登记 `pending_delete`，下次启动继续

默认不提供“保留数据”复选框。外部创建文件和远端账号数据不在自动删除承诺内，卸载提示需明确这个边界。

## 9. Rust SDK 与 CLI

Rust SDK 由 WIT 绑定生成代码加薄层构成，提供：

- 类型安全的 View Builder 和事件路由
- 错误类型、分页、资源句柄与取消辅助
- 本地测试用 Mock Host
- SDK 与协议版本矩阵
- 最小 `view` 与 `one-shot` 插件模板
- manifest、WIT、生成绑定和 Host 行为共用的 golden contract fixtures

`novahub` CLI 提供：

```text
novahub plugin new <dir> --interaction view
novahub plugin new <dir> --interaction one-shot
novahub plugin dev
novahub plugin check
novahub plugin test
novahub plugin pack
novahub plugin sign
```

命令边界保持轻量：`plugin new <dir> --interaction view|one-shot` 生成自包含的 Rust/WIT 脚手架，只会写入空目录，并将 manifest、Rust 绑定和最小实现同步为同一交互类型；`plugin check <dir|archive>` 在临时目录中完成清单、API 范围、ZIP 边界、资源和 WebAssembly Component 校验，不改变用户的 active pointer；`plugin test <dir|archive>` 执行同一套确定性包契约测试并输出摘要。`plugin dev <dir>` 是受控的一次打包与校验入口，MVP 不启动 watcher；开发者可重复执行它获得新的包，从而避免为开发重载引入生产空闲成本。

安装或更新前先执行只读预览：宿主只读取归档内的 `novahub.toml`，校验 ZIP 条目上限、清单格式和 `plugin_api` 兼容范围，不解压、不加载 Component，也不改变 active/previous 指针。预览必须展示名称、发布者、版本、插件类型、权限和命令摘要；清单缺失、归档损坏或宿主版本不兼容时，安装在预览阶段终止并返回确定性错误。

生产安装必须同时提供归档签名和发布公钥：

```text
novahub plugin install <archive.novahub-plugin> --signature <archive.ed25519> --public-key <publisher.pub>
```

`--allow-unsigned` 仅用于本地开发者模式；它不会成为发布安装或官方市场的默认路径。

`dev` 通过受控开发通道加载一次新包，但不绕过签名模式、权限和资源上限；`check` 校验清单、权限、WIT 兼容、包大小和确定性构建信息。显式 `plugin dev --watch` 属于 Phase 2，只在开发者主动运行时存在，且每次重载都重新校验包；它不进入生产构建或 NovaHub 常驻进程树。

开发环境可设置 `NOVAHUB_COMPONENT_FIXTURE` 指向已构建的 `wasm32-wasip2` Component，主 Shell 中输入 `plugin <input>` 运行一次受控的 `load/open/update/close` 往返。已安装插件使用 `plugin active <id> <input>`，由宿主读取签名校验后的 active pointer，再把组件路径交给 sibling Plugin Host。上述入口只验证应用到 Plugin Host 的 IPC 边界，不替代签名安装或生产插件清单；主进程仍不链接 Wasmtime。

迁移 CLI 同样默认不写用户数据：`novahub migrate --dry-run <export.json>` 只生成分类统计；用户明确选择 `--apply --db <path>` 后，宿主 Storage 才会在单个 SQLite 事务中写入 Quicklink/Snippet，任一记录校验或写入失败都会整体回滚。

## 10. 兼容策略

- `manifest_version` 控制包解析格式，未知主版本拒绝安装
- 每个 WIT package 声明精确 SemVer，例如 `package novahub:plugin@1.1.0;`；主版本内只做兼容扩展，破坏性变更发布新主版本 package
- `plugin_api` 使用标准 SemVer range 表达兼容范围；宿主在安装与每次启动时用同一解析实现双重校验，预发布版本只有被范围显式包含时才匹配
- Task 2 冻结契约时必须在 `rust-toolchain.toml`、`Cargo.lock` 和工具清单中锁定 Rust、Wasmtime、`wasm-tools`、绑定生成器及 WIT package 精确版本，并保存生成绑定的可复现校验结果
- 废弃接口至少跨一个稳定主版本，并提供编译期迁移提示
- UI 语义由协议控制，Slint 内部实现可演进而不破坏插件
- TypeScript SDK 若未来加入，必须通过同一套协议一致性与行为测试

## 11. 插件测试矩阵

- 清单和签名错误、路径穿越、压缩炸弹
- WIT 主版本不兼容与缺失导出
- 无限循环、内存增长、超大 View、深层 View、无效资源
- 纯计算耗尽 fuel/epoch 被终止；慢 IO 在未超过独立 deadline 时不被计算预算误杀
- 同一稳定节点 ID 的排序、增删和字段更新不无条件重建整棵 Slint 组件树
- 网络重定向到未授权域名和本地网段
- HTTP 成功路径、逐跳授权、重定向上限、连接/读取/总时限、响应大小和非 UTF-8 拒绝
- 文件选择器签发 token 后读取成功，以及伪造、跨插件、跨会话、关闭/崩溃后 token 失效
- 活跃会话禁用、更新、卸载和 Host 崩溃
- Windows/macOS 同一 View 的键盘、主题和无障碍快照
- SDK 示例插件从创建到打包的端到端测试
- SDK/WIT/Wasmtime 目标版本矩阵和生成绑定 round-trip 测试
- `new -> check -> test -> pack -> sign -> install -> real Host round-trip` 的 `view`/`one-shot` golden E2E

## 12. 交付阶段边界

MVP 必须冻结并实现：强类型权限与交集授权、权限 diff、本地信任事实、`view`/`one-shot`、官方 View 最小完整语义、stable-ID 宿主差量、虚拟列表、宿主原生诊断面、两类模板、golden fixtures、真实 Host E2E 和精确版本矩阵。这些内容会影响首个稳定 manifest/WIT 或发布安全边界，延后会形成兼容债。

Phase 2 再提供：显式 `plugin dev --watch`、多语言 SDK/模板，以及签名范围内的 source repo、source commit、builder identity、workflow、SBOM digest、transparency log reference 和对应可视化。它们必须复用 MVP 契约，不得引入生产 watcher、第二套 ABI、第二个权限裁决器或 Web 运行时。

无论阶段均不支持：Tauri/WebView 主 Shell 或 fallback、iframe/HTML/CSS 插件 UI、JavaScript/Deno/Node 生产运行时、NPM/JSR 直接执行、任意 shell/环境变量/原始 Socket、后台插件、定时任务、全局监听或插件自行创建窗口。

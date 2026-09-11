# NovaHub 系统架构

## 1. 架构目标

NovaHub 的架构优先级依次是：宿主稳定、交互低延迟、空闲低内存、插件隔离、跨平台一致、可演进性。任何为了插件自由度而让第三方代码进入 UI 进程、绕过权限网关或复制协议所有权的方案都不符合基线。

## 2. 技术选型

| 领域 | 选择 | 原因 |
|---|---|---|
| 语言 | Rust stable | 内存安全、原生性能、跨平台生态与 WASM 工具链统一 |
| 桌面 UI | Slint | 无浏览器运行时、声明式数据绑定、适合聚焦型桌面工具；生产渲染后端必须经 Phase 0 实测后显式固定 |
| 并发模型 | `std::thread` + 有界 `mpsc` + Slint Timer | MVP 不引入常驻异步运行时；插件 IPC 和宿主任务在有界 worker 中执行，降低空闲内存基线 |
| 插件运行时 | Wasmtime Component Model | WIT 强类型契约、资源限制、WASI 能力模型 |
| 插件协议 | WIT | 语言中立且可生成绑定，是唯一 ABI 权威 |
| 搜索匹配 | 低层 `nucleo-matcher` | 宿主复用单个 matcher/scratch，提供有界模糊排序，不引入高层常驻线程池 |
| 持久化 | SQLite + `rusqlite` | 单机事务、迁移、备份和诊断成本低 |
| IPC 编码 | Protobuf | 长度前缀、强类型、向后兼容字段演进 |
| 观测 | `tracing` + 本地滚动日志 | 结构化诊断，发布构建可关闭敏感字段 |
| 系统 API | `windows`、`objc2`/AppKit | 避免跨平台抽象泄漏关键桌面行为 |

## 3. 进程拓扑

```text
novahub-app（始终运行）
├── Slint Shell / Official View Renderer
├── PetWindow + PetRenderer（宿主拥有的轻量浮层）
├── Query Coordinator + Ranker
├── Built-in Providers
├── Platform Services
├── Plugin Manager / Capability Broker
├── Diagnostics Buffer / Trust Summary
├── ClipboardVault（有界 LRU/TTL 策略，默认 500 条、7 天，AEAD 密文）
└── SQLite / Credential Store
        │ 本地认证 IPC
        ▼
novahub-plugin-host（用户动作期间按需启动，close 后退出）
├── Wasmtime Engine / Component Linker
├── 每插件独立 Store、Limiter、Session
├── WIT Host API Adapter
└── IPC Client
        │
        ▼
*.wasm Component
```

主进程不能链接或执行第三方原生动态库。Plugin Host 不能直接打开主数据库、系统凭据库或任意用户文件；它只能发起带插件身份的 capability 请求。桌面宠物窗口、位置约束和动画时钟由主进程拥有，宠物包只能提供经过安装校验的声明式资源。

### 3.1 轻量渲染与窗口边界

- `PetWindow` 是 Slint 的第二个轻量窗口，不创建专用宠物进程，不引入 WebView。
- `SettingsWindow` 只在用户触发 `Cmd/Ctrl+,` 后按需创建；保存和关闭后隐藏并复用实例，启动关键路径不常驻第二个设置窗口。
- `ClipboardWindow` 只在执行 `clipboard.history` 后按需创建；历史列表固定为 8 个槽位，类型筛选、预览、复制、固定、暂停和清空都由宿主回调完成。
- 设置只通过宿主 API 写入 SQLite；主题、快捷键和剪贴板保留策略不能由插件读取或修改。主题保存后立即更新 Slint 语义表面，快捷键保存后先注册新组合键再替换旧注册，不要求重启。
- `PetRenderer` 只把当前状态映射为有限关键帧；完整动画最高 30 FPS，减少动画最高 6 FPS，静态模式为 0 FPS。
- `PetPosition` 使用逻辑坐标保存显示器 ID、边缘吸附和工作区约束。平台适配器只负责提供显示器工作区与最终原生窗口调用，不能自行复制位置策略。
- 宠物不可见时窗口隐藏且不刷新动画；缺失资源回退宿主静态帧，不能因为资源错误拖垮 Shell。

## 4. 建议工作区结构

```text
NovaHub/
├── apps/
│   ├── novahub-app/
│   ├── novahub-plugin-host/
│   └── novahub-cli/
├── crates/
│   ├── core-domain/
│   ├── search/
│   ├── storage/
│   ├── ipc/
│   ├── plugin-manager/
│   ├── plugin-runtime/
│   ├── ui-protocol/
│   ├── ui-slint/
│   ├── providers/
│   ├── migration/
│   └── platform/
│       ├── windows/
│       └── macos/
├── sdk/rust/
├── wit/novahub-plugin/
├── plugins/examples/
├── plugins/official/
├── docs/
└── xtask/
```

这是一份实施目标结构，不要求在设计阶段创建空目录。

## 5. 模块所有权与依赖方向

| 模块 | 唯一所有权 | 允许依赖 |
|---|---|---|
| `core-domain` | 命令、查询、动作、结果等稳定领域类型 | 标准库和小型通用库 |
| `ui-protocol` | 官方 View、Event、`command-result` 和资源句柄 | `core-domain` |
| `search` | 候选归一、匹配、排序、学习信号 | `core-domain` |
| `plugin-manager` | 安装、版本、启停、生命周期，以及 `DeclaredPermission`、`EffectiveGrant`、`PermissionDiff` | 领域、存储、IPC |
| `plugin-runtime` | Wasmtime 执行与资源限制 | WIT 生成绑定、IPC |
| `storage` | Schema、迁移、事务和插件命名空间 | 领域类型 |
| `platform/*` | OS 快捷键、窗口、应用、文件搜索、凭据 | 领域接口 |
| `ui-slint` | 宿主渲染、主题、焦点和无障碍映射 | UI 协议、应用服务 |
| `providers` | 内置提供器实现与结果映射 | 领域、搜索、存储、平台接口 |
| `migration` | 导入中立模型、源格式解析和事务编排 | 领域、存储 |
| `plugins/official` | 官方插件业务逻辑与清单 | WIT 生成绑定、Rust SDK；不得依赖宿主私有 crate |

依赖必须指向稳定契约。平台实现、Slint 组件和 Wasmtime 细节不能进入 `core-domain`；Rust SDK 不得反向成为 WIT 的协议来源。

## 6. 主搜索数据流

1. UI 将查询文本和递增 `query_id` 发送给 Query Coordinator。
2. Coordinator 取消旧查询，并行调用内置提供器；插件只返回静态命令入口，不在每个按键上启动 WASM。
3. 提供器流式返回归一化候选，Ranker 使用文本匹配、类型优先级、最近使用和用户固定项排序。
4. UI 先渲染首批结果，后续结果按稳定键更新，禁止重排当前键盘选择。
5. 用户执行插件命令后，Plugin Manager 才启动 Plugin Host 并创建插件会话。

命令注册项同时携带 `interaction`。`view` 创建官方 View 会话；`one-shot` 只执行一次受限调用并返回 `command-result`。两者进入同一个 Coordinator、权限 broker 和 sibling Host，不建立第二条插件执行管线。

内置提供器设置独立超时和结果上限。慢提供器不能阻塞首批结果；旧 `query_id` 的返回值必须丢弃。

## 7. IPC

- Windows 使用限制当前用户访问的 Named Pipe；macOS 使用权限为 `0600` 的 Unix Domain Socket
- 主进程生成一次性启动令牌，通过继承句柄或受控参数交付 Plugin Host
- 双方握手校验协议主版本、进程身份、随机数和会话令牌
- 每条消息包含 `request_id`、`deadline`、`plugin_id` 和可取消标识
- 消息采用 32 位长度前缀 Protobuf，默认单包上限 1 MiB
- 图片和大资源使用宿主管理的资源句柄，禁止反复在 IPC 中复制原始字节
- Host 断开时所有未完成请求以确定性错误结束，不无限重试

## 8. 并发与资源模型

- Slint 事件循环只处理渲染和轻量状态变更
- 文件、数据库与插件调用在有界 worker 线程中执行，通过有界通道返回；未来引入网络或结构化异步 I/O 时再单独评审 Tokio 的启动和内存成本
- 高频查询使用取消令牌，不仅依赖 debounce
- Plugin Host 默认最多并发 4 个活跃插件会话；单插件默认 1 个前台会话
- 每个 Store 配置 64 MiB 线性内存上限；该上限是可访问内存限制，不等同于实际 RSS 预算
- 插件纯计算使用 fuel/epoch interruption 和默认 2 秒硬 deadline；宿主异步 IO 等待不消耗计算 fuel
- Plugin Host IPC 将 Component 加载/编译单独限制为 30 秒，普通 `open/update/close` 响应仍限制为 2 秒；加载预算只覆盖一次性编译，不得放宽插件调用或 UI 线程预算
- HTTP、文件和其他宿主 IO 使用能力级独立 deadline 与取消令牌；达到 IO deadline 后取消宿主操作并返回确定性错误
- Plugin Host 默认只覆盖当前用户动作：`view` 在 `close` 后退出，`one-shot` 在完成、取消或超时后退出，并释放 Wasmtime Engine；这样牺牲重复调用的编译冷启动，换取无插件时不常驻 Wasmtime 内存
- 所有缓存都有条目数或字节预算，不允许无界 `HashMap`、图片缓存或日志队列
- ClipboardVault 的文本上限 1 MiB、图片上限 8 MiB；历史默认最多 500 条且 TTL 为 7 天，用户只能在宿主提供的有限范围内缩短或调整这两个上限。载荷只以 XChaCha20-Poly1305 密文留在内存，密钥通过 `CredentialStore` 获取。
- ClipboardWindow 的固定项由 Vault 标记；固定项不因 TTL/LRU 清理自动移除，未固定项仍优先按数量上限淘汰，避免用户明确保留的内容被静默删除。
- 剪贴板策略更新先通过边界校验，再裁剪已有 LRU 和过期时间并持久化到 SQLite；设置损坏时回退到默认策略，不能扩大进程内存预算。
- 插件 Form 的编辑状态由 Shell 维护 `form_dirty` 标志；失焦或 Escape 不会静默隐藏脏表单，只有视图切换或 Host 响应完成后才清理该状态。
- Windows/macOS 适配器通过 `CredentialStore` 读取 Credential Manager/Keychain；其他平台或凭据读取失败时，剪贴板捕获必须返回明确的 capability unavailable，不得使用固定开发密钥伪装成生产加密。

### 8.1 官方 View 更新模型

- WIT 返回完整、受限的 View 快照，是插件 UI 状态的唯一协议来源。
- 宿主按稳定 ID 计算 `insert/remove/move/update`，只更新改变的 Slint 模型节点；详情变化不得重建未变化的列表行。
- 选择、焦点和滚动锚点绑定稳定 ID。异步结果到达后不能用数组下标恢复交互状态。
- List/Grid 使用窗口化模型，只实例化可见行和小幅 overscan；协议单页仍限制 100 项，宿主搜索候选按 100/1,000/10,000 项分别测量。
- 不发布 `inherits` 或通用 Patch DSL，避免 WIT 快照与增量协议形成两个状态所有者。

### 8.2 有界诊断模型

主进程持有固定容量的结构化事件环形缓冲区，MVP 默认最多 256 条。事件只记录阶段、时间、request ID、插件 ID、Host PID、退出码、耗时、错误分类、权限能力与规范化范围、active/previous pointer、版本、包哈希、签名状态和 `pending_delete`；每个字符串字段均有长度上限。

诊断页读取该缓冲区并生成可预览、可删减的本地诊断包。它不记录查询输入、文件内容、完整 URL 或插件表单值，不上传遥测，不启动额外进程，也不增加 watcher。缓冲区和本地滚动日志只是观测数据源，不拥有插件生命周期或权限裁决。

### 8.3 资源测量与启动关键路径

- 性能报告同时记录进程 RSS、私有工作集/Private Bytes 或平台等价指标、完整 NovaHub 进程树总量，以及平台可获取的 GPU/纹理内存；不同指标不能混为一个“内存占用”数字
- 固定参考机至少覆盖：无插件闲置、首次唤起、一个插件会话、四个并发会话、Host 会话退出后、宠物关闭/静态/动画七种场景
- 单独记录 Plugin Host 基线、首个插件增量和后续会话增量；发布门槛不能只检查无 Host 主进程的 70 MiB 指标
- 首帧前只加载窗口、主题、快捷键和最小命令快照。SQLite 扫描、文件预览和图片解码必须退出首帧关键路径；低层 `nucleo-matcher` 只保留一个可复用 scratch，不构建常驻索引或线程池
- 应用发现采用宿主内存中的有界惰性快照：首次需要应用结果时从平台索引加载最多 128 条，后续按键只做内存过滤；应用安装/卸载或平台变更信号到达后显式使快照失效并低频刷新，禁止每次按键重新枚举开始菜单或 `/Applications`
- 索引未就绪时返回可预测的 loading/partial 状态；索引按版本快照后台恢复并增量更新，禁止每次按键重建
- Slint 渲染后端、字体策略和关键资源格式在 Windows/macOS Phase 0 探针后分别固定；升级渲染后端必须重跑首帧、帧时间、RSS 和 GPU 基线

## 9. 平台适配边界

统一接口覆盖全局快捷键、窗口定位、开机启动、应用发现、文件搜索、通知、剪贴板、凭据存储和系统命令。平台差异由 capability 探测表达，不伪造最低公分母：

- Windows：Win32/WinRT、Windows Search、Credential Manager、MSIX 或签名安装器
- macOS：AppKit/LaunchServices、Spotlight、Keychain、Notarization 和 Hardened Runtime
- 不支持的命令从搜索源中移除，而非在执行时弹出通用失败

### 9.1 插件 Capability 适配器

插件平台能力沿唯一调用链执行：

```text
WIT import
→ sibling Plugin Host（类型转换，不持有授权）
→ 认证 IPC
→ novahub-app Capability Broker（每次调用重新裁决）
→ host-owned adapter
→ OS / SQLite
```

Broker 是授权的唯一所有者，adapter 只执行已经规范化的操作，但仍负责输入、资源和时限上限。Plugin Host
不得打开主 SQLite、解析真实文件路径或创建网络客户端。adapter 使用小型同步 request/response 契约，放入
现有有界 worker 执行；MVP 不为平台能力引入 Tokio runtime、常驻线程池、后台服务或第二套 IPC/ABI。

MVP 最小成功路径固定为：

- HTTP 只支持 HTTPS `GET`，禁止用户信息、裸 IP、本地网段和自动注入凭据；最多跟随 5 次重定向，
  每一跳都把完整目标 URL 重新交给 Broker。默认连接时限 3 秒、单次读取时限 5 秒、总时限 10 秒，
  UTF-8 响应体上限 512 KiB；超限、非文本响应和取消都返回稳定错误码。
- Files 由系统文件选择器签发不可猜测 token；App 保存 `token → plugin/session/handle` 映射。只允许读取
  UTF-8 文本且单次上限 512 KiB；关闭会话、Host 崩溃、撤销授权、禁用或卸载时立即失效。路径和 OS handle
  不进入 WIT、Host IPC 响应、日志或诊断包。
- Storage 的 typed KV 由 App-owned SQLite 实现，Plugin Host 不持有数据库路径；key、value、命名空间和
  总配额在 broker 与事务内双重校验。
- Clipboard 只通过平台 adapter 读写有界文本；读、写分别授权，图片和历史集合不暴露给插件。

以上数字是默认上限，只能在参考机和安全评审后收紧或显式调整。选择 HTTP 实现库时必须比较二进制增量、
冷启动、完整进程树 RSS、后台线程和 TLS 后端；不得仅以 API 易用性作为选型依据。

## 10. 关键故障策略

| 故障 | 系统行为 |
|---|---|
| 插件陷阱或超时 | 销毁对应 Store，显示可恢复错误，主程序继续 |
| Plugin Host 崩溃 | 标记会话失败，按用户重试重新启动，不进行无限自动重启 |
| 数据库迁移失败 | 事务回滚，保留旧文件并进入受限恢复模式 |
| 系统索引不可用 | 告知用户并停用文件结果，不做全盘扫描 |
| UI 渲染异常数据 | 协议校验拒绝超限 View，回退到标准错误视图 |
| 更新启动失败 | 更新器恢复上一个已验证版本 |

## 11. 架构演进门槛

- TypeScript SDK 只能在 WIT 契约稳定并有跨语言一致性测试后引入
- 独立每插件进程只在共享 Host 隔离不足有真实证据时引入
- Slint 无障碍不足时先补 `ui-slint` 语义桥或局部原生控件；核心流程仍无法通过 Narrator/VoiceOver 时退回架构评审，不以 Tauri 作为局部 fallback
- WebView 只在真实高价值复杂编辑场景达到量化触发器，且独立、按需、可回收 Host 的完整进程树资源预算通过后评审；不得替换核心 Slint Shell、进入主进程或改变 WIT View 所有权
- Host 复用只有在参考机 P95 首次/重复 Component 调用都超过用户目标，且完整进程树内存增量可量化控制时才评审；复用不得变成主进程依赖或解除会话、资源和崩溃回收边界
- 自建文件索引只在系统索引无法满足已量化需求时评审
- 云服务不得成为本地主入口、插件执行或设置读取的可用性依赖

## 12. MVP 性能切片 64：渲染、进程与 Wasmtime 缓存

Windows Release 的默认 Slint 后端固定为 `winit/software`，通过 `SLINT_BACKEND` 保留显式探针和回归覆盖入口。原因是当前参考机上 software 后端不分配 GPU 纹理内存，四 Host 场景的完整进程树 RSS 为 108.97 MiB；显式 FemtoVG 对照约为 109 MiB RSS，并额外产生约 71 MiB GPU Shared。该选择只影响渲染后端，不改变 Slint Shell、WIT ABI 或宿主所有权。升级后端必须重新测首帧、帧时间、RSS、Private Bytes、Private Working Set 与 GPU。

Release App 与 sibling Plugin Host 使用 Windows subsystem，避免 `conhost.exe` 进入进程树。应用发现从 `window.show()` 前移路径移除，改为事件循环启动后的惰性刷新；首帧观察使用 backend-neutral 回调：GPU 后端走 `AfterRendering`，software 后端走首次 Winit `RedrawRequested` 后的有界回调。

资源报告同时记录 RSS、Private Bytes、Private Working Set、GPU Dedicated/Shared Memory 和按 GPU engine 类型的最大利用率。GPU engine 利用率不跨引擎相加；software 后端没有 GPU 分配时记录为不可用，而不是填充为零后混入结论。Plugin Host 仍按用户动作 sibling、默认最多四个活跃会话，退出后释放 Wasmtime Engine。

Wasmtime 仅启用官方 `cache` feature。缓存目录由 App/Host 明确传递，默认位于用户数据目录 `cache/wasmtime`，并限制为最多 256 个文件、128 MiB；不使用自定义预编译格式或 `unsafe deserialize`。缓存降低重复 Component 编译冷启动，但不改变 Host 的生命周期边界或主进程依赖边界。

最终 Windows Release 报告位于 `target/reference-benchmark/windows-x86_64-release.json`，参数为 20 samples、3 warmup、15 秒资源保持，实际策略为 `winit-software`。冷启动 Shell 首帧 P50/P95 为 919.98/1383.12 ms；单 Host 为 96.27/111.35 ms；四 Host 聚合为 153.43/250.90 ms。Shell 数据是冷启动首帧，不等同于热快捷键到首帧 ≤100 ms 的目标；四 Host P95 是全部会话 ready 的聚合时间，也不等同于单 Host 500 ms 门槛。

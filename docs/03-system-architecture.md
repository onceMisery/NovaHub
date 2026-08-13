# NovaHub 系统架构

## 1. 架构目标

NovaHub 的架构优先级依次是：宿主稳定、交互低延迟、空闲低内存、插件隔离、跨平台一致、可演进性。任何为了插件自由度而让第三方代码进入 UI 进程、绕过权限网关或复制协议所有权的方案都不符合基线。

## 2. 技术选型

| 领域 | 选择 | 原因 |
|---|---|---|
| 语言 | Rust stable | 内存安全、原生性能、跨平台生态与 WASM 工具链统一 |
| 桌面 UI | Slint | 无浏览器运行时、声明式数据绑定、适合聚焦型桌面工具 |
| 异步运行时 | Tokio | 网络、IPC、取消与结构化并发生态成熟 |
| 插件运行时 | Wasmtime Component Model | WIT 强类型契约、资源限制、WASI 能力模型 |
| 插件协议 | WIT | 语言中立且可生成绑定，是唯一 ABI 权威 |
| 搜索匹配 | `nucleo` | 高性能模糊匹配和增量结果能力 |
| 持久化 | SQLite + `rusqlite` | 单机事务、迁移、备份和诊断成本低 |
| IPC 编码 | Protobuf | 长度前缀、强类型、向后兼容字段演进 |
| 观测 | `tracing` + 本地滚动日志 | 结构化诊断，发布构建可关闭敏感字段 |
| 系统 API | `windows`、`objc2`/AppKit | 避免跨平台抽象泄漏关键桌面行为 |

## 3. 进程拓扑

```text
novahub-app（始终运行）
├── Slint Shell / Official View Renderer
├── Query Coordinator + Ranker
├── Built-in Providers
├── Platform Services
├── Plugin Manager / Capability Broker
└── SQLite / Credential Store
        │ 本地认证 IPC
        ▼
novahub-plugin-host（首次调用插件时启动，空闲回收）
├── Wasmtime Engine / Component Linker
├── 每插件独立 Store、Limiter、Session
├── WIT Host API Adapter
└── IPC Client
        │
        ▼
*.wasm Component
```

主进程不能链接或执行第三方原生动态库。Plugin Host 不能直接打开主数据库、系统凭据库或任意用户文件；它只能发起带插件身份的 capability 请求。

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
│   └── platform/
│       ├── windows/
│       └── macos/
├── sdk/rust/
├── wit/novahub-plugin/
├── plugins/examples/
├── docs/
└── xtask/
```

这是一份实施目标结构，不要求在设计阶段创建空目录。

## 5. 模块所有权与依赖方向

| 模块 | 唯一所有权 | 允许依赖 |
|---|---|---|
| `core-domain` | 命令、查询、动作、结果等稳定领域类型 | 标准库和小型通用库 |
| `ui-protocol` | 官方 View、Event 和资源句柄 | `core-domain` |
| `search` | 候选归一、匹配、排序、学习信号 | `core-domain` |
| `plugin-manager` | 安装、版本、启停、权限和生命周期 | 领域、存储、IPC |
| `plugin-runtime` | Wasmtime 执行与资源限制 | WIT 生成绑定、IPC |
| `storage` | Schema、迁移、事务和插件命名空间 | 领域类型 |
| `platform/*` | OS 快捷键、窗口、应用、文件搜索、凭据 | 领域接口 |
| `ui-slint` | 宿主渲染、主题、焦点和无障碍映射 | UI 协议、应用服务 |

依赖必须指向稳定契约。平台实现、Slint 组件和 Wasmtime 细节不能进入 `core-domain`；Rust SDK 不得反向成为 WIT 的协议来源。

## 6. 主搜索数据流

1. UI 将查询文本和递增 `query_id` 发送给 Query Coordinator。
2. Coordinator 取消旧查询，并行调用内置提供器；插件只返回静态命令入口，不在每个按键上启动 WASM。
3. 提供器流式返回归一化候选，Ranker 使用文本匹配、类型优先级、最近使用和用户固定项排序。
4. UI 先渲染首批结果，后续结果按稳定键更新，禁止重排当前键盘选择。
5. 用户执行插件命令后，Plugin Manager 才启动 Plugin Host 并创建插件会话。

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
- 文件、网络、数据库与插件调用在 Tokio 任务中执行，通过有界通道返回
- 高频查询使用取消令牌，不仅依赖 debounce
- Plugin Host 默认最多并发 4 个活跃插件会话；单插件默认 1 个前台会话
- 每个 Store 配置 64 MiB 线性内存、fuel/epoch interruption 和 2 秒硬超时
- Plugin Host 无活跃会话后 60 秒退出；该值可通过性能测试调整
- 所有缓存都有条目数或字节预算，不允许无界 `HashMap`、图片缓存或日志队列

## 9. 平台适配边界

统一接口覆盖全局快捷键、窗口定位、开机启动、应用发现、文件搜索、通知、剪贴板、凭据存储和系统命令。平台差异由 capability 探测表达，不伪造最低公分母：

- Windows：Win32/WinRT、Windows Search、Credential Manager、MSIX 或签名安装器
- macOS：AppKit/LaunchServices、Spotlight、Keychain、Notarization 和 Hardened Runtime
- 不支持的命令从搜索源中移除，而非在执行时弹出通用失败

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
- WebView 插件只在官方组件无法覆盖明确高价值场景，且能接受独立进程资源预算时评审
- 自建文件索引只在系统索引无法满足已量化需求时评审
- 云服务不得成为本地主入口、插件执行或设置读取的可用性依赖

# NovaHub 工程质量、发布与运维

## 1. 质量分层

### 单元测试

- 命令、查询归一、低层 `nucleo-matcher` 排序和学习信号
- 计算器解析、数值边界和单位换算
- 清单解析、版本范围、权限计算和卸载计划
- `DeclaredPermission` 规范化、`EffectiveGrant` 交集和权限 diff 五种分类
- SQLite schema、迁移、配额和加密载荷
- WIT View 校验、协议错误与资源限制

### 契约测试

- WIT 生成绑定与 Rust SDK 语义一致
- Protobuf 消息的主版本兼容、未知字段和取消
- View 快照跨 Windows/macOS 一致
- Host capability 对拒绝、超时和句柄失效的行为一致
- 同一权限 fixtures 在 parser、preview、grant store、broker、SDK 和 UI 中得到一致结果；未知能力/字段默认拒绝
- 真实 Component 覆盖 Clipboard、typed KV、有界 HTTP `GET` 和用户选择文件的成功路径；所有调用均经过 sibling Host、认证 IPC 与 App-owned broker
- HTTP 覆盖逐跳 Origin 裁决、最多 5 次重定向、连接/读取/总时限、512 KiB 响应上限和非 UTF-8 拒绝；Files 覆盖 token 伪造、跨插件/会话错绑及关闭、崩溃、撤销后的失效
- `view` 与 `one-shot` 的 WIT 导出、结果上限、取消、超时和回收语义一致

### 集成测试

- `novahub-app` 与 Plugin Host 的真实 IPC
- 安装 → 启用 → 打开 → 更新 → 禁用 → 卸载全生命周期
- `new -> check -> test -> pack -> sign -> install -> real Host round-trip` 同时覆盖最小 `view` 和 `one-shot` 模板
- Host 崩溃、插件超时、数据库迁移失败后的恢复
- Windows 应用发现/快捷键/文件搜索和 macOS 对等能力

### UI 与可访问性测试

- Slint 组件状态快照与键盘导航
- 100%/150%/200% DPI 或文本缩放
- 深浅色、高对比度、减少动画
- Windows Narrator、macOS VoiceOver 核心流程
- 权限新增/扩大/收窄/移除/未变化、信任摘要和诊断恢复路径不依赖颜色表达
- 多显示器、不同缩放、全屏应用和虚拟桌面

### 性能与压力测试

- 启动、热唤起、首帧、首批结果和首个插件调用 P50/P95/P99
- 1/100/1000/10000 个命令索引和长文本查询
- 4 个并发插件会话、资源限制、Host 会话结束回收和反复冷启动
- 剪贴板默认 500 条、文本/图片大小预算、AEAD 解密、可配置 TTL/LRU 淘汰和数据库 VACUUM 策略
- 无插件闲置、一个插件、四个插件和 Host 回收后的主进程/Host/完整进程树 RSS 与私有工作集；平台可获取时同时记录 GPU/纹理内存
- HTTP adapter 选型同时记录二进制增量、冷启动、完整进程树 RSS、后台线程与 TLS 后端；不得引入常驻网络 runtime
- Slint 渲染后端或版本升级前后使用相同参考机、主题、DPI、字体和数据集对比，不接受只测单进程或只测安装包大小
- stable-ID 的 insert/remove/move/update 在 100/1,000/10,000 项下保持选择与滚动锚点；详情更新不整体重建列表行
- 虚拟列表只实例化可见行和限定 overscan，动态 loading、附件和进度不造成布局跳动

## 2. CI 基线

每个变更至少运行：

```text
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p contract-tests
cargo deny check
cargo audit
cargo run -p xtask -- capability-e2e
cargo run -p xtask -- release-check
cargo run -p xtask -- resource-report --pid <novahub-root-pid>
```

`capability-e2e` 构建真实 `wasm32-wasip2` Component 和 sibling Plugin Host，并验证授权、撤销、typed KV、
session 失效、opaque token、有效文件读取和逐跳 HTTP 完整往返 App broker。HTTP 响应与文件选择在该测试中
由 App 边界注入确定性结果，因此 Windows/macOS 真实 TLS 与系统 picker smoke 仍是发布前证据。
`release-check` 负责静态发布边界、插件示例、WIT、已退休评审文档、该 E2E 的 CI 接线和主进程依赖检查；Windows/macOS 实机平台能力、完整进程树 RSS/Private Working Set、GPU/纹理内存和签名安装器 smoke 必须通过参考机 CI 注入 `NOVAHUB_RELEASE_PLATFORM_EVIDENCE` 与 `NOVAHUB_RELEASE_RESOURCE_EVIDENCE` 后，再使用 `--strict-platform` 执行。

`resource-report` 会按根 PID 递归收集 NovaHub 子进程；Windows 输出 RSS 与 Private Bytes，macOS/POSIX 输出 RSS 并将私有工作集标记为不可用。参考机应在 Shell、Plugin Host 和宠物的目标场景中保存 JSON 原始报告，不把单个子进程数字替代完整进程树总量。

跨平台矩阵：Windows x86_64、macOS arm64、macOS x86_64。仓库 CI 对三者运行格式、Clippy、串行 workspace 测试和静态发布检查，并在标签构建时上传未签名 release artifact；签名、Notarization 和安装器 smoke 只在受保护的发布环境执行。WASM 插件构建使用固定 Rust 工具链和 `wasm32-wasip2` 目标。`rust-toolchain.toml`、`Cargo.lock` 和工具清单固定 Rust、Wasmtime、`wasm-tools`、WIT package 与绑定生成器精确版本；升级必须通过 SDK/WIT/Wasmtime 版本矩阵。UI 截图测试在固定系统字体与缩放配置下运行，允许人工审核基线变更。

## 3. 发布轨道

- `nightly`：内部自动构建，仅用于宿主与 SDK 回归
- `beta`：签名构建，允许开发者模式和诊断开关
- `stable`：经过跨平台验收、性能预算和安全审查的公开构建

版本使用 SemVer，但插件 API 主版本与应用版本独立。宿主可发布小版本而保持同一插件 API；API 破坏性变更必须进入新的主版本和迁移窗口。

## 4. 安装器与更新器

Windows 选择 MSIX 或签名安装器时以启动速度、文件关联和企业部署需求实测决定；macOS 使用签名、Notarization、DMG/ZIP 中的一种稳定分发方式。更新器独立于 UI 运行，可在重启前完成 staging 和签名校验。

发布依赖树必须保证 `novahub-app` 不链接 Wasmtime 编译器或插件 SDK，Wasmtime 及其最小必要 feature 只进入按需 Plugin Host。CLI 与更新器均按需运行，不作为常驻辅助进程规避主程序预算。发布构建剥离非必要调试符号；安装包压缩率、二进制体积和运行时工作集分别报告，不能互相替代。

更新事务：下载 → 校验 → staging → 健康检查 → 原子激活 → 旧版本保留 → 下次成功启动后清理。失败永远保留可回滚路径。

## 5. 诊断与支持

- 默认只写本地结构化日志，按组件和请求 ID 关联
- 日志字段拥有敏感等级：公开诊断、设备本地、禁止记录
- 性能日志记录耗时、队列长度和内存预算使用，不记录用户内容
- 用户导出诊断包前可查看清单并去除路径、插件参数和网络信息
- 崩溃恢复只尝试有限次数，避免 Host 或更新器重启风暴
- 插件诊断事件使用默认 256 条的有界环形缓冲区并限制字段长度；容量溢出只淘汰最旧事件，不增加后台进程或无界队列
- 测试诊断包预览/删减、敏感字段过滤和禁录规则，确保不包含查询输入、文件内容、完整 URL 或插件表单值

## 6. 发布门槛

发布前必须有：

- 两个平台签名和安装/卸载测试证据
- 关键工作流无阻塞级 bug
- 资源预算和 P95 指标报告
- 一个与四个插件会话的完整进程树资源上限已经在 M1 前冻结并通过，不只满足无 Host 主进程指标
- 插件 SDK 示例通过真实 Host 端到端测试
- 最小 `view`/`one-shot` 模板、golden contract fixtures 和 SDK/WIT/Wasmtime 精确版本矩阵通过
- 权限 contract matrix、stable-ID 差量/虚拟列表和诊断容量/隐私测试通过
- 安全 fuzz、依赖审计和权限回归结果
- 已知限制、数据删除边界和回滚方案

## 7. 版本兼容与迁移

用户数据迁移必须向前兼容一个稳定版本，并在失败时保留原始数据。插件安装器、协议、数据库和设置各自拥有版本号；不要用一个全局版本号掩盖不同契约的兼容状态。

废弃项必须记录替代方案、观察期和删除触发条件。若没有消费者证据或稳定 API 使用量，不为未来兼容保留空分支。

## 8. 运维指标

即使 MVP 没有云端，也应在本地和发布测试中追踪：启动耗时、首帧、搜索延迟、Host 冷启动、崩溃率、更新回滚率、插件安装失败原因、卸载残留任务数量。未来若用户主动同意匿名遥测，只上传聚合指标，不上传内容。

## 9. Slice 64：Windows Release 资源证据

本轮固定的 Windows Release 证据来自 `target/reference-benchmark/windows-x86_64-release.json`：20 个 samples、3 次 warmup、资源保持 15 秒，实际渲染策略为 `winit-software`。报告同时保存 stderr、进程数、RSS、Private Bytes、Private Working Set、GPU Dedicated/Shared Memory，以及按 GPU engine 类型记录的最大利用率。

| 场景 | P50 | P95 | RSS | Private Bytes | Private Working Set | 进程数 |
|---|---:|---:|---:|---:|---:|---:|
| Shell 冷启动首帧 | 919.98 ms | 1383.12 ms | 43.39 MiB | 14.94 MiB | 11.91 MiB | 1 |
| 1 个 Plugin Host | 96.27 ms | 111.35 ms | 44.41 MiB | 16.92 MiB | 10.42 MiB | 2 |
| 4 个 Plugin Host | 153.43 ms | 250.90 ms | 108.97 MiB | 43.53 MiB | 26.26 MiB | 5 |

四 Host 的 P95 是所有会话 ready 的聚合时间，不能解读为单 Host 500 ms 门槛；Shell 结果是冷启动首帧，不能解读为热唤起 ≤100 ms 已达标。FemtoVG 对照约 109 MiB RSS、71 MiB GPU Shared 和 0.62% 3D engine 利用率，因此当前 software 默认仍符合低内存优先级。

Wasmtime 官方缓存目录为用户数据目录下的 `cache/wasmtime`，上限 256 个文件、128 MiB。本轮实测缓存为 2 个文件、约 121 KiB；单 Host 专项 P95 从约 649~1021 ms 降至约 267 ms。缓存只减少重复编译，不引入常驻 Host、第二套 ABI 或不安全反序列化。

发布前仍需补齐 macOS renderer/RSS/Private Working Set/GPU/首帧、无障碍、Host 崩溃回收、签名安装器/Notarization 和真实 picker/TLS smoke。上述缺口属于平台发布证据，不触发 Tauri/WebView fallback。

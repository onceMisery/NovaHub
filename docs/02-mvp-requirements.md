# NovaHub MVP 产品需求

## 1. MVP 目标

MVP 必须同时证明两件事：NovaHub 本身是可日常使用的桌面工具；Rust/WASM 插件可以被安全安装、调用和卸载。仅有插件平台内核不构成可用 MVP。

## 2. 核心工作流

### 唤起与搜索

1. 用户配置全局快捷键，默认建议 Windows 为 `Alt+Space`、macOS 为 `Option+Space`，冲突时引导重新绑定。
2. 快捷键唤起后焦点直接进入搜索框，窗口出现在当前活动显示器中央。
3. 输入即时返回应用、命令、计算结果、历史和插件入口。
4. 上下键移动选择，Enter 执行主动作，Escape 返回上一级或关闭窗口。
5. 失去焦点时默认隐藏；正在确认破坏性操作或编辑表单时不得静默丢失状态。

### 应用启动

- Windows 扫描开始菜单快捷方式和已注册应用
- macOS 使用 LaunchServices/NSWorkspace 发现并打开 `.app`
- 支持应用名、别名、拼音首字母和使用频率排序
- 应用清单变化时增量刷新，不进行高频全盘扫描

### 计算器

- 支持基础算术、括号、百分比、常见数学函数和单位换算
- 计算在本地执行，不使用 `eval` 或脚本运行时
- 结果可复制；解析失败不得执行猜测性表达式

### 剪贴板历史

- 支持文本和受限尺寸图片
- 默认保留 7 天、最多 500 条，可在设置中缩短或清空
- 内容使用宿主生成的本地密钥加密，密钥保存在系统凭据存储
- 支持暂停记录和一键清空
- MVP 不同步、不上传、不向插件开放历史集合

### 基础文件搜索

- Windows 优先使用 Windows Search，macOS 优先使用 Spotlight
- 仅在用户输入时查询系统索引，不维护 NovaHub 全盘常驻索引
- 支持打开文件、打开所在目录和复制路径
- Shell 结果动作分别由宿主 `files.open`、`files.reveal` 和 `files.copy_path` 执行；插件不能直接获得文件管理器或剪贴板句柄
- 系统索引不可用时明确降级，不静默执行昂贵扫描

### 系统命令

- 支持打开设置、锁屏、睡眠、注销等明确命令
- 关机、重启和清理类操作必须二次确认
- 平台不支持的命令不展示，不返回无法执行的占位结果

### 插件管理

- 从本地 `.novahub-plugin` 包安装
- 安装前展示发布者、版本、命令、权限精确范围和本地已验证的信任事实
- 更新预览区分权限的新增、扩大、收窄、移除和未变化；新增或扩大权限被拒绝时保留旧 active pointer
- 支持启用、禁用、原子更新与卸载
- 卸载默认删除插件拥有的本地数据
- 开发者模式允许加载未签名本地插件，并持续显示开发状态
- 提供宿主原生诊断页，按请求 ID 展示安装、加载、调用、回收、回滚和待删除状态，不记录用户内容

### 桌面宠物

- 支持一个活动桌面宠物在桌面边缘显示、隐藏、拖动、吸附并记忆多显示器位置
- 宠物使用 `kind = "desktop-pet"` 的声明式 `.novahub-plugin` 包，用户可安装和制作自定义宠物
- 显示/隐藏通过右键菜单、可配置的图标化边缘手柄、全局快捷键或系统托盘完成，不在桌面永久展示操作文案
- 单击宠物打开宿主管理的 Pet Action Shelf，用户可固定最多 5 个 NovaHub 命令、Quicklink、Snippet 或插件命令
- 宠物插件不能创建窗口、常驻运行、监听全局输入、读取屏幕内容或继承快捷动作的权限
- MVP 内置 Waterman 水人、Nova 星灵和 Pixel 小方；当前宠物不可用时回退 Nova 星灵
- 宿主通过 `pet list` 展示可用宠物，通过 `pet use <id>` 激活内置或已安装的声明式宠物；自定义资源只允许经过校验的 SVG 帧进入 Slint

## 3. MVP 官方视图

- ListView：搜索结果、操作列表、状态文本、图标资源、badge、快捷键提示和游标分页
- GridView：图标或媒体型结果，以及与 ListView 一致的受限附件语义
- DetailView：安全 Markdown、text/link/tag 元数据和资源句柄预览
- FormView：text、password、select、checkbox、switch，以及初始值、helper 和校验错误
- ViewState：loading、确定进度、empty 和可恢复错误
- ActionPanel：默认动作、次要动作和破坏性确认
- Toast：成功、警告和可恢复错误
- Navigation：宿主管理的层级导航

插件命令必须在清单中声明 `interaction = "view" | "one-shot"`；缺省为 `view`。`one-shot` 返回有界标准结果，不创建完整 View，不获得后台执行或自行创建窗口的能力。

所有交互节点必须有会话内稳定 ID。插件继续返回完整 View 快照，宿主只在内部按稳定 ID 计算增删、移动和字段更新；列表只实例化可见行和小幅 overscan，不发布第二套 Patch DSL。

## 4. Kunkun 借鉴能力的 MVP 边界

| 能力 | MVP 决策 | 交付要求 |
|---|---|---|
| 强类型权限 | 必须 | 区分 `DeclaredPermission` 与 `EffectiveGrant`，未知能力/字段默认拒绝 |
| 安装与更新权限 diff | 必须 | 展示新增、扩大、收窄、移除、未变化及拒绝影响 |
| Capability 运行时闭环 | 必须 | 每次调用都经过 App-owned broker；覆盖授权持久化、撤销、卸载清理、Host IPC 和真实 Component E2E |
| `view` / `one-shot` | 必须 | 共用 WIT、权限、IPC、fuel/epoch、deadline 和 Host 回收边界 |
| 官方 View 最小完整语义 | 必须 | 附件、typed metadata、完整 Form 状态、进度、动作和分页先进入 WIT |
| stable-ID 差量与虚拟列表 | 必须 | 选择和滚动锚点不因局部更新丢失；覆盖 100/1,000/10,000 项测量 |
| 原生诊断面 | 必须 | 有界本地事件缓冲、隐私删减和诊断包预览，不新增常驻进程 |
| SDK/CLI 闭环 | 必须 | `view`/`one-shot` 模板、golden fixtures 和真实 Host E2E |
| `plugin dev --watch` | MVP 后 | 仅显式开发模式，不进入生产常驻进程树 |
| 多语言 SDK | MVP 后 | 复用同一 WIT 语义并通过跨语言一致性测试 |
| 扩展 provenance | MVP 后 | source、commit、builder、workflow、SBOM、transparency log 均须可验证 |

MVP 的信任摘要只展示宿主已验证或可客观判断的 publisher key ID、归档 SHA-256、签名状态、插件 API 兼容性和开发者模式状态。包内自报的来源或构建字段不得直接显示为“已验证”。

Capability 闭环必须包含最小可用的成功路径，不能只证明拒绝逻辑：Storage 提供宿主拥有的 typed KV；
Clipboard 覆盖独立的文本读写授权；HTTP 仅提供有界 HTTPS `GET`；Files 仅读取用户通过系统选择器选中的
UTF-8 文本文件。HTTP 和 Files 的具体预算由架构文档统一定义，插件始终只看到 WIT 类型、稳定错误码和
opaque token，不看到数据库路径、文件路径、OS handle 或网络客户端。Notification 与 Logging 可以在 MVP
返回明确的 `unsupported`，但不得绕过 broker 或伪造成功。

## 5. 性能验收预算

以下数字是工程目标，必须在约定参考机器上实测，不能仅由框架推断：

| 指标 | MVP 目标 |
|---|---:|
| 无插件、无更新任务时完整 NovaHub 进程树闲置 RSS P95 | ≤ 70 MiB，且不得以常驻辅助进程规避统计 |
| 热状态快捷键到首帧 P95 | ≤ 100 ms |
| 输入到首批本地结果 P95 | ≤ 50 ms |
| 搜索期间 UI 主线程单帧工作 P95 | ≤ 16.7 ms |
| Plugin Host 冷启动到可调用 P95 | ≤ 500 ms |
| 单插件默认 WASM 线性内存上限 | 64 MiB |
| 单次插件纯计算硬超时 | 2 s |
| 一个插件会话的完整进程树 RSS/私有工作集 | Phase 0 实测并在 M1 前冻结发布上限 |
| 四个并发插件会话的完整进程树 RSS/私有工作集 | Phase 0 实测并在 M1 前冻结发布上限 |
| GPU/纹理内存 | Phase 0 记录基线，渲染后端升级不得无审查回退 |
| 启用宠物相对关闭状态的增量 RSS P95 | ≤ 20 MiB |
| 宠物闲置 CPU P95 | < 单逻辑核心 1% |
| Pet Action Shelf 交互反馈 | ≤ 100 ms |

内存报告必须同时列出主进程、Plugin Host、其他 NovaHub 子进程和完整进程树总量；64 MiB WASM 线性内存是单 Store 上限，不代表常驻 RSS。Windows 与 macOS 使用各自稳定、可复现的系统指标，并在报告中说明共享页和 GPU 内存口径。

代码层通过有界应用快照、首帧后加载和无 WebView 主 Shell 固定路径来保护这些预算；具体 P95 数值仍必须在参考机上实测，不能由静态检查替代。

## 6. 稳定性验收

- 插件陷阱、超时或超限不会导致主程序退出
- Plugin Host 崩溃后主程序仍可使用内置能力并能重启宿主
- 活跃插件卸载后立即退出其视图并撤销能力
- 数据库迁移失败时保持旧版本可读或安全回滚
- Windows 与 macOS 使用同一官方 View 契约
- `view` 和 `one-shot` 在完成、取消、超时或崩溃后都能确定性结束并回收 Host
- 权限解析、预览、授权存储、broker、SDK 和 UI 对未知字段与范围变化执行同一默认拒绝规则
- 无障碍名称、焦点顺序、文本缩放和减少动画模式通过平台测试
- 宠物资源、动作或浮层失败不会影响主窗口；关闭边缘手柄和快捷键后仍可从系统托盘或设置页恢复

## 7. MVP 交付物

- Windows 与 macOS 可签名安装包
- NovaHub 主程序、Plugin Host 和更新辅助能力
- Rust 插件 SDK 与 `novahub` CLI
- 至少两个可复制模板：最小 `view` 插件、最小 `one-shot` 插件；带表单和网络权限的示例可在 `view` 模板上扩展
- manifest、WIT、生成绑定、权限裁决与 Host 行为共用的 golden contract fixtures
- `new -> check -> test -> pack -> sign -> install -> real Host round-trip` 端到端测试，以及 SDK/WIT/Wasmtime 精确版本兼容矩阵
- 三个内置桌面宠物，以及一个可供用户复制修改的声明式宠物示例包
- 插件开发、打包、权限、调试和版本兼容文档
- 性能基线报告、安全威胁模型和发布检查表

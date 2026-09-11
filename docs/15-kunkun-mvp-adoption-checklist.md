# Kunkun 借鉴落地与 NovaHub MVP 清单

本文把 `docs/14-kunkun-reference-analysis.md` 中的借鉴结论转换为可执行的 MVP 清单。它不改变
NovaHub 的架构权威；Rust + Slint 主 Shell、WIT 唯一插件 ABI，以及按需 sibling Plugin Host
仍分别由 `docs/03-system-architecture.md` 和 `docs/04-plugin-platform.md` 定义。

## 1. 已吸收的设计思想

| Kunkun 的可借鉴思想 | NovaHub 的落地方式 | 当前状态 |
| --- | --- | --- |
| 按交互形态区分命令 | manifest 使用 `view` / `one-shot`；两者共用 WIT、权限、资源和 IPC 约束 | 已实现 |
| 权限生成能力面 | `DeclaredPermission`、三方交集 `EffectiveGrant`、`CapabilityBroker` 和权限 diff | 授权、撤销、卸载清理、typed KV、HTTP/Files 受限路径与真实 Component E2E 已实现；真实平台 smoke 待参考机 |
| 宿主拥有 UI 语义 | 插件只返回受限的 List/Grid/Detail/Form/ActionPanel 及 Loading/Progress/Error 状态 | 基础 WIT/View、typed metadata、完整 Form 控件、有界 accessory、cursor pagination 和 ActionPanel 默认动作已实现 |
| 稳定 ID 与局部更新 | `StableListOp`、差量计算和选择状态保留；不发布第二套插件 Patch ABI | 宿主差量模型、真实 Slint 固定槽位分页和 100/1,000/10,000 项必要测试已实现；参考机帧时间与内存证据仍是 MVP P1 |
| 开发脚手架和验证命令 | `plugin new/dev/check/test/pack/sign/install/update/rollback`，并提供 view/one-shot 双模板 | 已实现 |
| 生命周期诊断 | 固定容量诊断环、Host PID、阶段、错误分类和本地摘要 | 有界事件环与原生摘要已实现；预览、删减和导出仍是 MVP P1 |
| 安装来源解释 | 安装预览显示 publisher key ID、归档哈希、签名状态、API 兼容性和权限 diff | 已实现；provenance 可视化为 Phase 2 |

## 2. MVP 必须实施的项目

### P0：权限运行时闭环

P0 是影响安全边界的 MVP 主线，优先级最高。运行时调用链、授权持久化、撤销、卸载清理、typed
host-owned KV、HTTP/Files 最小 adapter 和真实 Component E2E 已接入；剩余工作是参考机上的真实系统
picker/TLS smoke 与完整发布证据：

1. 在现有 WIT `capabilities` import 上定义版本化、类型化的 Host 请求/响应消息。**已完成。**
2. Plugin Host 通过单向请求把 `plugin_id`、session、request ID 和有界参数交给主进程；主进程用
   `CapabilityBroker` 重新校验身份、`EffectiveGrant`、Origin、文件句柄和 storage quota。**已完成主路径接线，并由 Shell installed-plugin 路径复用 execution context。**
3. Clipboard 文本读写和 host-owned typed KV 走现有平台/SQLite adapter；Plugin Host 不直接持有 OS 或
   数据库资源。**已完成。**
4. HTTP 提供有界 HTTPS `GET`：最多 5 次重定向，每跳重新过 broker，连接/读取/总时限分别为
   3/5/10 秒，响应不超过 512 KiB 且必须是 UTF-8。**代码与 Component E2E 已完成；真实 TLS smoke 是发布前证据。**
5. Files 通过系统文件选择器发行绑定 plugin/session 的 opaque token，只读不超过 512 KiB 的 UTF-8
   文本；close、Host crash、撤销、禁用和卸载后失效。**有效 token 发行/读取、错绑拒绝和显式 session 清理已完成；真实 picker smoke 是发布前证据。**
6. parser → grant → broker → WIT → Host IPC → adapter 共用 golden fixtures。
   **storage success/revocation、typed KV read/write、真实 Host close 后 follow-up 拒绝、opaque token
   错绑拒绝、session 清理以及 broker 级 redirect-per-hop、HTTP/Files 成功路径均已完成。**
7. 插件卸载或进入 pending-delete 时清理/冻结对应 grant，避免同 ID 重装继承旧授权。
   **已完成：卸载前写入 canonical 空 grant；文件删除成功后移除 grant；pending-delete 保持空 grant，重试删除成功后按插件 ID 清理 grant；同 ID 重装不会继承旧授权。**

适配器必须位于宿主平台边界；Plugin Host 不得直接打开主 SQLite、系统凭据库、任意路径或网络
客户端。该设计不会增加常驻进程，也不会把 Wasmtime 链入主 Shell。

### P1：渲染、工具与可观测性完善

- 有界 accessory 已对齐 WIT 1.4、Rust SDK、Plugin Host、App 与 Slint：List/Grid 项可携带 status、badge、
  shortcut 和宿主管理的 icon resource ID；宿主限制长度与控制字符，Slint 只显示轻量资源标记，不读取插件路径。
  typed metadata（text/link/tag）与完整 Form 控件/状态（text/password/select/checkbox/switch、helper/error、
  初始值和 progress）也已完成全链路接线。
- cursor pagination 已对齐 WIT 1.5、Rust SDK、Plugin Host、App 与 Slint：每页最多 100 项，opaque cursor
  最多 256 字符；宿主先消费当前固定槽位快照，仅在页尾发送 `load_more`，下一页替换当前快照而非无限累积。
- ActionPanel 默认/次要动作已对齐当前 WIT 1.6、Rust SDK、Plugin Host、App 与 Slint：每个非空面板恰好
  一个默认动作且最多 6 项，Enter 稳定路由该动作；破坏性插件动作必须经过宿主原生确认，取消或失焦不会执行事件。
- 将 stable-ID 差量接入真实 Slint model，采用固定可见窗口和小幅 overscan，保留 selection、focus
  与 scroll anchor；用 100/1,000/10,000 项测量帧时间和内存。**宿主窗口模型、真实 Slint 固定槽位分页和大列表必要测试已完成；参考机测量待完成。**
- 将诊断摘要扩展为独立原生页面，并提供用户主动触发的预览、删减和导出；默认仍不记录输入、表单值、
  文件内容、完整路径或完整 URL。**有界删减、本地 JSON 导出接口和原生页面交互已完成。**
- 在 Windows/macOS 真机上验收 RSS、Private Working Set、GPU、冷启动、无障碍和 Host 崩溃回收。

P1 仍属于 MVP，不是 Phase 2；它可以排在安全闭环之后，但在 MVP 发布前必须有实现和参考机证据。

## 3. 明确不进入 MVP 的内容

- Tauri、WebView、JavaScript 生产运行时和插件自建 iframe/window。
- 常驻 watcher、后台插件、常驻 Plugin Host、Tokio 服务或第二套插件 ABI。
- 多语言 SDK、`plugin dev --watch`、provenance/SBOM/transparency log 的可视化；这些属于 Phase 2，
  但必须复用 MVP 的 WIT、manifest、权限和签名契约。

## 4. 验收门槛

MVP 只有在以下证据齐全后才可称为“权限闭环完成”：

- capability 请求在真实 sibling Host 中可往返主进程，并且每次调用都重新经过 broker；
- 未授权、超范围、Origin 不匹配、文件句柄错绑、配额超限均有稳定错误码；
- 有界 HTTP `GET` 和有效文件 token 至少各有一个真实 Component 成功往返，且大小、时限、重定向和生命周期上限可复现；
  参考机还需补真实 TLS 与系统 picker smoke，不得把测试注入结果冒充平台覆盖；
- Host 超时、崩溃和关闭后，session、句柄和临时授权均失效；
- `cargo fmt`、严格 Clippy、全 workspace 测试、真实 Host E2E、golden fixtures 和版本矩阵均通过；
- 主进程依赖检查确认没有 Tauri/WebView/Wasmtime，且空闲时不存在 Plugin Host。

当前代码已满足核心契约、基础渲染边界、CLI/SDK、typed metadata、完整 Form View 控件、诊断事件环与
导出接口、宿主 stable-ID 窗口模型、P0 运行时主路径、持久化授权/撤销、卸载授权清理以及 HTTP/Files
capability Component E2E，以及完整的代码级 View 语义。仍未完成的 MVP 是参考机性能/无障碍与发布证据；
这些缺口不得通过 Tauri、WebView 或常驻 runtime 绕过。

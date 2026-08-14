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

[permissions]
network = ["https://api.example.com"]
clipboard = ["read", "write"]
```

插件 ID 使用反向域名格式，安装后不可更改。命令 ID 只需在插件内唯一；宿主 canonical ID 为 `plugin_id/command_id`。

`kind` 缺省为 `command`，保持旧包兼容。桌面宠物使用 `kind = "desktop-pet"`，可以是只包含 `pet.json` 与受限资源的声明式包并省略 `entry`；若同时提供普通命令，仍通过同一 WIT、权限和按需 Plugin Host 运行。宠物浮层、显示/隐藏、位置、全局快捷键和快捷动作栏由宿主拥有，完整契约见 `docs/aegis/specs/2026-08-14-desktop-pet-plugin-design.md`。

## 3. WIT 世界

WIT 是协议唯一权威，建议拆分为稳定接口：

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
- `open(command-id, context) -> result<session-view, plugin-error>`：创建短生命周期会话
- `update(session-id, event) -> result<view-update, plugin-error>`：处理输入或动作并返回新视图
- `close(session-id)`：释放插件内部会话状态

MVP 的 `view-update` 返回完整官方 View。宿主使用稳定节点 ID 对 Slint 模型进行差量更新；协议层暂不发布通用 UI Patch DSL。

## 4. 官方 UI 协议

插件只能返回以下顶层视图：

- `list-view`：分区、列表项、附件、状态和分页游标
- `grid-view`：固定列策略、网格项和主动作
- `detail-view`：安全 Markdown、元数据、资源引用
- `form-view`：文本、密码、选择、复选、开关和提交
- `empty-view`：标题、说明和最多一个恢复动作

公共结构包括 `navigation-title`、`search-bar`、`action-panel`、`toast` 和 `resource-ref`。插件不能设置任意坐标、字体、颜色、阴影、动画或窗口尺寸。

协议限制：

- 单次 View 编码后不超过 1 MiB
- 单页列表默认最多 100 项，使用游标加载更多
- 所有交互节点必须有当前会话内稳定 ID
- Markdown 禁止原始 HTML、脚本、外部 iframe 和自动网络加载
- 图片必须是包内资源或经宿主 HTTP API 获取的缓存资源句柄
- 宿主校验深度、字符串长度、项数和资源尺寸后才渲染

## 5. 事件与会话

```text
用户打开命令 → initialize（必要时）→ open → View
用户输入/点击 → Event → update → View
返回/关闭/卸载 → close → 销毁 Store 或会话
```

事件携带 `session_id`、`view_revision` 和 `event_id`。宿主丢弃过期 revision 返回，避免慢请求覆盖新状态；用户离开视图即取消未完成调用。插件不能依赖进程常驻保存重要状态，需持久化的数据必须写入宿主 storage API。

## 6. Capability 权限

| 能力 | MVP 规则 |
|---|---|
| Storage | 仅插件命名空间，配额默认 10 MiB |
| HTTP | 仅清单列出的 HTTPS Origin；禁止裸 IP、重定向逃逸和本地网段 |
| Clipboard | 读写分别声明；不开放剪贴板历史 |
| Files | 通过系统文件选择器返回持久或会话句柄，不接受任意绝对路径 |
| Notification | 用户动作触发的有限通知，不得用于后台推送 |
| Logging | 自动标记插件 ID，发布构建做敏感字段过滤 |

MVP 明确不提供进程启动、环境变量、原始 Socket、全局快捷键、系统事件订阅、无界目录扫描和后台定时器。

桌面宠物不会放宽以上限制。声明式动画由主程序渲染，宠物插件不能自行注册全局快捷键、监听桌面输入或通过可选 WASM 命令建立后台循环。

## 7. 安装与更新事务

```text
读取归档
→ 防 Zip Slip/炸弹校验
→ 校验清单、哈希、签名和 API 范围
→ 展示新增权限
→ 解压到 staging
→ Wasmtime 预编译与实例化检查
→ 原子切换 active.json
→ 更新命令索引
→ 异步清理旧版本
```

插件目录按版本保存，更新失败时 active 指针仍指向旧版本。更新增加权限时必须再次确认；拒绝后保留旧版本。发布市场出现前，本地安装包仍需哈希校验，未签名包仅允许开发者模式加载。

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

`novahub` CLI 提供：

```text
novahub plugin new
novahub plugin dev
novahub plugin check
novahub plugin test
novahub plugin pack
novahub plugin sign
```

`dev` 通过受控开发通道热重载插件包，但不绕过资源上限；`check` 校验清单、权限、WIT 兼容、包大小和确定性构建信息。

## 10. 兼容策略

- `manifest_version` 控制包解析格式，未知主版本拒绝安装
- WIT 世界以主版本命名，主版本内只做兼容扩展
- 插件声明支持范围；宿主安装和启动时双重校验
- 废弃接口至少跨一个稳定主版本，并提供编译期迁移提示
- UI 语义由协议控制，Slint 内部实现可演进而不破坏插件
- TypeScript SDK 若未来加入，必须通过同一套协议一致性与行为测试

## 11. 插件测试矩阵

- 清单和签名错误、路径穿越、压缩炸弹
- WIT 主版本不兼容与缺失导出
- 无限循环、内存增长、超大 View、深层 View、无效资源
- 网络重定向到未授权域名和本地网段
- 活跃会话禁用、更新、卸载和 Host 崩溃
- Windows/macOS 同一 View 的键盘、主题和无障碍快照
- SDK 示例插件从创建到打包的端到端测试

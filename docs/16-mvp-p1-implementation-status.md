# NovaHub MVP P1 实施状态

更新时间：2026-08-19

本文记录 Kunkun 参考设计吸收后的 P1 实施结果，作为 `docs/15-kunkun-mvp-adoption-checklist.md` 的补充状态记录。

## 已完成

- 稳定 ID 列表窗口已接入真实 Slint Shell。宿主最多保留 100 条列表项，Shell 只绑定 6 个固定槽位，并提供上一页、下一页按钮。
- 分页事件使用完整快照的偏移量映射回稳定项 ID，不会因窗口切换触发错误项。
- `StableList::window` 负责边界裁剪，渲染组件数量不会随列表总量增长。
- 诊断页面已使用原生 Shell 控件，支持预览、仅失败筛选、清空和导出。
- 诊断预览进入 UI 前限制为 12,000 字符；导出由宿主写入用户数据目录下的时间戳 JSON 文件，插件不能指定路径或直接写文件。
- 诊断内容继续只包含清洗后的生命周期元数据，不包含插件输入、表单值、文件内容、完整路径或完整 URL。
- 破坏性系统命令已接入宿主 ActionPanel 二次确认；确认、取消、Escape 和失焦路径都由主进程 pending action 状态统一裁决。
- 官方 Form View 已贯通当前 WIT 1.6、Rust SDK、Plugin Host、App renderer-neutral surface 与 Slint 原生控件：
  `text`、`password`、`select`、`checkbox`、`switch` 均支持；select 使用稳定 option ID，helper/error、
  初始值和有界 progress 会在宿主侧归一后呈现。
- Form surface 对未知控件、重复/空 option、非法 toggle 初值和无效 select 初始值执行防御性丢弃或归一，
  不把不受信任的 JSON 直接交给 Slint。
- Detail typed metadata 已贯通当前 WIT 1.6、Rust SDK、Plugin Host 和 App：`text`、`link`、`tag` 三种值有
  独立协议类型；宿主限制 key/value 长度、控制字符、重复 key，并只接受 HTTPS link，原生 Detail 文本
  以类型感知格式展示。
- 官方 JSON Toolkit Component 已使用 `tag` metadata；真实 headless App → sibling Host 运行输出包含
  `format: #json`，证明 metadata 没有停留在 mock 或 SDK 层。
- List/Grid item accessory 已贯通 WIT 1.4、Rust SDK、Plugin Host、App renderer-neutral surface 与 Slint
  固定槽位：status、badge、shortcut 和 icon resource ID 均执行长度与控制字符校验；icon 只作为宿主管理资源
  标识呈现，不允许插件提交路径或触发任意资源加载。
- 真实 Component fixture 已通过 WIT → Wasmtime sibling Host → 认证 IPC → App 返回 accessory payload，并断言
  `FIXTURE` badge 与 `fixture.icon` resource ID，证明该语义不是仅在 mock 或 builder 层存在。
- cursor pagination 已贯通 WIT 1.5、Rust SDK、Plugin Host、App 与 Slint。cursor 限制为 256 字符且禁止控制
  字符；宿主只保留当前最多 100 项的页面快照，在固定槽位耗尽后发出 `load_more`，新页面替换旧快照。
- ActionPanel 已在当前 WIT 1.6 中使用 typed default/secondary role；宿主要求每个非空面板恰好一个默认动作，
  且最多保留 6 项。Enter 只触发默认动作；破坏性插件动作进入宿主原生确认态，Confirm 后才发送事件，
  Cancel/Escape/失焦保持不执行。
- 真实 Component E2E 已断言 `fixture-page-2` cursor 返回第二页，并保留 ActionPanel 的 `default`、`secondary`
  与 `destructive` 字段，证明分页和动作角色均经过生产 WIT、Wasmtime sibling Host 与认证 IPC。

## MVP 验证

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --offline -- -D warnings`
- `cargo test --workspace --offline -- --test-threads=1`
- `cargo check --manifest-path plugins/official/translate/Cargo.toml --target wasm32-wasip2 --offline`
- `cargo check --manifest-path plugins/official/json-toolkit/Cargo.toml --target wasm32-wasip2 --offline`
- `cargo run -p xtask --offline -- capability-e2e`
- `cargo run -p xtask --offline -- release-check`

以上必要验证已通过。Form、typed metadata、有界 accessory、cursor pagination 与 ActionPanel 动作角色的
代码级语义已完成验收。发布前仍需在 Windows/macOS 参考机补充 RSS、
Private Working Set、GPU、冷启动、无障碍和 Host 崩溃回收证据；这些是测量工作，不要求引入 Tauri、
WebView 或常驻 Plugin Host。

## Slice 64：Windows Release 性能与进程边界

- Windows Release 默认使用 Slint `winit/software`；`SLINT_BACKEND` 可显式覆盖，用于后端探针和回归对照。FemtoVG 对照约增加 71 MiB GPU Shared，因此 MVP 保留 software 默认以优先控制完整进程树内存。
- Release App 与 Plugin Host 使用 Windows subsystem，进程树不再包含 `conhost.exe`。应用发现改为事件循环启动后的惰性刷新；首帧观察按后端分别使用 `AfterRendering` 或首次 Winit `RedrawRequested` 后的有界回调。
- 资源采样现在覆盖 RSS、Private Bytes、Private Working Set、GPU Dedicated/Shared Memory，并按 GPU engine 类型记录最大利用率，不跨 engine 相加。software 后端没有 GPU 分配时保留不可用状态。
- Wasmtime 启用官方 `cache` feature，App/Host 显式传递用户数据目录下的 `cache/wasmtime`，上限 256 文件、128 MiB；本轮报告缓存为 2 文件、约 121 KiB，不使用自定义预编译格式或 `unsafe deserialize`。
- 最终报告为 `target/reference-benchmark/windows-x86_64-release.json`（20 samples、3 warmup、15 秒资源保持，`effective_policy=winit-software`）。Shell 冷启动首帧 P50/P95 为 919.98/1383.12 ms；1 Host 为 96.27/111.35 ms；4 Host 聚合为 153.43/250.90 ms，完整进程树 RSS 分别为 43.39/44.41/108.97 MiB。
- 本轮只完成 Windows Release 证据。Shell 数值是冷启动首帧，不代表热快捷键到首帧 ≤100 ms；4 Host P95 是全部会话 ready 聚合，不代表单 Host 500 ms 门槛。macOS、无障碍、签名安装器、真实 picker/TLS 和 Host 崩溃回收仍为发布前工作。
- 2026-08-19 必要回归已通过：workspace 格式、严格 Clippy、串行测试、两个官方 `wasm32-wasip2` 插件检查、真实 capability E2E、`release-check`、diff、UTF-8 无 BOM 和退休文档扫描。

本 Slice 保持 Rust + Slint 主 Shell、WIT 唯一 ABI、按需 sibling Host 和标准库线程/有界 mpsc 边界；没有引入 Tauri、WebView、JavaScript runtime、常驻 Host、Tokio 服务或第二套渲染协议。

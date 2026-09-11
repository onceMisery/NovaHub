# NovaHub 低内存架构加固基线

## 状态

2026-08-14 根据技术评审结论及用户确认更新。本文记录对初始基线的增量修订；未列出的所有权与兼容边界继续沿用 `2026-08-12-initial-baseline.md`。

## 第一原则

目标不是以框架名称判定轻量，而是在 Windows/macOS 参考机上证明 NovaHub 具备低空闲内存、低交互延迟、插件隔离和完整无障碍。主 Shell 保持 Rust + Slint，不以 Tauri/WebView 作为局部 fallback。

## 保留架构

- `novahub-app` + 按需共享 `novahub-plugin-host` 双进程拓扑不变。
- WIT 拥有插件 ABI，宿主拥有渲染与 capability 裁决，Plugin Host 拥有 Wasmtime 执行。
- 官方插件与第三方插件走同一 WIT、权限和资源预算，不获得宿主私有 API。
- Wasmtime 与插件 SDK 不进入 `novahub-app` 依赖树。

## 新增硬门槛

- 无插件、无更新任务时完整 NovaHub 进程树闲置 RSS P95 不超过 70 MiB。
- Phase 0 分别测量主进程、Host、完整进程树、私有工作集和可获取的 GPU/纹理内存；一个与四个插件会话的上限在 M1 前冻结。
- Slint 完整组件实现前必须通过 Narrator/VoiceOver、虚拟列表、稳定 ID 差量和候选渲染后端探针。
- SQLite 全量读取、`nucleo` 索引构建/恢复、预览和图片解码退出首帧关键路径。
- WIT package 使用精确 SemVer；Rust、Wasmtime、`wasm-tools`、绑定生成器和 WIT package 版本锁定并通过版本矩阵。
- 插件纯计算由 fuel/epoch 与 2 秒硬 deadline 限制；宿主 IO 使用独立 capability deadline 和取消语义。
- 桌面宠物先静态关键帧后帧动画；静态预算不通过时不得启用动画或提高资源上限。

## 退休与演进

- 退休“Tauri 作为 Slint 无障碍局部替代”的旧路径。
- 无障碍失败先补 `ui-slint` 平台语义桥，必要时桥接局部原生标准控件；仍失败则停止扩展并退回架构评审。
- 受限 WebView 只可能作为独立、按需、可回收 Host，在量化复杂编辑需求和完整进程树预算均成立后评审；不得替换核心 Shell、进入主进程或改变 WIT View 所有权。
- 每插件独立进程仍只由可复现跨插件影响触发，并必须量化额外内存成本。

## 未决证据

当前仓库没有 Rust/Slint/WIT 生产实现，全部资源目标仍需 Phase 0 实测。具体依赖版本由创建工作区时选取当时稳定版本并锁定，本基线不虚构版本号。

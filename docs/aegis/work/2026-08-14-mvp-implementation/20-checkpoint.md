# MVP 实施检查点

## Current Todo

- [completed] Task 1：建立 Rust workspace、工具链和架构测试
- [completed] Task 2：冻结领域、WIT、View 与 IPC 契约
- [completed] Task 3：保持并扩展确定性原型回归
- [completed] Task 4：Slint Shell 与无障碍探针（代码级）
- [completed] Task 5：搜索协调器与 Provider
- [completed] Task 6：平台 capability API 契约与适配器（代码级）
- [completed] Task 7：存储迁移与本地数据边界
- [completed] Task 8：Plugin Host/WIT session 生命周期
- [completed] Task 9：插件清单权限、官方插件与安装事务
- [completed] Task 10：主进程内置命令与迁移 CLI
- [completed] Task 11-12：平台、宠物渲染与发布（代码级；实机发布证据待补）

## Active Slice

当前已形成可运行的本地 MVP 核心闭环：10 个内置 Provider、SQLite 设置/命令/剪贴板元数据、事务迁移 CLI、SDK/官方 JSON 与翻译示例、真实 WIT/Wasmtime Component Host IPC、独立 Slint Shell 和桌面宠物状态机。代码级架构调整已完成；剩余工作是 Windows/macOS 参考机上的原生能力、无障碍、安装器签名和资源基线验收。

## Completed

- 架构文档已按低内存方案加固。
- 评审文档已删除，引用已清理。
- 原型回归此前通过 35 个画板。
- Task 1 架构测试 RED：根 `Cargo.toml` 缺失。
- Task 1 GREEN：`cargo metadata --no-deps`、`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`。
- Task 2 GREEN：Query/Action、View 限额与 stable ID、Protobuf Envelope、SemVer 插件清单和 WIT 1.1.0 契约测试通过。
- Task 4 GREEN（纯 Rust bridge）：Shell 状态焦点转移、stable-ID 列表更新测试通过，`.slint` Shell 源文件存在且架构边界测试通过。
- Task 5 GREEN：SearchCoordinator 过期结果丢弃、generation 从 1 开始和批次上限 50 测试通过。
- Task 6 GREEN（API skeleton）：Available/Unavailable capability 注册与拒绝测试通过。
- Task 7 GREEN（migration skeleton）：连续版本、正向顺序和逆向回滚测试通过。
- Task 8 GREEN（runtime skeleton）：单调 session revision、过期事件拒绝和 close 后拒绝更新测试通过。
- Task 9 GREEN（manager skeleton）：SemVer 权限清单、权限查询和安装提交状态测试通过。
- Task 10 GREEN：`novahub-app` 内置命令搜索测试与 `cargo run -p novahub-app --offline` 通过；主进程未链接 Plugin Host/runtime。
- SQLite GREEN：settings、命令快照、schema 幂等迁移、剪贴板敏感拒绝与 TTL 清理测试通过。
- Provider GREEN：8 个唯一 Provider、无 `eval` 计算器、系统破坏性命令确认和 SHA-256 剪贴板哈希测试通过。
- Migration/CLI GREEN：安全 JSON 导入拒绝脚本/凭据，`novahub migrate --dry-run tests/fixtures/migration-safe.json` 输出 `accepted=2, rejected=1, writes=0`。
- View/SDK GREEN：Grid/Detail/Form/ActionPanel 校验、SDK capability guard、官方 JSON/翻译示例插件测试通过。
- Pet GREEN：desktop-pet manifest/descriptor 资源限制和宿主 PetController 状态转移测试通过。
- Plugin install GREEN：CLI 默认要求 `--signature` 与 `--public-key`，校验 64/32 字节原始密钥材料后调用签名安装；未签名包仅在显式 `--allow-unsigned` 开发者模式下接受。
- Migration transaction GREEN：`migrate --dry-run` 默认零写入，`migrate --apply` 对整批记录先校验后在单个 SQLite 事务中提交，任意错误整体回滚。
- Plugin View GREEN：宿主将 Empty/List/Grid/Detail/Form/ActionPanel 转换为有界 renderer-neutral surface，最多 8 个条目，正文最多 4 KiB，插件不能注入 Slint/HTML。
- Release boundary GREEN：主进程不链接 Wasmtime、Plugin Runtime、Tauri 或 WebView；Component 执行只发生在按需 sibling Plugin Host，Host IPC 具有 2 秒响应期限和有限重试。

## Evidence

- `docs/aegis/baseline/2026-08-14-architecture-hardening-baseline.md`
- `docs/aegis/plans/2026-08-13-v1-design-and-implementation.md`

## Blockers

- 当前无代码级阻塞；跨平台实机验证暂未具备。

## Next Step

代码级 MVP 与低内存架构闭环已完成；下一步只在 Windows/macOS 参考机 CI 采集无障碍、原生平台行为、安装器签名和进程资源基线。

## Drift Check

- Scope: 当前工作仍服务于 Rust + Slint 主 Shell、按需 sibling Plugin Host 和 WIT 唯一 ABI 的既定目标。
- Compatibility: 本轮没有新增运行时、进程所有者、WebView 或插件协议分支。
- Retirement: Tauri/WebView 主 Shell 与隐藏降级路径继续保持退休状态。
- Decision: continue; 代码验证充分，外部平台证据是唯一未闭合项。

## Slice 15 Checkpoint

- Completed: bounded Host frame allocation, request ID echoing, and stable session error codes.
- Evidence: workspace format, Clippy, tests, app bootstrap, and a rebuilt Host process probe all passed; see `90-evidence.md`.
- Blocked: real Wasmtime/WIT execution, Slint code generation, platform APIs, and release resource measurements.
- Next: continue with the smallest dependency-supported Wasmtime/Slint integration slice; do not introduce a Tauri/WebView fallback.

- Scope: aligned with MVP objective and existing architecture.
- Compatibility: no new runtime, fallback, or protocol owner.
- Retirement: no Tauri/WebView main Shell; no competitor compatibility runtime.
- Decision: continue; Task 4 的真实 Slint 编译和跨平台实机能力仍是未完成验收项。

## Slice 17 Checkpoint

- Completed: real `wasm32-wasip2` fixture build, generated WIT bindings, WASI P2 linker setup, bounded Component loading, JSON View serialization, and Host process round-trip.
- Evidence: `cargo fmt --all -- --check`, workspace Clippy, workspace tests with `--test-threads=1`, headless app bootstrap, and the real Host stdin/stdout probe all passed.
- Blocked: Windows/macOS platform adapters, Slint accessibility probes on reference machines, and RSS/cold-start measurements.
- Next: keep the main Shell Rust + Slint and add platform/resource evidence without introducing Tauri/WebView fallback.

- Scope: still within the approved low-memory process model.
- Compatibility: WIT remains the only plugin ABI; WASI is linked only by the Plugin Host runtime Store.
- Retirement: no Tauri/WebView main Shell or JavaScript plugin runtime was introduced.
- Decision: continue; remaining work is platform and release evidence, not a runtime architecture downgrade.

## Slice 18 Checkpoint

- Completed: plugin-manager validates `plugin.wasm` as a real WebAssembly Component with `wasmparser`; core Wasm modules and malformed Component bytes are rejected before extraction.
- Completed: `NovaHubApp::run_component_fixture` and the `NOVAHUB_COMPONENT_FIXTURE` development entry drive one `load/open/update/close` session through the sibling Plugin Host without linking Wasmtime into the app.
- Evidence: plugin-manager tests (9 passed), app tests (5 passed), targeted Clippy, and a real headless app-to-Host fixture run completed at revision 2.
- Blocked: platform adapters, accessibility probes, RSS/private working set measurements, signed release installers, and crash-restart evidence remain external or pending.
- Next: finish platform capability defaults and a host-owned minimal desktop-pet renderer, then collect reference-machine resource/accessibility evidence.

- Scope: remains within the Rust + Slint Shell and on-demand Plugin Host architecture.
- Compatibility: WIT remains the only plugin ABI; `wasmparser` is install-time validation only and Wasmtime remains confined to Plugin Host/runtime.
- Retirement: no Tauri/WebView Shell, JavaScript runtime, or hidden fallback was added.
- Decision: continue; this slice closes package authenticity and application IPC integration gaps without changing the low-memory process model.

## Slice 19 Checkpoint

- Completed: Windows and macOS capability probes now publish only the implemented `FileOpen` capability; Clipboard, GlobalHotkey, and WindowManagement remain explicitly unavailable until real adapters exist.
- Evidence: platform adapter tests, workspace format, Clippy, tests, and release-check passed after the capability correction.
- Blocked: native clipboard/hotkey/window APIs and reference-machine accessibility/resource measurements remain pending.
- Next: implement one host-owned desktop-pet renderer slice or complete the first platform adapter, with tests for unsupported capability behavior.

- Scope: the correction tightens the existing capability boundary and does not add a new process or fallback.
- Compatibility: plugin capability requests still require an explicit registry status; no unsupported operation reports success.
- Retirement: Tauri/WebView remains absent from the main Shell.
- Decision: continue; capability reporting now matches the actual implementation surface.
- Governance note: Aegis helper bundle/check remains blocked by pre-existing workspace metadata gaps; code-level drift and verification checks are current.

## Slice 20 Checkpoint

- Completed: added a real Slint `PetWindow`, host-owned frame/visibility helpers, `pet show`/`pet hide` commands, click-to-shelf state routing, and a bounded action-shelf summary.
- Completed: added logical-coordinate `PetPosition`/`PetPlacement` with display fallback, bounds clamping, edge snapping, and app-owned position APIs.
- Completed: added encrypted, bounded `ClipboardVault` with `CredentialStore`, text/image limits, 500-item LRU, and seven-day TTL tests.
- Evidence: targeted UI/app/domain/provider tests and workspace Clippy passed; full workspace test and release-check passed before the final shelf/retry polish, with those targeted checks rerun afterward.
- Blocked: native Clipboard/Credential Manager/Keychain adapters, platform overlay positioning, accessibility probes, complete shelf data binding, and reference-machine RSS/GPU evidence remain pending.
- Next: run the full verification set, then implement plugin update rollback/pending-delete lifecycle and platform credential adapters as separate slices.

- Scope: all changes remain inside the Rust + Slint Shell and host-owned capability boundaries.
- Compatibility: WIT remains the only plugin ABI; ClipboardVault and PetWindow do not add a plugin runtime or process.
- Retirement: no Tauri/WebView main Shell, JavaScript runtime, or hidden fallback was introduced.
- Decision: continue; this slice materially closes the host rendering/position and clipboard policy gaps but is not MVP completion.

## Slice 21 Checkpoint

- Completed: application discovery now uses a host-owned lazy snapshot capped at 128 entries; search filters the snapshot in memory and exposes an explicit refresh path before reloading platform data.
- Completed: application matches have stable `apps.launch:<name>` identities and exact-name launch resolution remains inside the host platform boundary.
- Completed: Shell clipboard commands now cover history summary, capture, pause/resume, and clear; adjacent duplicate payloads update their timestamp instead of growing the vault, and SQLite metadata updates by content hash.
- Evidence: target package tests, workspace Clippy, workspace tests, headless app bootstrap, `release-check`, `git diff --check`, and UTF-8 BOM scan all passed.
- Blocked: native clipboard listeners, platform overlay positioning, accessibility probes, action-shelf dynamic binding, crash-restart evidence, installers/signing, and reference-machine RSS/GPU measurements remain pending.
- Next: add a host-side clipboard polling/listener scheduler and complete native desktop-pet window behavior without changing the Rust + Slint Shell or Plugin Host process model.

- Scope: remains within the approved low-memory Rust + Slint host architecture; application indexing and clipboard retention are host-owned.
- Compatibility: WIT remains the only plugin ABI; dynamic applications and clipboard history are not exposed as plugin-owned storage or runtime APIs.
- Retirement: no Tauri/WebView Shell, JavaScript runtime, or hidden fallback was introduced.
- Decision: continue; this slice reduces per-keystroke platform work and closes the basic clipboard command loop while leaving native reference-machine evidence outstanding.

## Slice 22 Checkpoint

- Completed: bounded calculator now supports common math functions, constants, and offline dimension-checked unit conversion without adding a script runtime.
- Completed: normal desktop startup opens a user data directory SQLite database; headless and tests retain the in-memory path. The Shell now registers `Alt+Space` on supported desktop targets with recoverable conflict handling.
- Completed: the host-owned PetWindow binds bounded built-in SVG frames, supports drag callbacks, and validates declared pet resources during installation. Example packaging now covers Nova, Pixel, and Waterman.
- Completed: Shell query dispatch was split into readable pet, shortcut, clipboard, provider, and calculator handlers without changing command precedence.
- Evidence: workspace format, workspace Clippy, workspace tests, `release-check`, headless bootstrap, example packaging, and `git diff --check` all passed after the final changes. The retired review documents remain absent and the BOM scan remains clean.
- Blocked: Windows/macOS reference-machine checks for hotkey conflicts, native overlay dragging/positioning, accessibility, signed installers, and RSS/private working set/GPU/cold-start measurements remain pending.

- Scope: changes stay within the Rust + Slint Shell, host-owned platform/storage boundaries, and on-demand Plugin Host process model.
- Compatibility: WIT remains the only plugin ABI; calculator, clipboard, pet assets, hotkey registration, and persistence do not add a WebView or JavaScript runtime.
- Retirement: no Tauri/WebView main Shell, hidden fallback, or competitor compatibility runtime was introduced; `docs/12-technical-review.md` and `docs/13-technical-analysis-report.md` remain retired.
- Decision: continue; the code-level architecture adjustment is implemented and verified, while platform and release evidence is still an explicit acceptance item.

## Slice 23 Checkpoint

- Completed: plugin installation now keeps an atomic `previous.json` pointer; `rollback_plugin` switches to the validated previous version and preserves the enabled flag. CLI now exposes `plugin update`, `plugin rollback`, and reports pending-delete markers.
- Completed: `NovaHubApp` resolves production components from the host-owned active pointer under the user data directory. The explicit `plugin active <id> <input>` command passes only the validated component path to the sibling Host; the development fixture path remains available separately.
- Completed: Plugin Host View responses now produce bounded List/Grid/Detail/Form/ActionPanel summaries for the Shell and headless probe instead of showing only a revision number.
- Evidence: plugin-manager (12 tests), app (16 tests), CLI checks, formatting, and diff checks passed for this slice.
- Blocked: platform-native LaunchServices/Windows Search adapters, overlay positioning on reference displays, accessibility probes, installers/signing, and RSS/private working-set/GPU measurements remain pending.
- Next: finish the remaining host-side search/platform behavior where it can be implemented without unsafe code, then run the full workspace verification set.

- Scope: still within the Rust + Slint Shell, host-owned storage/platform boundaries, and on-demand Plugin Host process model.
- Compatibility: WIT remains the only plugin ABI; active pointers contain only validated version paths; no new runtime or process owner was added.
- Retirement: no Tauri/WebView Shell, JavaScript runtime, or hidden fallback was introduced; the two retired technical review documents remain absent.
- Decision: continue; code-level update/rollback and active-plugin integration are verified, while reference-machine and installer evidence is still outstanding.

## Slice 24 Checkpoint

- Completed: host application search now matches compact names and initials over the bounded 128-entry snapshot, sorts by in-memory launch frequency, and increments usage only after a successful host launch. The snapshot remains explicitly refreshable and no per-keystroke platform enumeration was added.
- Evidence: application matching tests pass; the full workspace verification set is the next acceptance check after this slice.
- Blocked: native index priority, display metrics/overlay placement, accessibility, installer signing, and resource measurements remain reference-machine work.
- Next: run full format, Clippy, workspace tests, release-check, headless bootstrap, example packaging, BOM scan, and update `90-evidence.md` with the fresh outputs.

- Scope: search behavior remains host-owned and bounded; no new index, helper process, or plugin capability was introduced.
- Compatibility: existing command precedence and stable application IDs remain unchanged.
- Retirement: Tauri/WebView remains absent from the main Shell and no compatibility runtime was added.
- Evidence: the full regression command set completed successfully after the search slice; targeted app checks also pass after bounding the usage map at 128 entries.
- Decision: continue; code-level MVP paths are verified, while platform/reference-machine and installer evidence remain explicit external acceptance items.

## Slice 25 Checkpoint

- Completed: the default shortcut is now labeled `Option+Space` on macOS and `Alt+Space` elsewhere; both labels map to the native Alt modifier while custom bindings remain validated by the host.
- Completed: window-centering, edge placement, negative monitor coordinates, logical/physical conversion, and DPI conversion have focused pure-function tests.
- Completed: plugin manifests accept the documented `plugin_api` alias, optional name/publisher/commands, and list or table permission forms; legacy `host_api` and list permissions remain compatible. Install receipts and CLI output now include bounded name, publisher, kind, permission, and command summaries.
- Evidence: target tests, strict workspace Clippy, workspace tests, `release-check`, headless bootstrap, five example archives, `git diff --check`, retired-document checks, and UTF-8 BOM scan all passed.
- Blocked: Windows/macOS reference-machine hotkey conflicts, native overlay/DPI behavior, accessibility narration, signed installers, and RSS/private-working-set/GPU/cold-start measurements remain external acceptance evidence.

- Scope: all changes stay within the existing Rust + Slint Shell, host-owned platform/storage boundaries, and on-demand Plugin Host process model.
- Compatibility: WIT remains the only plugin ABI; manifest additions are metadata-only and preserve old package fields. No new runtime, process, or WebView fallback was introduced.
- Retirement: `docs/12-technical-review.md` and `docs/13-technical-analysis-report.md` remain absent; Tauri/WebView remains outside the main Shell.
- Decision: continue; code-level architecture adjustments are verified, while platform and release evidence remain explicit before production sign-off.

## Slice 26 Checkpoint

- Completed: Shell Escape/失焦行为收敛为可测试的宿主决策：先取消待确认操作，再清空查询并重新聚焦，最后才隐藏窗口；有待确认操作时失焦不会隐藏 Shell。
- Completed: 插件安装/更新增加只读归档预览，预览阶段只解析 `novahub.toml`，不解压、不执行 Component，并覆盖损坏归档与宿主版本不兼容错误。
- Completed: Windows `where.exe` 和 macOS 目录扫描 fallback 默认关闭；只有显式诊断选项才允许递归扫描。Plugin Host IPC reader 现在对无响应进程返回确定性的 2 秒 deadline 错误，并交给现有有限重试 supervisor 处理。
- Evidence: 定向测试新增 Shell 失焦/确认（7 个 app 测试）、归档预览错误（13 个 plugin-manager 测试）、fallback 默认关闭（Windows 4 个、macOS 3 个测试）和真实无响应 Host 超时；定向 Clippy 全部通过。
- Blocked: Windows/macOS 实机快捷键冲突、Spotlight/Windows Search 质量、原生透明浮层与 DPI、无障碍、签名安装器以及 RSS/private working set/GPU/cold-start 指标仍需参考机器验证。

- Scope: 仍保持 Rust + Slint 主 Shell、按需 sibling Plugin Host 和 WIT 唯一 ABI；没有新增 Tauri/WebView、JavaScript runtime 或常驻插件进程。
- Compatibility: 预览和 fallback 策略只收紧宿主边界，不改变 WIT、IPC envelope 或已安装插件的 active pointer 语义。
- Retirement: `docs/12-technical-review.md`、`docs/13-technical-analysis-report.md` 继续保持删除；Tauri/WebView 不作为主 Shell 或故障降级路径。
- Decision: continue; 代码级低内存架构调整和新增边界测试已完成，剩余项目是跨平台与发布证据收集。

## Slice 27 Checkpoint

- Completed: 迁移导入、宿主 View surface、插件更新/回滚、桌面宠物资源、内置 Provider 和平台 capability 默认拒绝路径均已实现并通过代码级回归。
- Evidence: `cargo fmt --all -- --check`、严格 workspace Clippy、串行 workspace tests、`release-check`、`build-examples`、headless bootstrap、迁移 dry-run、`git diff --check` 和 UTF-8 BOM 扫描全部通过。
- Blocked: 真实 Windows/macOS API 质量、全局快捷键冲突、Spotlight/Windows Search 结果质量、原生浮层/DPI、Narrator/VoiceOver、签名安装器以及 RSS/private working set/GPU/cold-start 仍需参考机证据。
- Next: 在参考机 CI 上采集平台与资源基线；若无障碍探针失败，优先补 `ui-slint` 语义桥或局部原生标准控件，不改变 Rust + Slint 主 Shell。

- Scope: 保持 Rust + Slint 主 Shell、按需 sibling Plugin Host、WIT 唯一插件 ABI 和 workspace `unsafe` 禁止边界。
- Compatibility: 迁移和 View surface 只增加宿主校验与有界摘要，不改变已安装插件的 WIT、IPC envelope 或 active pointer 语义。
- Retirement: `docs/12-technical-review.md`、`docs/13-technical-analysis-report.md` 保持删除；没有 Tauri/WebView 主 Shell、JavaScript runtime 或隐藏降级路径。
- Decision: continue; 代码级目标已验证，发布结论等待跨平台实机与资源证据。

## Slice 28 Checkpoint

- Completed: 文件搜索结果现在由宿主执行 `files.open`、`files.reveal`、`files.copy_path` 和 `clipboard.copy`；插件只提交稳定的 Action，不获得文件管理器或剪贴板句柄。
- Completed: 活动自定义桌面宠物通过 active pointer、manifest、`pet.json` 和 SVG 二次校验后加载；单帧最多 2 MiB、活动帧总量最多 8 MiB，`pet use <id>` 会持久化选择，失效资源回退到 Nova。
- Completed: 发布 CI 覆盖 Windows x86_64、macOS x86_64 和 macOS arm64，标签构建上传未签名归档；签名、Notarization 和安装器 smoke 保留在受保护发布环境。
- Evidence: strict fmt、workspace Clippy、workspace tests、`release-check`、五个示例归档、headless bootstrap、迁移 dry-run、`git diff --check`、UTF-8 BOM 扫描均通过。
- Blocked: Windows/macOS 原生 API 质量、全局快捷键冲突、Spotlight/Windows Search、透明浮层/DPI、Narrator/VoiceOver、签名安装器、完整进程 RSS/GPU/冷启动和生产 crash-restart 仍需参考机或受保护发布环境证据。

- Scope: 继续保持 Rust + Slint 主 Shell、按需 sibling Plugin Host、WIT 唯一插件 ABI 和 workspace `unsafe` 禁止边界；文件动作、宠物资源和 CI 没有新增 WebView 或常驻运行时。
- Compatibility: 文件动作通过已有宿主 PlatformServices；自定义宠物只向 Slint 传递经过限制的 SVG 字节，不改变 WIT、IPC envelope 或 active pointer 语义。
- Retirement: `docs/12-technical-review.md`、`docs/13-technical-analysis-report.md` 保持删除；Tauri/WebView 不作为主 Shell 或故障降级路径。
- Decision: continue; 低内存架构的代码级调整和回归验证已完成，剩余是跨平台和发布验收证据，不应通过引入 Tauri 解决。

## Slice 29 Checkpoint

- Completed: 修复 CI 标签构建的 CLI 产物命名，构建目标 `novahub-cli` 现在分别以 `novahub`/`novahub.exe` 写入 macOS/Windows 归档；`release-check` 增加静态断言，防止复制不存在的旧文件名。
- Completed: 修复 `migrate --source <format> <file>` 的参数扫描，`--source` 与 `--db` 的值不再被误认为导入文件；新增 CLI 回归测试。
- Completed: `xtask resource-report --pid <pid>` 递归收集完整进程树，Windows 输出 RSS/Private Bytes，POSIX 输出 RSS 并显式缺省私有工作集。
- Evidence: 定向 Clippy、CLI/xtask 测试和 `resource-report` 运行通过；完整 workspace 回归与发布检查待本 Slice 结束后重跑。

- Scope: 仍保持 Rust + Slint 主 Shell、按需 sibling Plugin Host、WIT 唯一插件 ABI；修复只涉及发布脚本、CLI 参数和观测工具，不新增常驻运行时或 WebView。
- Compatibility: `novahub` 仍是发布归档中的用户-facing CLI 名称，源 Cargo binary 保持 `novahub-cli`；迁移参数向后兼容原有无 `--source` 调用。
- Retirement: 不恢复 Tauri/WebView fallback；两个评审文档继续保持删除。
- Decision: continue; 当前新增问题已修复，需以全量测试和发布检查确认没有回归。

## Slice 30 Checkpoint

- Completed: 官方 JSON Toolkit 与 Translate manifest 现在声明命令；宿主只从 enabled active pointer 读取命令并建立有界搜索索引，使用 `plugin:<plugin_id>:<command_id>` 作为 canonical ID。
- Completed: 选中插件命令后，应用通过 active pointer 将经过校验的 `plugin.wasm` 路径交给 sibling Plugin Host；空输入保留 `open` View，非空输入才发送一次 `update`，不会在宿主进程内引入 Wasmtime。
- Completed: 插件 ID 统一拒绝路径分隔符、冒号、`..` 和控制字符，manifest 解析、安装路径、active pointer、卸载和 pending-delete 共用同一安全边界。
- Evidence: `cargo fmt --all -- --check`、workspace Clippy、串行 workspace tests、`release-check`、五个示例归档、headless bootstrap、带 `--source` 的迁移 dry-run、递归 `resource-report`、`git diff --check` 和 UTF-8 BOM 扫描全部通过。
- Blocked: Windows/macOS 原生 API、快捷键冲突、Spotlight/Windows Search、透明浮层/DPI、Narrator/VoiceOver、签名安装器、GPU/冷启动及生产 crash-restart 仍需参考机或受保护发布环境证据。

- Scope: 继续保持 Rust + Slint 主 Shell、按需 sibling Plugin Host、WIT 唯一插件 ABI 和 workspace `unsafe` 禁止边界；命令索引和 ID 校验没有新增运行时或常驻进程。
- Compatibility: 插件命令仅扩展 manifest 元数据和宿主搜索，active pointer、WIT、IPC envelope 与已有安装包语义保持兼容；无 Tauri/WebView fallback。
- Retirement: `docs/12-technical-review.md`、`docs/13-technical-analysis-report.md` 保持删除；Tauri/WebView 不作为主 Shell 或故障降级路径。
- Decision: continue; 代码级架构调整和回归验证已闭合，剩余事项是跨平台/发布验收证据，不应通过引入 Tauri 解决。

## Slice 32 Checkpoint

- Completed: CLI now exposes `plugin new`, `plugin dev`, `plugin check` and `plugin test`. `new` writes a self-contained Rust/WIT scaffold only into an empty directory; `dev` performs one bounded pack-and-validate cycle; `check/test` validate source directories or archives in temporary storage without touching active pointers.
- Completed: SDK now provides explicit Empty/List/Grid/Detail/Form/ActionPanel builders and a validation-backed `MockHost`; all builders reuse the host-owned View limits and stable-ID checks.
- Evidence: CLI scaffold smoke generated UTF-8 files and its standalone Cargo project passed offline `cargo check`; real JSON Toolkit and Translate Component archives passed `plugin check` and `plugin test`; targeted CLI/SDK tests and Clippy passed.
- Scope: no new runtime, watcher process, WebView, Tauri dependency, or plugin ABI was introduced. Tokio remains outside the current MVP stack.
- Compatibility: WIT remains the only plugin ABI; package validation continues through the existing ZIP, manifest, Component, resource, and temporary install boundary.
- Retirement: `docs/12-technical-review.md` and `docs/13-technical-analysis-report.md` remain deleted; no Tauri/WebView fallback was restored.
- Decision: continue; code-level CLI/SDK gaps are closed, while full official View interaction rendering and reference-machine performance/accessibility evidence remain separate follow-up work.

## Slice 33 Checkpoint

- Completed: 官方 List/Grid/Form/ActionPanel View 已映射到 Shell 的固定槽位；项目激活、Action 触发、表单输入和表单提交都通过短生命周期 Plugin Host 回传，未把插件运行时放进 UI 线程。
- Completed: `PluginViewSurface` 对条目、字段和动作维持有界数量与长度；Slint 侧固定六个槽位，避免动态内容造成布局漂移，并保留 loading、success、error 和普通命令之间的清理逻辑。
- Evidence: `cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets --offline -- -D warnings`、`cargo test --workspace --offline -- --test-threads=1` 全部通过；应用测试覆盖 View surface 限额与交互槽位。
- Blocked: Windows/macOS 原生无障碍朗读、真实 DPI/透明浮层和参考机 RSS/GPU/cold-start 指标仍需实机验证。
- Next: 完成插件安装、调用、禁用、卸载的宿主所有权 E2E，并记录全量回归证据。

- Scope: 交互仅扩展现有 Rust + Slint Shell 与 WIT/IPC 边界，没有新增 WebView、JavaScript runtime 或常驻插件进程。
- Compatibility: WIT 仍是唯一插件 ABI；View 事件通过已有受限请求/响应协议传递，插件不能注入 Slint 或 HTML。
- Retirement: `docs/12-technical-review.md` 和 `docs/13-technical-analysis-report.md` 保持删除；Tauri/WebView 不作为主 Shell 或 fallback。
- Decision: continue; 代码级 View 交互闭环已验证，剩余项属于平台验收证据。

## Slice 34 Checkpoint

- Completed: 生命周期 E2E 覆盖生成插件包、Ed25519 签名安装、active pointer 解析、搜索索引、真实 Component Host 调用、disable 后命令消失以及 uninstall 后版本目录删除。
- Completed: `plugin check/test`、SDK builders 和 `MockHost` 与现有 ZIP、manifest、Component、资源和 API 兼容性边界复用，避免引入第二套插件验证路径。
- Evidence: `cargo test --workspace --offline -- --test-threads=1` 全部通过，其中 `plugin_install_call_disable_uninstall_lifecycle_stays_host_owned` 通过；`release-check`、示例打包、headless Shell、迁移 dry-run、差异和 BOM 扫描均通过。
- Blocked: 发布 Release App 构建、Windows/macOS 实机资源与无障碍、签名安装器和生产 crash-restart 仍是外部发布验收项。
- Next: 以当前代码级证据收束 MVP 记录；不因这些外部证据缺口引入 Tauri 降级路径。

- Scope: 生命周期状态仍由 host-owned plugin manager 管理，Component 执行仍只发生在按需 sibling Plugin Host。
- Compatibility: active pointer 仅接受已验证版本路径；WIT、IPC envelope、权限和稳定命令 ID 未改变。
- Retirement: 两份旧评审文档继续删除；主进程继续不依赖 Wasmtime、Plugin Runtime、Tauri 或 WebView。
- Decision: continue; 代码级架构调整和回归验证已闭合，剩余风险已外部化到发布环境验收。

## Slice 35 Checkpoint

- Completed: 全量格式、Clippy、workspace 测试、发布静态检查、示例打包、headless Shell、迁移 dry-run、资源报告、差异检查和 UTF-8 BOM 扫描均已重新执行并通过。
- Completed: 发布检查确认 CI 产物命名、应用依赖边界和两份旧评审文档删除状态一致。
- Evidence: workspace tests 全部通过；应用测试 25 项通过；`release-check`、`build-examples`、headless bootstrap 和迁移输出均有新鲜命令行证据。
- Blocked: 跨平台原生 API、RSS/Private Working Set/GPU/cold-start、无障碍、签名安装器和 crash-restart 仍需参考机或受保护发布环境；Release App 优化构建已在延长窗口内完成。
- Next: 交付当前代码级架构调整结果，并将上述外部项保留为发布验收清单。

- Scope: 本切片只复核既有实现和证据，不增加运行时、进程、fallback 或插件 ABI。
- Compatibility: Rust + Slint 主 Shell、按需 sibling Plugin Host、WIT 唯一 ABI 和 host-owned platform/storage 边界保持不变。
- Retirement: Tauri/WebView fallback 不恢复；两份旧评审文档保持删除。
- Decision: continue; 代码级 MVP 验证充分，发布级未知项明确记录，不以未知项驱动架构降级。

## Slice 36 Checkpoint

- Completed: `cargo build -p novahub-app --release --offline` 在延长窗口内成功完成，并生成优化版 `target/release/novahub-app.exe`。
- Completed: Release 可执行文件以 `NOVAHUB_HEADLESS=1` 启动成功，输出 `NovaHub host bootstrap (10 builtin command)`。
- Evidence: Release 构建耗时约 5 分 33 秒，退出状态为 0；Release headless 启动退出状态为 0。
- Blocked: 参考机 RSS/Private Working Set/GPU/cold-start、Windows/macOS 原生行为、无障碍、签名安装器和 crash-restart 仍需外部发布验收。
- Next: 交付已通过代码和本机 Release 启动验证的架构调整结果。

- Scope: 只增加 Release 产物验证，不改变 Rust + Slint 主 Shell、按需 sibling Host 或 WIT ABI。
- Compatibility: Release 构建继续保持主应用不链接 Wasmtime、Plugin Runtime、Tauri 或 WebView。
- Retirement: Tauri/WebView fallback 不恢复；旧评审文档继续保持删除。
- Decision: continue; 本机 Release 构建闭合，平台级指标仍按发布验收清单处理。

## Slice 37 Checkpoint

- Completed: 应用发现改为宿主后台 worker，首次最多加载 128 个应用快照；搜索按键只过滤内存快照，避免同步枚举开始菜单或 `/Applications`。
- Completed: 剪贴板平台读取移出 Slint/UI 线程，由短任务线程读取后通过有界 `mpsc` 回传；暂停、去重、加密、TTL/LRU 和 SQLite 元数据仍由宿主统一处理。
- Completed: 文件搜索改为后台 worker，Shell 通过定时器消费有界结果；`files open/reveal/copy_path` 继续保持短命宿主操作和显式 capability 边界。
- Completed: Windows Search、`where.exe`、macOS `mdfind` 与应用索引命令增加 5 秒可终止 deadline；插件 IPC/计算的既有 2 秒限制保持不变。
- Evidence: `cargo fmt --all -- --check`、严格 workspace Clippy、串行 workspace tests、`release-check`、五个示例归档、Release 构建和 `NOVAHUB_HEADLESS=1` 启动均通过；新增应用快照、文件搜索解析、剪贴板 worker 和平台 deadline 测试通过。
- Blocked: 参考机 RSS/Private Working Set/GPU/cold-start、Windows/macOS 原生行为、无障碍、签名安装器和 crash-restart 仍需外部发布验收；macOS 平台 crate 的 x86_64/arm64 交叉检查已通过，完整 workspace 交叉检查仍受当前环境缺少 `cc`（SQLite 原生构建）影响。
- Next: 在 Windows/macOS 参考机 CI 上采集平台 API、无障碍和进程资源证据；继续保持主 Shell 架构，不以外部证据缺口引入 Tauri/WebView fallback。

- Scope: 本切片只收紧宿主调度、平台命令超时和 UI 线程边界，保持 Rust + Slint 主 Shell、按需 sibling Plugin Host、WIT 唯一插件 ABI。
- Compatibility: 后台 worker 使用标准库线程、有界 `mpsc` 和 Slint Timer，不增加常驻 watcher、Tokio 或第二套插件协议；平台超时只终止宿主子进程，不改变 capability 或 IPC envelope。
- Retirement: `docs/12-technical-review.md`、`docs/13-technical-analysis-report.md` 保持删除；Tauri/WebView 不作为主 Shell 或故障降级路径。
- Decision: continue; 代码级低内存架构调整和回归验证已完成，剩余问题属于跨平台/发布验收证据，不需要架构降级。

## Slice 38 Checkpoint

- Completed: Shell 搜索结果改为 8 个固定、可访问且尺寸稳定的结果行，支持选中态、鼠标激活、上下键导航和 Enter 执行；空槽位保持显式，不会导致面板尺寸抖动。
- Completed: `Cmd/Ctrl+K` 打开宿主拥有的 ActionPanel，仅提供有界的“执行当前命令”和“关闭面板”动作；Escape 优先关闭面板，再执行既有 Shell 返回/隐藏逻辑。
- Completed: `clipboard copy <序号>` 只通过宿主平台边界恢复解密后的文本历史项；图片项返回明确的不支持结果，不伪造图片写入能力。
- Completed: worker 与 Plugin Host 响应通道统一使用 `sync_channel(1)`，限制待处理响应占用的内存；UI 回调仍通过 Slint Timer 消费结果，不等待 IPC 或 Wasmtime。
- Evidence: workspace fmt、Clippy、串行测试、release-check、五个示例归档、Debug/Release headless 启动、迁移 dry-run、resource-report、diff check 和 tracked-file BOM 扫描均在本切片后通过。
- Blocked: Windows/macOS 参考机快捷键冲突、原生应用搜索/剪贴板/浮层行为、无障碍朗读、RSS/private-working-set/GPU/cold-start 指标、签名安装器和生产 crash-restart 仍属于发布环境验收；完整 workspace macOS 交叉检查仍受 `libsqlite3-sys` 缺少 `cc` 阻塞。
- Scope: 本切片只在 Rust + Slint、宿主平台服务和按需 sibling Plugin Host 内收紧 Shell 交互与有界调度。
- Compatibility: WIT 仍是唯一插件 ABI；插件不能获得 Slint/HTML 句柄，未引入 Tokio、WebView、Tauri 或常驻 JavaScript runtime。
- Retirement: `docs/12-technical-review.md`、`docs/13-technical-analysis-report.md` 保持删除；没有新增 Tauri/WebView 主 Shell 或隐藏 fallback。
- Decision: continue；代码级架构和有界交互路径已验证，剩余缺口需要参考平台或受保护发布基础设施，不需要架构降级。

## Slice 39 Checkpoint

- Completed: 插件 Form 增加宿主-owned `form_dirty` 状态；编辑中的表单在失焦或 Escape 时保持可见并保留输入，提交成功/失败或切换视图后清理状态。
- Completed: 剪贴板 TTL/数量从编译期常量提升为有界宿主策略，默认仍为 7 天/500 条；策略写入 SQLite，重启后恢复，修改时同步裁剪内存 Vault。
- Completed: 新增按需创建的 Slint `SettingsWindow`，`Cmd/Ctrl+,` 打开主题、全局快捷键、剪贴板 TTL/数量设置；窗口关闭/保存后恢复 Shell，输入值由宿主验证。
- Evidence: `cargo fmt --all -- --check`、workspace Clippy、串行 workspace tests（全部通过）、`release-check`、五个示例归档、Release build/headless 启动、resource-report、diff check 和 BOM 扫描均通过。
- Blocked: Windows/macOS 真实焦点、无障碍朗读、快捷键冲突、原生剪贴板/LaunchServices、RSS/private-working-set/GPU/cold-start、签名安装器和 crash-restart 仍需参考机或发布环境证据。
- Scope: 只在 Rust + Slint 主 Shell、host-owned storage/platform 和按需 sibling Plugin Host 内增加状态保护与设置，不增加进程、WebView、Tauri 或第二套插件 ABI。
- Compatibility: WIT 仍是唯一插件 ABI；设置窗口按需创建，不形成常驻辅助进程；剪贴板策略继续受内存和隐私上限约束。
- Retirement: 两份技术评审文档保持删除；Tauri/WebView 不作为主 Shell 或 fallback。
- Decision: continue；代码级建议已落实并验证，剩余项目是外部平台/发布验收，不应通过架构降级解决。

## Slice 40 Checkpoint

- Completed: 全局快捷键状态从定时器闭包提升为宿主状态；设置保存会先注册新组合、再注销旧组合，失败时尝试恢复旧注册，冲突后可重新绑定。
- Completed: 新增按需创建的 `ClipboardWindow`，提供文本/图片筛选、固定 8 行、文本预览、文本复制、暂停/恢复、清空、固定/取消固定和关闭回 Shell；图片项展示明确的不支持预览状态。
- Completed: `ClipboardVault` 增加固定项语义；固定项不受 TTL 清理影响，数量超限时优先淘汰未固定项，SQLite 旧 schema 会迁移 `pinned` 列，UI 复制改用稳定 item ID。
- Completed: 主题状态现在实际应用到 Shell、ActionPanel、插件视图和设置窗口；保存后即时更新，不要求重启。应用匹配同时覆盖展示名和启动路径文件名别名。
- Completed: 按职责拆分剪贴板窗口回调，并以 `RunHandlerContext` 收拢运行时依赖；主 Shell 仍保持 Rust + Slint、按需 sibling Plugin Host 和 WIT 唯一插件 ABI。
- Evidence: `cargo check --workspace --offline`、`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets --offline -- -D warnings`、`cargo test --workspace --offline -- --test-threads=1`、`cargo build -p novahub-app --release --offline`、Release `NOVAHUB_HEADLESS=1` 启动和 `cargo run -p xtask --offline -- release-check` 均通过；workspace 测试包含应用 31 项、Shell 主程序 17 项、Providers 23 项和 Storage 12 项回归，另有固定项 TTL 与旧 schema 迁移测试。
- Blocked: Windows/macOS 原生焦点与剪贴板、LaunchServices/Windows Search、DPI/透明浮层、Narrator/VoiceOver、参考机 RSS/Private Working Set/GPU/cold-start、签名安装器/Notarization 和生产 crash-restart 仍需外部发布验收。
- Scope: 本切片只增强宿主状态、固定布局、剪贴板策略和代码可读性，不增加进程、WebView、Tauri、常驻 JavaScript runtime、Tokio 或第二套插件 ABI。
- Compatibility: WIT 仍是唯一插件 ABI；Wasmtime 只存在于按需 sibling Plugin Host；两份旧技术评审文档保持删除。
- Decision: continue；代码级架构调整和回归验证已闭合，剩余事项属于跨平台/发布证据，不构成降级为 Tauri 的理由。

## Slice 41 Checkpoint

- Completed: 搜索模块改为低层 `nucleo-matcher`，宿主复用单个 matcher 和文本 scratch，按标题、摘要、ID 做有界模糊排序；未引入高层常驻线程池。
- Completed: 应用搜索保留展示名、启动文件名和拼音别名，并将完整紧凑名/首字母命中提升到稀疏模糊命令之前，避免 `vsc` 被低置信度命令遮蔽。
- Completed: macOS 应用发现优先读取 LaunchServices 注册数据库，失败后使用 `mdfind`；目录扫描仍只作为显式诊断 fallback。Shell 应用快照最多 128 条，启动后台加载，之后 30 秒低频刷新。
- Evidence: 搜索 crate 与应用测试通过（应用库 32 项、Shell 主程序 17 项）；随后 workspace check、Clippy、串行全量测试、Release 构建、headless 启动、release-check、BOM 和 diff 检查均通过，macOS platform crate 的 x86_64/aarch64 交叉检查也通过。
- Blocked: macOS 原生 LaunchServices/NSWorkspace、Windows Search、焦点/DPI/无障碍、参考机资源指标、签名安装器/Notarization 和生产 crash-restart 仍需外部发布验收。
- Scope: 本切片只调整搜索匹配器、应用发现优先级和低频刷新，不增加进程、WebView、Tauri、常驻 JavaScript runtime、Tokio 或第二套插件 ABI。
- Compatibility: WIT 仍是唯一插件 ABI；Wasmtime 只存在于按需 sibling Plugin Host；两份旧技术评审文档继续保持删除。
- Decision: continue；当前调整增强搜索质量和可读性，同时保留低内存架构边界，不构成降级为 Tauri 的理由。

## Slice 42 Checkpoint

- Completed: 将 Kunkun 的命令交互分类真正接入 NovaHub 命令注册和执行路由。宿主快照保存每个插件命令的 `view`/`one-shot`；搜索结果和宠物动作按该类型分派，one-shot 只调用 `run` 并回收 sibling Host。
- Completed: one-shot 结果经过宿主 512 字符上限，完成后清理 Component、View 和表单状态；新增 active manifest 交互查询和对应回归测试。
- Completed: `plugin new` 支持 `--interaction view|one-shot`，manifest 与生成的 Rust/WIT 模板保持同一契约；安装预览输出命令交互和权限 scope/reason；one-shot scaffold 已离线编译通过。
- Evidence: workspace fmt、严格 Clippy、串行 workspace tests、`release-check`、Markdown link check、UTF-8 without BOM 检查均通过。
- Blocked: 权限 broker 的真实平台调用矩阵、stable-ID 到完整 Slint 渲染路径的实机测量、参考机资源/无障碍/签名发布证据仍待后续发布环境；这些不改变当前低内存架构选择。
- Scope: 只补齐 Kunkun 借鉴项在 manifest、WIT、Host 路由、CLI 模板和测试边界的落地；不增加 Tauri/WebView、JavaScript 运行时、watcher、常驻 Host 或第二套插件 ABI。
- Compatibility: WIT 仍是唯一插件 ABI；旧 manifest 缺省 `interaction = "view"`；one-shot 不新增后台生命周期。
- Retirement: 两份旧技术评审文档保持删除，Tauri/WebView 不作为主 Shell 或故障降级路径。
- Decision: continue；代码级契约和路由验证充分，剩余问题属于平台/发布验收或后续能力，不需要架构降级。
## Slice 43 Checkpoint

- Completed: `novahub-app` now owns a fixed-capacity (default 256) diagnostic event ring. Events retain only bounded lifecycle metadata such as phase, request ID, plugin ID, Host PID when available, duration, error class, pointer state, version, package hash, signature state, and `pending_delete`.
- Completed: diagnostic error classes strip text after the first colon, control characters are removed, and pointer metadata accepts only controlled state values. The preview JSON therefore cannot contain query input, form values, file paths, or complete URLs.
- Completed: View and one-shot plugin execution record both success and activation/disabled/Host failure outcomes. `diagnostics.view` is a host-owned searchable command that presents a native bounded summary; the detailed local preview remains an explicit app API and does not start a process.
- Evidence: diagnostic ring, privacy preview, app failure-path, command registration, app/provider tests, strict Clippy, and formatting passed for this slice.
- Blocked: actual platform capability broker calls, full Slint virtual-list measurement, reference-machine resource/accessibility checks, signed installers, and crash-restart evidence remain external or follow-up acceptance items.
- Scope: this slice stays inside the Rust + Slint main Shell and on-demand sibling Plugin Host. It adds no WebView, Tauri fallback, resident watcher, telemetry service, or second ABI.
- Compatibility: WIT and IPC envelopes are unchanged; diagnostics are host-owned metadata and do not become a plugin-visible storage or lifecycle authority.
- Retirement: `docs/12-technical-review.md` and `docs/13-technical-analysis-report.md` remain deleted; Tauri/WebView remains outside the main Shell.
- Decision: continue; the native diagnostics loop is now code-level complete for MVP, while permission broker enforcement and reference-machine evidence remain the next acceptance gaps.

## Slice 44 Checkpoint

- Completed: `plugin-manager` now exposes a stateless `CapabilityBroker` contract with `PluginIdentity`, opaque host-issued `HostFileHandle`, typed `CapabilityRequest`, and deterministic `CapabilityDenied` results.
- Completed: every broker decision rechecks the `EffectiveGrant`; Clipboard read/write are independent, HTTP runtime URLs are reduced to a fresh HTTPS Origin for every hop, file handles are bound to plugin/session identity, and Storage writes are checked against the declared quota.
- Evidence: broker scope, identity, Origin, handle, and quota regressions pass; strict plugin-manager Clippy passes.
- Blocked: the current WIT world has no capability imports, so the broker is not yet called by a real plugin import or native platform adapter. Adding that import and carrying user grants through Host remains the next P0 slice.
- Scope: this is a shared authorization contract only; it does not add a second ABI, runtime, process, path access, or fallback.
- Compatibility: existing manifest parsing and `EffectiveGrant` intersection remain unchanged; plugins without future capability imports continue to behave identically.
- Retirement: Tauri/WebView and JavaScript production runtime remain excluded; the two retired review documents remain deleted.
- Decision: continue; the authorization owner is now explicit and testable, but MVP permission completion still requires WIT/Host wiring and real platform-call fixtures.

## Slice 45 Checkpoint

- Completed: 新增 `docs/15-kunkun-mvp-adoption-checklist.md`，把 Kunkun 借鉴项、NovaHub 落地状态、MVP P0/P1 优先级和验收门槛集中成一份可执行清单。
- Completed: 清单明确区分“CapabilityBroker 契约已实现”和“真实 WIT → Host IPC → 原生 adapter 尚未闭环”，避免将安全契约误报为平台能力完成。
- Scope: 本切片只补充设计与验收文档，不增加进程、运行时、WebView、Tauri 或第二套 ABI。
- Decision: continue; 下一项仍是 P0 capability IPC 与最小宿主适配器，P1 为 stable-ID 到真实 Slint model 的接入和真机资源/无障碍证据。

## Slice 46 Checkpoint

- Completed: WIT capability imports now flow through a runtime adapter into the sibling Plugin Host; Host requests carry plugin identity, session, request ID, protocol version and auth token to the App, which rechecks `CapabilityBroker` before entering the host platform boundary.
- Completed: installed-plugin Shell View, one-shot and subsequent View events retain `ActivePluginExecution`; development fixtures remain on the deny-by-default supervisor path.
- Completed: regressions cover identity mismatch, missing grant, storage quota boundary, authorized-but-unsupported HTTP, and installed Shell execution-context retention.
- Blocked: persistent user grant/revocation, native file/HTTP adapters, capability Component golden E2E, and Windows/macOS reference-machine resource/accessibility/release evidence remain pending.
- Next: add persistent grant/revocation and a real capability Component fixture without changing the Rust + Slint Shell or on-demand Host process model.
- Scope: this slice closes the P0 runtime call path and Shell context propagation; it does not add a process, runtime, fallback, or second ABI.
- Compatibility: WIT remains the only plugin ABI; Wasmtime remains confined to the sibling Plugin Host.
- Retirement: Tauri/WebView, JavaScript production runtime, resident watcher/Host, and the two retired technical review documents remain absent.
- Decision: continue; code-level P0 path is verified, while persistence, golden E2E and platform release evidence remain open.

## Slice 47 Checkpoint

- Completed: host `SQLite` schema version 2 adds a bounded `plugin_user_grants` table; normalized grants survive restart and explicit empty grants persist full revocation.
- Completed: `prepare_active_plugin_execution` now computes `DeclaredPermission ∩ persisted UserGrant ∩ HostPolicy`; updates cannot silently add expanded permissions, malformed persisted grants fail closed, and single-capability or full revocation is supported.
- Completed: native Shell management commands expose permission status, revoke and explicit re-approval without starting Plugin Host.
- Evidence: storage migration/round-trip/input-bound tests, permission narrowing tests, App restart/update/fail-closed tests, Shell management regressions, focused strict Clippy and focused package tests passed.
- Blocked: real capability Component golden E2E, host-owned typed plugin KV operations, and reference-machine resource/accessibility/release evidence remain pending.
- Next: build one real capability Component fixture that proves WIT → Host IPC → App broker → adapter success and denial paths.
- Scope: this slice adds only host-owned persistence and management APIs; it does not add a process, watcher, runtime, fallback, or second ABI.
- Compatibility: old databases migrate idempotently; old grants can only narrow new declarations until the user explicitly approves again.
- Retirement: Tauri/WebView fallback, resident Host/JavaScript runtime and deletion-as-revocation remain excluded.
- Decision: continue; persistent authorization is code-level complete, while capability golden E2E is the next P0 gap.

## Slice 48 Checkpoint

- Completed: the real `wasm32-wasip2` fixture imports WIT `storage-write`; an installed-plugin App test proves success through Component → Wasmtime → sibling Host → authenticated nested IPC → App broker → storage preflight adapter.
- Completed: the same Component returns deterministic `capability not granted` after the persisted storage grant is revoked.
- Completed: `xtask capability-e2e` builds the fixture and Host, injects explicit paths, runs the E2E on all supported CI platforms, and `release-check` verifies the CI step remains present.
- Evidence: the E2E first failed because the fixture only echoed input, then passed after using the generated WIT capability import; the standalone xtask command passed locally.
- Blocked: uninstall/pending-delete grant cleanup, redirect-per-hop, file-handle mismatch, session invalidation and typed KV operation remain for the full P0 matrix; platform release evidence remains external.
- Next: close uninstall/pending-delete grant cleanup, then extend the golden matrix and implement host-owned typed plugin KV.
- Scope: no test-only ABI or in-process shortcut was added; the test uses the production Component, Host process, IPC and App broker path.
- Compatibility: CI adds an on-demand build/test step only; production idle process topology and dependency boundaries are unchanged.
- Retirement: Tauri/WebView, direct Host database access and fake adapter success remain excluded.
- Decision: continue; the first real capability E2E is closed, but the complete P0 matrix is not yet complete.

## Slice 49 Checkpoint

- Completed: plugin uninstall now persists a canonical empty user grant before touching the filesystem; a fully removed plugin then deletes its grant row, while pending-delete retains the empty grant.
- Completed: pending-delete retry returns the successfully removed plugin IDs, allowing the CLI to remove only those authorization rows. The default database path is the plugin root's parent `novahub.sqlite3`, with an explicit `--db` override.
- Completed: CLI and helper tests cover explicit empty grants, terminal uninstall cleanup, retry cleanup, default/overridden database paths, and same-ID reinstall not inheriting old approval.
- Evidence: targeted tests, real CLI uninstall probe, workspace format/check, strict Clippy, serial workspace tests, capability E2E, release-check, diff check, and UTF-8 BOM scan passed.
- Blocked: redirect-per-hop, opaque file-handle mismatch, session invalidation, typed KV operation, and Windows/macOS reference-machine release evidence remain open P0/P1 follow-ups.
- Next: extend the capability golden matrix and add the host-owned typed plugin KV operation without changing the Rust + Slint Shell or on-demand Plugin Host model.
- Scope: authorization lifecycle changes stay in host-owned SQLite and plugin-manager/CLI orchestration; no new process, runtime, fallback, or ABI was introduced.
- Compatibility: empty grant remains distinct from a deleted row; old callers of `retry_pending_deletes` keep the count-returning API.
- Retirement: Tauri/WebView, JavaScript production runtime, resident watcher/Host, and the two retired technical review documents remain absent.
- Decision: continue; uninstall authorization lifecycle is code-level complete, while the remaining P0 matrix and external platform evidence are not yet complete.

## Slice 50 Checkpoint

- Completed: the real capability E2E now also keeps the sibling Host alive after a test-only close request and proves a follow-up update is rejected with `session_not_found`.
- Completed: `xtask capability-e2e` runs both the persisted storage success/revocation test and the closed-session rejection test on the same production Component/Host path.
- Evidence: the focused App tests, strict App/xtask Clippy, and capability E2E passed after the filter and test-only transport hook were added.
- Blocked: redirect-per-hop and opaque file-handle mismatch still lack a complete Component golden matrix; typed KV and native adapters remain open.
- Next: design the smallest WIT-compatible opaque-handle fixture and host-owned typed KV contract without making Plugin Host a storage owner.
- Scope: the test-only close hook is confined to App's IPC client tests; production `close` still shuts down the short-lived Host.
- Compatibility: WIT remains the only ABI and no production process topology changes.
- Retirement: Tauri/WebView and resident runtime fallbacks remain absent.
- Decision: continue; session invalidation evidence is stronger, but P0 authorization matrix is not complete.

## Slice 51 Checkpoint

- Completed: the broker now has a redirect-per-hop golden matrix covering an allowed first hop, an allowed second hop, a disallowed redirect target, and an insecure HTTP target.
- Evidence: `cargo test -p novahub-plugin-manager runtime_http_origin_is_rechecked_for_each_redirect_hop --offline -- --test-threads=1` and strict Plugin Manager Clippy passed.
- Blocked: a real network adapter is intentionally still unsupported; end-to-end redirect transport evidence must wait for a bounded host-owned HTTP adapter. Opaque file-handle mismatch and typed KV remain open.
- Next: keep the redirect matrix at the broker boundary and design the HTTP adapter contract separately, without granting a redirect chain from the first hop.
- Scope: this slice changes only authorization tests; no network client, background worker, process, or ABI was added.
- Compatibility: every runtime URL remains normalized and authorized independently.
- Retirement: Tauri/WebView and resident runtime fallbacks remain absent.
- Decision: continue; broker-level redirect semantics are verified, while adapter-level E2E remains intentionally deferred.

## Slice 52 Checkpoint

- Completed: WIT adds a typed `read-file` capability that accepts only an opaque token. Plugin Runtime binds the token to the current plugin/session before local broker authorization.
- Completed: the App keeps the authoritative issued-token set. A token absent from that host-owned set returns `capability_handle_mismatch` before the unsupported native file adapter boundary.
- Completed: the real Component fixture sends a forged token through WIT → Wasmtime → sibling Host → authenticated IPC → App and receives the stable mismatch code.
- Evidence: workspace check and `xtask capability-e2e` passed with both session invalidation and forged opaque-token coverage.
- Blocked: native file selection/read remains intentionally unsupported; HTTP adapter E2E and typed KV remain open.
- Next: implement host-owned typed plugin KV without allowing Plugin Host to open SQLite, then revisit a bounded native adapter contract.
- Scope: plugins receive no file path or owner identity; the new ABI surface is one bounded token call and no file content is returned without a host-issued token.
- Compatibility: existing WIT calls are unchanged; the new import is additive before the first stable plugin ABI.
- Retirement: Tauri/WebView, direct Plugin Host filesystem access, resident runtime, and second ABI remain excluded.
- Decision: continue; opaque-handle denial is now end-to-end verified, while the real file adapter is not claimed complete.

## Slice 53 Checkpoint

- Completed: Storage schema version 3 adds a host-owned `plugin_kv` namespace with bounded keys/values, atomic quota checks, replacement semantics, and terminal namespace cleanup.
- Completed: WIT `storage-put`/`storage-get` use typed byte payloads; Runtime, authenticated Host IPC, App broker and App-owned SQLite adapter carry the same typed contract.
- Completed: the real Component fixture performs a KV write/read roundtrip; App E2E verifies `kv:ok:hello` and the persisted host row before revocation.
- Completed: CLI terminal uninstall and pending-delete retry remove both grant and KV state transactionally after filesystem deletion succeeds.
- Evidence: Storage/Plugin Manager/CLI tests, strict workspace Clippy, workspace check, and `xtask capability-e2e` passed.
- Blocked: valid native file-handle/read adapter, HTTP adapter E2E, and external platform/release evidence remain open.
- Next: run the final full workspace gate after documentation updates and keep native adapters explicitly unsupported until their host-owned contracts are implemented.
- Scope: Plugin Host never opens SQLite; KV calls reopen only the App-selected database through the App capability handler, with bounded values and no resident service.
- Compatibility: existing `storage-write` quota preflight remains supported; typed KV imports are additive before stable ABI freeze.
- Retirement: Tauri/WebView, direct Host database access, resident runtime, and second ABI remain excluded.
- Decision: continue; typed KV is now code-level MVP complete, while native adapter and external evidence gaps remain.

## Slice 54 Checkpoint

- Completed: Kunkun adoption is now explicit in the authority docs rather than only in the reference analysis. The MVP boundary includes command interaction, typed authorization, permission diff, host-owned View semantics, stable-ID rendering, CLI/SDK, diagnostics, trust facts, and capability success/denial paths.
- Completed: the host capability adapter boundary is frozen as `WIT → sibling Host → authenticated IPC → App-owned broker → host-owned adapter`; it adds no second ABI or authorization owner.
- Completed: bounded MVP contracts now require HTTPS GET with per-hop authorization, redirect/deadline/size limits, and system-picker-issued file tokens with session binding and bounded UTF-8 reads.
- Completed: adoption status no longer reports full View semantics or the complete capability matrix as implemented; remaining P0/P1 work is separated from completed code.
- Verification: documentation consistency, whitespace, UTF-8 without BOM, retired review-document absence, `xtask release-check` and `xtask capability-e2e` passed for this slice.
- Scope: this slice changes architecture and delivery documentation only; it introduces no dependency, runtime, process, code path or fallback.
- Compatibility: Rust + Slint remains the main Shell, WIT remains the only plugin ABI, and Wasmtime remains confined to the on-demand sibling Host.
- Retirement: Tauri/WebView fallback, JavaScript production runtime, resident watcher/network runtime, and `docs/12`/`docs/13` remain excluded.
- Decision: continue to verification; HTTP/file adapter implementation and reference-machine evidence remain the next MVP work.

## Slice 55 Checkpoint

- Completed: WIT `pick-file`、Runtime、认证 IPC 与 App broker 形成 `系统 picker → opaque token → read-file` 成功路径；token 绑定 plugin/session，内容限制为 512 KiB UTF-8，插件不获得路径或 OS handle。
- Completed: Windows/macOS adapter 按需调用原生 picker，App 在每次成功或失败的 Component session 后通过显式 `end_session` 清理文件 token，不依赖缓存执行上下文析构。
- Evidence: 有效 token、伪造 token 和 session 清理必要测试通过；`capability-e2e` 通过真实 Component/Host/IPC 覆盖选择与读取往返。
- Boundary: `rfd` 只在用户动作中按需调用；未增加常驻进程、线程池、watcher、WebView 或第二套 ABI。
- Residual risk: macOS 真实 picker、取消交互和跨平台文件编码仍需参考机 smoke。
- Decision: continue；Files 代码级 P0 闭环完成，真实平台证据仍是发布门槛。

## Slice 56 Checkpoint

- Completed: Windows/macOS adapter 使用同步 `ureq + rustls` 执行单跳 HTTPS GET，禁用自动重定向；App 最多处理 5 跳并在每一跳重新调用 broker。
- Completed: 连接/读取/总时限为 3/5/10 秒，响应限制为 512 KiB UTF-8；HTTP、URL credentials、零上限和零 timeout 在网络 I/O 前拒绝，timeout 映射为稳定 `http_deadline_exceeded`。
- Evidence: 平台输入边界必要测试、App HTTP 大小/编码/逐跳授权测试、严格 Clippy、workspace 测试与 `capability-e2e` 通过。
- Boundary: 主 Shell 未引入 Tokio、Tauri、WebView 或 Wasmtime；HTTP adapter 不拥有 redirect policy，不增加常驻 runtime。
- Residual risk: 真实公网 TLS、代理/证书环境、二进制体积增量和完整进程树 RSS 仍需参考机测量。
- Decision: continue；HTTP 代码级 P0 闭环完成，真实平台与资源证据仍是发布门槛。

## Slice 57 Checkpoint

- Completed: Kunkun 借鉴状态已同步到参考分析、MVP 清单、采纳规格和发布文档；P0 代码闭环与 P1/Phase 2 边界不再混写。
- Completed: capability handler 按 clipboard、files、storage、HTTP 拆分私有方法，过长测试拆分为 fixture 与断言函数，保持代码可读性。
- Evidence: `cargo fmt --all -- --check`、严格 workspace Clippy、串行 workspace 测试、`capability-e2e`、`release-check`、`git diff --check`、UTF-8 无 BOM 与退休文档检查均通过。
- Scope: 仍服务于 Rust + Slint、WIT 唯一 ABI 与按需 sibling Host；没有引入新的权限所有者或 fallback。
- Retirement: Tauri/WebView、JavaScript 生产 runtime、常驻 Host/watcher，以及 `docs/12`/`docs/13` 继续保持退休。
- Decision: continue；Kunkun 优点的架构吸收与 P0 代码闭环已有新验证证据，P1 和参考机发布证据继续按 MVP 清单推进。

## Slice 58 Checkpoint

- Completed: `StableList` 增加 host-owned bounded window，限制可见行与 overscan，并按 stable ID 保留 selection、focus 和 scroll anchor；100/1,000/10,000 行必要测试覆盖窗口不随数据总量增长。
- Completed: diagnostics 增加最近 N 条/仅失败删减选项，以及宿主显式 JSON 文件导出接口；导出仍只包含已清洗生命周期元数据。
- Evidence: `cargo fmt --all`、`cargo clippy -p novahub-ui-slint -p novahub-app --all-targets --offline -- -D warnings`、UI/App 必要测试通过（UI 13、App 51）。
- Boundary: 插件仍只提交完整 View，窗口和导出均由宿主拥有；没有新增 Patch ABI、后台服务、WebView 或常驻 runtime。
- Residual risk: 真实 Slint 控件绑定、原生诊断页面按钮/文件 picker、参考机帧时间/RSS/GPU/无障碍证据仍未完成。
- Decision: continue；本轮完成 P1 的宿主模型层，下一步接入真实 Slint surface 与原生诊断页面交互。

## Slice 59 Checkpoint

- Completed: WIT 1.2 Form contract, Rust protocol validation, SDK builders, Plugin Host/App mapping and native
  Slint controls are connected for text/password/select/checkbox/switch, helper/error, initial values and
  determinate progress.
- Completed: App parser normalizes unknown controls, duplicate/empty options, invalid toggle defaults and invalid
  select defaults before rendering; the UI bridge now uses a named field-slot configuration for readability.
- Evidence: targeted strict Clippy passed; serial tests passed with App 54, Shell main 20, Plugin Host 5, SDK 3,
  UI protocol 6 and Slint 13; official translate and json-toolkit Components passed wasm32-wasip2 checks.
- Blocked: accessory, cursor pagination, reference-machine performance/accessibility/release evidence and signed
  installer/crash-restart checks remain pending.
- Evidence: final workspace fmt, strict Clippy, serial tests, capability E2E, release-check, diff, UTF-8 BOM and
  retired-document checks passed. A cold capability-E2E attempt exceeded the command limit, while isolated stages
  and a fresh complete rerun passed without code changes or a fallback.
- Next: implement the next bounded P1 semantic slice (accessory, typed metadata or cursor pagination) while keeping
  reference-machine performance/accessibility/release evidence explicit.
- Scope: remains Rust + Slint main Shell, WIT-only plugin ABI and on-demand sibling Plugin Host.
- Compatibility: Form fields are additive within the pre-stable WIT contract; existing text-only JSON remains
  backward compatible through host defaults.
- Retirement: Tauri/WebView, JavaScript production runtime, resident Host/watcher and retired review documents
  remain absent.
- Decision: continue; Form is code-level complete, but MVP acceptance still requires remaining View semantics and
  external platform/release evidence.

## Slice 60 Checkpoint

- Completed: WIT 1.3 typed Detail metadata (`text`/`link`/`tag`) is validated in ui-protocol, built by the Rust SDK,
  serialized by Plugin Host and normalized/rendered by App.
- Completed: official JSON Toolkit emits a tag metadata value and a real headless App → sibling Host probe prints
  the type-aware `format: #json` Detail line.
- Evidence: workspace Clippy/tests, official wasm32-wasip2 checks, capability E2E, release-check, format/diff/BOM
  checks all passed after the metadata change.
- Blocked: accessory, cursor pagination/default action and external platform/resource/accessibility/release evidence
  remain pending.
- Next: implement the next bounded View semantic without creating a patch ABI or persistent runtime.
- Scope: Rust + Slint Shell, WIT-only ABI and on-demand sibling Host remain unchanged.
- Compatibility: WIT 1.3 is an additive pre-stable contract change; the SDK text convenience keeps existing Rust
  callers readable while the host payload is now explicitly typed.
- Retirement: no string-only dual metadata owner, Tauri/WebView fallback, JavaScript runtime or resident Host was
  retained.
- Decision: continue; typed metadata is code-level complete, while MVP still has accessory, pagination and external
  release-evidence work.

## Slice 61 Checkpoint

- Completed: WIT 1.4 adds one bounded `item-accessory` contract for List/Grid items with status, badge, shortcut
  and host-owned icon resource ID; Rust UI protocol, SDK, Plugin Host and App share that single semantic source.
- Completed: the Slint Shell maps accessory data into the existing six fixed item slots through the readable
  `PluginItemSlot` configuration. Icons remain lightweight resource markers; plugins cannot submit paths or trigger
  arbitrary resource loading.
- Evidence: strict workspace Clippy, serial workspace tests, real capability Component E2E and `release-check`
  passed. The fixture payload preserves `FIXTURE` badge and `fixture.icon` through WIT, Wasmtime sibling Host,
  authenticated IPC and App parsing.
- Blocked: cursor pagination, ActionPanel default/secondary action semantics and external platform/resource/
  accessibility/release evidence remain pending.
- Next: implement cursor pagination or the bounded ActionPanel default-action contract without adding a patch ABI,
  persistent runtime or a second UI owner.
- Scope: Rust + Slint Shell, WIT-only ABI and on-demand sibling Host remain unchanged.
- Compatibility: WIT 1.4 is an additive pre-stable contract change; omitted accessory data keeps existing items
  valid, while host validation rejects oversized, control-character or empty accessory payloads.
- Retirement: Tauri/WebView fallback, JavaScript production runtime, resident Host/watcher and retired review
  documents remain absent.
- Decision: continue; accessory is code-level complete, while MVP still requires pagination/default-action semantics
  and external release evidence.

## Slice 62 Checkpoint

- Completed: WIT 1.5 adds an optional bounded `next-cursor` to List/Grid pages. Rust protocol validation and SDK
  builders limit opaque cursors to 256 control-free characters while retaining the existing 100-item page bound.
- Completed: Plugin Host and App preserve the cursor; Slint consumes local fixed slots first and emits one
  `load_more` event only at the page boundary. A returned page replaces the host snapshot, so memory does not grow
  with pagination history.
- Evidence: focused protocol/SDK/Host/App tests, strict workspace Clippy and real capability Component E2E passed.
  The fixture returns `fixture-page-2`, receives it through the production event path and returns `fixture.page-2`.
- Scope: the plugin still returns complete pages. No Patch DSL, cursor cache process, resident Host or second ABI was
  introduced.
- Next: implement the bounded ActionPanel default/secondary role and host-owned destructive confirmation.
- Decision: continue; cursor pagination is code-level complete and external platform/release evidence remains open.

## Slice 63 Checkpoint

- Completed: WIT 1.6 gives every ActionPanel item a typed default/secondary role. Protocol validation requires one
  and only one default action per non-empty panel; SDK helpers keep the common first-action path concise while also
  exposing explicit default and secondary builders.
- Completed: App rejects missing, duplicate or unknown default roles and panels above six actions before rendering.
  Slint routes Enter only to the validated default action; secondary actions remain explicit buttons.
- Completed: destructive plugin actions enter a host-owned native confirmation state. Confirm emits the action event;
  Cancel/Escape clears it, and focus loss never executes it.
- Evidence: full serial workspace tests passed, including App library 57, Shell main 21, Plugin Host 5, SDK 3 and
  UI protocol 10; strict workspace Clippy, both official Component checks and real Component E2E passed. The fixture
  preserved default/secondary/destructive fields through WIT, Wasmtime sibling Host and authenticated IPC.
- Scope: the change adds no plugin-native dialog, fallback renderer, resident process or authorization owner.
- Next: complete Windows/macOS reference-machine performance, accessibility, crash-recovery, signing and platform
  smoke evidence. No code-level official View semantic remains on the MVP checklist.
- Decision: continue; code-level View semantics are complete, while external release evidence remains pending.

## Slice 64 Checkpoint

- Completed: Windows Release 默认渲染策略固定为 `winit-software`；GPU/软件后端均有 backend-neutral 首帧观察路径，应用发现改为事件循环启动后的惰性刷新。
- Completed: App 与 sibling Plugin Host 使用 Windows subsystem；资源采样覆盖 RSS、Private Bytes、Private Working Set、GPU Dedicated/Shared Memory 和按 engine 类型的最大利用率。
- Completed: Wasmtime 官方 `cache` feature 已启用，缓存目录由 App/Host 显式传递并受 256 文件、128 MiB 上限约束；不使用自定义预编译格式或 unsafe deserialize。
- Evidence: `target/reference-benchmark/windows-x86_64-release.json` 记录 20 samples、3 warmup、15 秒资源保持和 `winit-software` 策略；Shell、1 Host、4 Host 场景均含完整进程树资源指标。
- Evidence: 2026-08-19 的 `cargo fmt --all -- --check`、严格 workspace Clippy、串行 workspace 测试、两个官方 Component 检查、`xtask capability-e2e`、`xtask release-check`、`git diff --check`、UTF-8 无 BOM 与退休文档扫描全部通过。
- Scope: 本 Slice 只硬化 Windows Release 资源与启动关键路径，不改变 Rust + Slint Shell、WIT 唯一 ABI、按需 sibling Host 或插件权限边界。
- Retirement: Tauri/WebView、JavaScript runtime、常驻 Host、Tokio 服务和第二套 ABI 仍未引入；`docs/12` 与 `docs/13` 继续退休。
- Blocked: macOS renderer/资源/首帧、无障碍、签名安装器/Notarization、真实 picker/TLS 和 Host 崩溃回收仍缺实机证据。
- Decision: continue; Windows 低内存切片已有直接证据，但 MVP 不标记整体完成，下一步只做必要跨平台发布验证。

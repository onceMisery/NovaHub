# NovaHub V1 设计原型与实施计划

## Goal

交付 NovaHub V1 的现代化桌面原型与生产实现：Windows 完整覆盖 14 项高频能力，macOS 覆盖核心能力；支持 uTools/Raycast 配置与清单迁移，但不运行其插件。

## Architecture

生产架构保持 `novahub-app` + 按需 `novahub-plugin-host`。宿主使用 Rust + Slint，内置 Provider 直接调用平台能力；官方工具使用 Wasmtime Component Model + WIT。设计轨使用本地确定性 HTML 原型作为可验证源真值，再映射到 Slint 和 Figma。

## Tech Stack

- Rust stable workspace、Tokio、Slint、SQLite/`rusqlite`、`nucleo`
- Wasmtime Component Model、WIT、Protobuf IPC、`tracing`
- Windows `windows` crate；macOS `objc2`/AppKit 与系统服务
- 原型：现有无构建步骤 HTML/CSS/JavaScript；Figma 三页 handoff
- 测试：Rust 单元/集成/属性测试、Slint 快照、Playwright 原型/E2E、Criterion/自定义性能采样

## Baseline/Authority Refs

- `docs/03-system-architecture.md`
- `docs/04-plugin-platform.md`
- `docs/05-ui-ux-design.md`
- `docs/07-security-privacy.md`
- `docs/08-quality-release.md`
- `docs/11-v1-popular-tools-and-modern-experience.md`
- `docs/aegis/specs/2026-08-13-v1-popular-tools-design.md`

## Compatibility Boundary

- WIT 是插件 ABI 唯一权威；SDK 与迁移器不得复制协议所有权。
- 不执行 uTools/Raycast 插件、脚本或私有 API，不导入凭据、剪贴板与云数据。
- 现有 10 个原型 URL、`.novahub-plugin` 生命周期和默认删除语义保持兼容。
- Windows/macOS 共享领域与 View 协议；平台差异只存在于 adapter/capability 层。

## Verification

每个阶段先建立失败测试或可观察验收，再实施最小功能。合并门槛包含 `cargo fmt --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`、WIT conformance、原型 Playwright、平台 E2E 和固定参考机性能报告。

## File Map

计划中的目标结构如下；文件在对应任务开始时创建，不预建空目录。

```text
apps/novahub-app/                 # Shell、协调器、组合根
apps/novahub-plugin-host/         # Wasmtime 与 IPC 进程
apps/novahub-cli/                 # SDK、打包、迁移命令
crates/core-domain/               # Query/Result/Action 等领域类型
crates/search/                    # Provider、取消、合并与排序
crates/storage/                   # SQLite schema、加密载荷、迁移
crates/ui-protocol/               # 官方 View/Event
crates/ui-slint/                  # 令牌、组件与窗口
crates/platform/windows/          # Windows adapter
crates/platform/macos/            # macOS adapter
crates/providers/                 # 8 个内置能力族
crates/migration/                 # 中立模型、解析器与事务导入
crates/plugin-manager/            # 安装、权限、默认删除
crates/plugin-runtime/            # Wasmtime、限制与会话
wit/novahub-plugin/               # ABI 源真值
sdk/rust/                         # Rust SDK 薄封装
plugins/official/                 # 翻译与开发者工具插件
prototype/                        # 确定性设计源真值
tests/e2e/                        # 跨进程和平台流程
benches/                          # 性能预算
```

## Ripple Signal Triage

V1 扩展了能力范围，但不新增协议所有者。影响扩展到 search/provider 注册、storage schema、UI 状态、平台 capability、官方插件和迁移导入。所有下游都通过既有领域/WIT/View 契约连接；任何实现若要求竞品特有字段进入 `core-domain` 或 WIT，必须退回规格评审。

---

## Task 1：建立 Rust 工作区与质量门槛

**Files:** 创建根 `Cargo.toml`、`rust-toolchain.toml`、`deny.toml`、`.cargo/config.toml`、`xtask/`、基础 crate 清单；创建 `.github/workflows/ci.yml`。

**Why:** 给后续每个功能提供统一编译、测试、许可和平台 CI 基线。

**Impact/Compatibility:** 只建立骨架，不实现产品能力；crate 名称和依赖方向必须匹配架构文档。

**Verification:** `cargo metadata --no-deps` 成功；Windows CI 完整运行，macOS CI 至少编译与核心测试。

- [ ] 写一个架构测试，断言 `core-domain` 不依赖 Slint、Wasmtime、SQLite 或平台 crate。
- [ ] 运行 `cargo test -p architecture-tests`，确认因工作区尚未建立而 RED。
- [ ] 创建最小 workspace、crate manifest、共享 lint 与 MSRV/稳定工具链配置。
- [ ] 运行 `cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`，确认 GREEN。
- [ ] 提交 `build: establish NovaHub Rust workspace and CI gates`。

## Task 2：冻结领域、WIT、View 与 IPC 契约

**Files:** 创建 `crates/core-domain/src/*`、`crates/ui-protocol/src/*`、`wit/novahub-plugin/*.wit`、`crates/ipc/proto/*.proto` 与对应测试。

**Why:** 先固定跨模块、跨进程与插件边界，避免各能力产生重复模型。

**Impact/Compatibility:** 延续现有包格式与官方声明式 UI；给类型加字段只能是向后兼容 optional/default。

**Verification:** WIT round-trip、Protobuf unknown-field、View 限额和 stable-ID 测试通过。

- [ ] 写 Query/Result/Action、View/Event、session lifecycle 和错误码的序列化/约束测试。
- [ ] 运行 `cargo test -p core-domain -p ui-protocol -p ipc`，确认缺少类型而 RED。
- [ ] 实现最小领域模型、WIT world、生成绑定和有界 IPC envelope。
- [ ] 重跑测试并执行 `cargo run -p xtask -- check-contracts`，确认 GREEN。
- [ ] 提交 `feat: define core, view, WIT, and IPC contracts`。

## Task 3：扩展现代化确定性原型

**Files:** 修改 `prototype/index.html`、`prototype/styles.css`、`prototype/app.js`、`prototype/components.html`、`prototype/README.md`、`prototype/figma-sync-manifest.json`；创建 `tests/prototype/*.spec.ts` 与截图基线。

**Why:** 在 Slint 实现前验证 14 项能力的信息架构、密度、错误和破坏性流程。

**Impact/Compatibility:** 保留 L1–L4、P1–P3、M1–M3 路由；新增 C/B/F/Q/W/D/T/G/R 状态，不改变生产技术选型。

**Verification:** 所有路由可直接加载；720/760/840/880/960 宽度和 1024/1440 桌面视口无内部溢出、控制台错误或资源失败。

- [ ] 先写 Playwright 路由、键盘、焦点、主题、200% 文本与 reduced-motion 验收，确认新增路由 RED。
- [ ] 运行 `npx playwright test tests/prototype` 并保存 RED 报告。
- [ ] 实现 P0 的 Command Center、Clipboard、File Search、Gallery、Migration，再实现 P1 工具流程和组件状态。
- [ ] 运行 Playwright、截图差异和可访问树检查，确认 GREEN；人工审阅浅/深主题关键截图。
- [ ] 提交 `design: expand deterministic V1 desktop prototype`。

## Task 4：实现 Slint Foundations、Shell 与无障碍基线

**Files:** 创建 `crates/ui-slint/ui/tokens.slint`、`components/*.slint`、`windows/*.slint`、Rust bridge 与快照测试。

**Why:** 将已验证原型转换为稳定、低内存的生产 UI 组件系统。

**Impact/Compatibility:** 颜色、间距、圆角与组件语义以 `docs/05` 和原型为准；不引入 WebView。

**Verification:** Launcher/Tool/Manager 三种窗口快照匹配；键盘、焦点和语义树测试通过。

- [ ] 写 SearchInput、ResultRow、ActionPanel、Form、Dialog、Toast、Preview 和 ToolSplitView 的状态快照测试。
- [ ] 运行 `cargo test -p ui-slint`，确认缺少组件而 RED。
- [ ] 实现语义令牌、虚拟列表、Shell 状态机、主题/缩放/减少动画适配。
- [ ] 重跑测试，并在 Windows Narrator 与 macOS VoiceOver smoke 流程验证 GREEN。
- [ ] 提交 `feat(ui): implement Slint foundations and shell`。

## Task 5：实现搜索协调器与 Provider 框架

**Files:** 创建 `crates/search/src/{provider,coordinator,ranker,registry}.rs`、固定语料和 benchmark。

**Why:** 让多来源结果在低延迟下可取消、可合并且不打乱当前选择。

**Impact/Compatibility:** 插件只注册静态 command descriptor，不参与每键动态查询。

**Verification:** 旧 query 丢弃、慢源 Partial、stable-ID 选择和 P95 预算测试通过。

- [ ] 写取消、deadline、流式首批、排序、固定项和 provider failure 测试。
- [ ] 运行 `cargo test -p search`，确认 RED。
- [ ] 实现有界通道协调器、`nucleo` 匹配、稳定合并与指标埋点。
- [ ] 重跑测试与 `cargo bench -p search`，首批本地结果目标 GREEN。
- [ ] 提交 `feat(search): add cancellable provider coordination and ranking`。

## Task 6：实现平台服务骨架与 Windows 完整适配

**Files:** 创建 `crates/platform/api`、`platform/windows` 与契约测试；覆盖快捷键、窗口、应用、文件、剪贴板、凭据、取色、系统命令。

**Why:** 为 Windows 14 项能力提供正式 OS 所有者。

**Impact/Compatibility:** 不支持能力返回 capability=false；禁止空操作成功和 UI 直接调用 Win32。

**Verification:** Windows 11 当前版本与上一支持版本的 adapter/E2E 测试通过。

- [ ] 写平台 trait 契约、权限/索引不可用、DPI/多显示器和敏感剪贴板排除测试。
- [ ] 运行 Windows adapter tests，确认 RED。
- [ ] 分片实现 Windows API，并给每个阻塞调用配置取消与线程边界。
- [ ] 重跑契约/E2E/资源泄漏测试，确认 GREEN。
- [ ] 提交 `feat(windows): implement V1 platform capabilities`。

## Task 7：实现存储、安全与 8 个内置 Provider

**Files:** 创建 `crates/storage/migrations/*`、`crates/providers/{apps,files,clipboard,snippets,quicklinks,windows,system,calculator}`。

**Why:** 完成 V1 高频、性能敏感和隐私核心能力。

**Impact/Compatibility:** 文件依赖系统索引；剪贴板加密且默认 7 天/500 项；Snippets 不执行脚本。

**Verification:** schema 升降级、加密、配额、过滤、Provider 结果与端到端动作通过。

- [ ] 为每个 Provider 写领域测试，并写 migration rollback、加密载荷、TTL/LRU 与敏感来源测试。
- [ ] 运行 `cargo test -p storage -p providers`，确认 RED。
- [ ] 按 apps→calculator→quicklinks/snippets→files→clipboard→system/window 顺序实现最小闭环。
- [ ] 重跑测试、Windows E2E 与 RSS/延迟基准，确认 GREEN。
- [ ] 提交 `feat: deliver built-in V1 providers and secure storage`。

## Task 8：实现 Plugin Host、权限与生命周期

**Files:** 创建 `apps/novahub-plugin-host`、`crates/plugin-runtime`、`crates/plugin-manager`、WIT host adapters 和故障测试夹具。

**Why:** 为官方/第三方工具提供可安装、可卸载、受限执行的前后端插件能力。

**Impact/Compatibility:** 共享 Host 按需启动、60 秒空闲回收；UI 仍由宿主渲染；卸载默认删除所有插件本地所有物。

**Verification:** 陷阱、超时、64 MiB、权限拒绝、Host 崩溃、原子更新、pending_delete 流程通过。

- [ ] 写恶意/损坏组件、越权、更新失败和活跃卸载测试。
- [ ] 运行 `cargo test -p plugin-runtime -p plugin-manager`，确认 RED。
- [ ] 实现 Wasmtime Store limiter、epoch/fuel、capability broker、认证 IPC 与事务生命周期。
- [ ] 重跑单元/跨进程 E2E，确认内置能力在 Host 崩溃后仍 GREEN。
- [ ] 提交 `feat(plugins): add isolated runtime and transactional lifecycle`。

## Task 9：交付 5 个官方工具插件族

**Files:** 创建 `plugins/official/{translate,json-toolkit,codec-toolkit,color-toolkit,qr-toolkit}`、共享测试夹具与包清单。

**Why:** 覆盖热门网络/开发者工具，并真实验证 SDK、View 与权限契约。

**Impact/Compatibility:** 官方插件无隐藏宿主 API；取色和文件选择经 capability broker；网络权限按域名声明。

**Verification:** Mock Host、真实 Host、离线/超时、大输入和包签名测试通过。

- [ ] 为五个插件写 golden input/output、错误 View、权限拒绝和取消测试。
- [ ] 运行 `cargo test --manifest-path plugins/official/Cargo.toml`，确认 RED。
- [ ] 先实现纯本地 JSON/codec，再实现 color/QR，最后实现可配置服务的 translate。
- [ ] 打包并在真实 Host 安装/执行/卸载，确认 GREEN 且无残留命名空间。
- [ ] 提交 `feat(official-plugins): add translation and developer tool suite`。

## Task 10：实现 Migration Center 与 CLI

**Files:** 创建 `crates/migration/src/{model,utools,raycast,plan,import}.rs`、`apps/novahub-cli/src/migrate.rs`、公开脱敏语料和 UI 流程。

**Why:** 让用户迁移 Quicklinks、Snippets、别名、设置与插件清单，并给开发者重写路径。

**Impact/Compatibility:** 仅解析用户选择的导出文件；源内容永不执行；不读取凭据、私有数据库或云端。

**Verification:** 分类确定性、冲突预览、恶意输入、事务回滚、dry-run 和报告快照通过。

- [ ] 写 uTools/Raycast 公开导出样例、路径穿越、超大输入、脚本字段与凭据拒绝测试。
- [ ] 运行 `cargo test -p migration`，确认 RED。
- [ ] 实现 `MigrationItem`、两个 parser、三类分类器、dry-run、事务导入与 Markdown/JSON 报告。
- [ ] 重跑测试与 `novahub migrate --dry-run <fixture>`，确认 GREEN 且文件系统无额外写入。
- [ ] 提交 `feat(migration): add safe uTools and Raycast migration assistant`。

## Task 11：实现 macOS 核心适配并消除平台漂移

**Files:** 创建/完善 `crates/platform/macos`、macOS 权限说明、签名 entitlements 与平台 E2E。

**Why:** 保持双平台架构真实可用，而不是 Windows 完成后的概念兼容。

**Impact/Compatibility:** 同一 platform trait 和领域测试；高级差异通过 capability matrix，不创建 macOS 专用 WIT。

**Verification:** macOS 完成应用、文件、剪贴板、Quicklinks/Snippets、计算、翻译/工具、核心窗口与插件管理流程。

- [ ] 在 macOS CI/实机运行共享契约测试，记录 RED 能力差距。
- [ ] 为 LaunchServices、Spotlight、Keychain、Pasteboard、Accessibility permission 写失败测试。
- [ ] 实现最小 adapter 与权限恢复路径，逐项关闭差距。
- [ ] 重跑共享测试、VoiceOver smoke、签名与 notarization dry run，确认 GREEN。
- [ ] 提交 `feat(macos): deliver V1 core platform capabilities`。

## Task 12：Figma 同步、全量验证与发布候选

**Files:** 更新 `prototype/figma-sync-manifest.json`、`docs/10-figma-prototype.md`、`docs/08-quality-release.md`；创建性能、无障碍、安全与迁移报告。

**Why:** 给设计评审、工程发布和长期回归留下可追溯证据。

**Impact/Compatibility:** Figma 配额不可用时不阻塞 RC；本地原型和 Slint 测试是源真值，Figma 是镜像交付。

**Verification:** 全工作区、两平台核心 E2E、14 项矩阵、性能 P95、安装更新回滚与默认删除全部通过。

- [ ] 先生成发布检查报告，确认缺失证据导致 gate RED。
- [ ] 同步 Figma 三页或记录配额阻塞，并校验 manifest 中每个画板都有本地证据。
- [ ] 修复全量测试、性能、无障碍、安全扫描和安装升级中发现的问题。
- [ ] 运行 `cargo run -p xtask -- release-check` 与平台安装包 smoke，确认全部工程 gate GREEN。
- [ ] 提交 `release: prepare NovaHub V1 release candidate evidence`。

---

## Milestones And Exit Gates

| 里程碑 | 包含任务 | 退出条件 |
|---|---|---|
| M0 合同与探针 | 1–2 | 架构、WIT、IPC、CI 可运行 |
| M1 体验与 Shell | 3–5 | 确定性原型与 Slint Shell、搜索预算通过 |
| M2 Windows Alpha | 6–8 | 8 个内置能力与插件生命周期主流程通过 |
| M3 工具迁移 Beta | 9–10 | 5 个插件族、Gallery、迁移报告与回滚通过 |
| M4 双平台 RC | 11–12 | macOS 核心、发布、安全、无障碍和性能门槛通过 |

## Risks And Rollback

- Slint 无障碍不足：先在 `ui-slint` 增加平台语义桥，不在同一 Shell 混入 WebView；仍失败时退回架构评审。
- 系统索引质量不足：功能显示受限状态，保留系统索引实现；自建索引需独立 ADR 和资源预算。
- Figma 配额持续不可用：保留本地确定性原型、截图、manifest 与 handoff ZIP，工程不等待 Figma。
- 迁移格式变化：parser 按版本隔离，未知字段保留在诊断但不写入；关闭对应源而不宽松猜测。
- 性能超预算：先关闭预览/缩略图等非核心路径并分析分配，不牺牲隔离或默认删除。

## Retirement

- `docs/02-mvp-requirements.md` 保留为历史 MVP 基线；V1 范围由 `docs/11` 接管，不删除旧验收。
- 原有 L/P/M 10 个原型状态保留并纳入回归；新的 C/B/F/Q/W/D/T/G/R 状态逐步成为 V1 设计基线。
- 不创建竞品兼容运行时，因此没有需长期维护的代码执行 fallback。
- Figma 恢复同步后，handoff ZIP 继续作为版本化证据，但不成为令牌或组件的双重所有者。

## Plan Self-review

- 14 项能力均有产品规格、所有者、原型状态、实现任务与验证路径。
- 直接兼容、凭据导入、任意脚本和 WebView 明确排除。
- Windows/macOS、WIT/View、数据删除和性能边界保持一致。
- 每项任务都有 RED→最小实现→GREEN→提交的执行序列；实际代码签名以 Task 2 冻结的契约为准，后续任务不得自行复制类型。
- 无未决占位、模糊任务或未定义的兼容 fallback。

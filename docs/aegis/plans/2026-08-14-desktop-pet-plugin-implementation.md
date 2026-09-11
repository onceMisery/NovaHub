# NovaHub MVP 桌面宠物插件实施计划

## Goal

在 NovaHub MVP 中实现声明式桌面宠物插件：用户可以安装和制作自定义宠物，宠物停靠桌面边缘并支持显示/隐藏、拖动吸附、点击互动、Pet Action Shelf 与多显示器位置记忆。宿主拥有窗口、权限、快捷键和生命周期；插件只提供受限资源、状态图和推荐动作。

## Architecture

保持 `novahub-app` + 按需 `novahub-plugin-host`。新增 `PetController`、`PetOverlay`、`PetRenderer` 与 `PetActionResolver`，全部属于宿主；纯声明式宠物不启动 Plugin Host。普通插件包默认 `kind = "command"`，桌面宠物包使用 `kind = "desktop-pet"`，可选 WASM 只用于用户主动执行的普通插件命令。

宿主负责：

- 透明无边框浮层、位置、DPI、多显示器、点击穿透和边缘手柄。
- 显示/隐藏、右键菜单、可配置全局快捷键、系统托盘恢复。
- 状态机、动画预算、资源回退和异常隔离。
- Pet Action Shelf、目标命令解析和目标权限校验。

插件负责：

- `pet.json`、包内图像/动画/声音资源。
- `idle/interact/working/success/error` 等声明式状态和推荐动作。
- 可选的普通 WIT 命令会话，不得修改宠物窗口或建立后台循环。

## Tech Stack

- Rust stable workspace、Tokio、Slint、SQLite/`rusqlite`、`tracing`
- Wasmtime Component Model、WIT、Protobuf IPC
- Windows Win32/DPI/工作区 API；macOS AppKit/NSPanel/屏幕与辅助功能 API
- 现有 HTML/CSS/JavaScript 确定性原型与 Playwright 验收
- Rust 单元、属性、WIT conformance、跨进程 E2E、性能采样和平台无障碍测试

## Baseline/Authority Refs

- `docs/aegis/specs/2026-08-14-desktop-pet-plugin-design.md`
- `docs/02-mvp-requirements.md`
- `docs/03-system-architecture.md`
- `docs/04-plugin-platform.md`
- `docs/05-ui-ux-design.md`
- `docs/07-security-privacy.md`
- `docs/08-quality-release.md`
- `prototype/README.md`
- `prototype/figma-sync-manifest.json`

## Scope Check

### Facts

- 当前仓库已提交文档与无构建步骤 HTML 原型，生产 Rust 工作区尚未创建。
- 现有插件包默认按命令插件设计，WIT 是 ABI 唯一权威。
- 用户已确认 A1：图标化边缘控制；显示/隐藏文案只出现在右键菜单、Tooltip、设置和无障碍名称中。
- 用户已确认声明式宠物包、Waterman/Nova/Pixel 三个内置宠物和宿主管理的快捷入口。

### Assumptions

- MVP 同时只激活一个宠物。
- 宠物资源与普通插件资源共享签名、归档、大小和删除流程。
- `waterman.jpg` 只有在版权或许可证记录完成后才能作为分发资产；否则使用原创替代图，但保留 Waterman 交互与包结构。

### Unknowns

- 当前 Windows/macOS 生产窗口层尚未存在，平台 API 的最终命名以 Task 1/2 的现有工程骨架为准。
- Figma Starter MCP 可能仍不可用，本计划以本地原型、截图和 manifest 作为设计源真值。

## Compatibility Boundary

- 未声明 `kind` 的旧包解析为 `command`，现有 10 个原型 URL、普通 WIT 和卸载默认删除语义不变。
- 不运行、翻译或包装 uTools/Raycast 插件代码，不导入凭据、剪贴板历史或云数据。
- `desktop-pet` 不增加 WIT UI 类型；声明式资源由宿主渲染，普通命令仍复用现有 WIT。
- 宠物不获得任意窗口、全局输入、屏幕读取、进程启动、原始 Socket、WebView 或后台定时器能力。
- Windows/macOS 共享领域模型和测试；差异只存在于 `platform/windows` 与 `platform/macos`。

## File Map

设计/原型：

```text
prototype/index.html
prototype/app.js
prototype/styles.css
prototype/v1.css
prototype/README.md
prototype/figma-sync-manifest.json
tests/prototype/prototype.spec.js
assets/pets/builtin/waterman/source.jpg
assets/pets/builtin/nova/*
assets/pets/builtin/pixel/*
```

生产目标：

```text
crates/core-domain/src/pet.rs
crates/plugin-manager/src/manifest.rs
crates/plugin-manager/src/pet_package.rs
crates/plugin-runtime/src/pet.rs
crates/ui-protocol/src/pet.rs
crates/ui-slint/src/pet_controller.rs
crates/ui-slint/src/pet_renderer.rs
crates/ui-slint/ui/pet.slint
crates/platform/windows/src/pet_overlay.rs
crates/platform/macos/src/pet_overlay.rs
crates/storage/migrations/*_desktop_pet.sql
wit/novahub-plugin/pet.wit
plugins/official/pets/{waterman,nova,pixel}/
tests/e2e/desktop_pet.rs
benches/desktop_pet.rs
```

## Ripple Signal Triage

本功能触及包解析、插件生命周期、宿主 UI、平台窗口、设置存储、命令注册和测试下游，但不新增协议所有者。`PetController` 是宠物可见性和状态唯一所有者；`PetActionResolver` 是动作目标唯一解析者；包解析器是 `kind` 和资源清单唯一所有者。不得在插件、平台 adapter 或 UI 组件中复制这些决策。

## Task 1：冻结旧包兼容与宠物契约测试

**Files:** `crates/plugin-manager/src/manifest.rs`、`crates/core-domain/src/pet.rs`、`crates/ui-protocol/src/pet.rs`、`wit/novahub-plugin/pet.wit`、`tests/contracts/desktop_pet.rs`。

**Why:** 先用失败测试固定 `kind` 默认值、宠物状态、动作目标和资源限制，防止实现过程把宠物能力散落到多个模块。

**Impact/Compatibility:** 未声明 `kind` 的旧清单必须解析为 `command`；普通插件不出现宠物字段时序列化结果保持兼容。

**Verification:** `cargo test -p contracts --test desktop_pet`；`cargo run -p xtask -- check-contracts`。

- [ ] 写清单兼容、`pet.json` schema、状态转移、动作目标和资源上限测试。
- [ ] 运行 `cargo test -p contracts --test desktop_pet`，确认因目标 crate 尚不存在而 RED。
- [ ] 创建领域类型、版本常量、WIT 声明式数据结构和序列化实现。
- [ ] 重跑测试与契约检查，确认旧 command 包和新 desktop-pet 包均 GREEN。
- [ ] 提交 `feat(pet): freeze desktop pet contracts and compatibility tests`。

## Task 2：实现安全宠物包解析与安装校验

**Files:** `crates/plugin-manager/src/pet_package.rs`、`crates/plugin-manager/src/resource_limits.rs`、`crates/plugin-manager/tests/pet_package.rs`、`assets/pets/fixtures/*`。

**Why:** 把资源、路径、签名和大小校验放在安装源头，避免渲染器或插件运行时做下游补丁。

**Impact/Compatibility:** 复用现有 `.novahub-plugin` staging、签名、Zip Slip 和原子切换流程；普通 command 包路径不改变。

**Verification:** `cargo test -p plugin-manager pet_package`；恶意 fixture 全部拒绝，合法 fixture 全部可安装。

- [ ] 写路径穿越、符号链接、重复文件、压缩炸弹、畸形图片、缺失 idle 和超限资源测试。
- [ ] 运行宠物包测试，确认缺少解析器和限制器而 RED。
- [ ] 实现 `kind` 分派、`pet.json` 解析、资源句柄和 25 MiB/4096px/240 帧限制。
- [ ] 重跑测试并验证签名覆盖 `novahub.toml`、`pet.json` 和全部资源。
- [ ] 提交 `feat(plugins): validate and stage desktop pet packages`。

## Task 3：实现宿主 PetController 状态机

**Files:** `crates/ui-slint/src/pet_controller.rs`、`crates/core-domain/src/pet.rs`、`crates/storage/src/pet_settings.rs`、对应单元测试。

**Why:** 由单一宿主所有者管理显示/隐藏、暂停、状态事件、单活动宠物和恢复策略。

**Impact/Compatibility:** 不让插件或平台窗口层持有业务状态；普通插件生命周期不变。

**Verification:** `cargo test -p core-domain -p ui-slint pet_controller`；完整状态机无非法转移。

- [ ] 写 `hidden/entering/idle/interact/working/success/error/sleeping/suspended` 转移、单活动宠物、卸载回退和损坏状态测试。
- [ ] 运行测试确认 RED。
- [ ] 实现事件 reducer、持久化配置、状态资源回退和 Nova 星灵恢复。
- [ ] 重跑单元测试，并加入重启/崩溃恢复测试确认 GREEN。
- [ ] 提交 `feat(pet): add host-owned pet controller state machine`。

## Task 4：实现 Windows/macOS 浮层、位置和图标手柄

**Files:** `crates/platform/windows/src/pet_overlay.rs`、`crates/platform/macos/src/pet_overlay.rs`、`crates/ui-slint/ui/pet.slint`、平台契约测试。

**Why:** 提供 A1 桌面边缘体验，同时把系统窗口权限隔离在平台 adapter。

**Impact/Compatibility:** 宿主控制透明浮层、DPI、工作区和点击穿透；插件不能设置任意窗口属性。

**Verification:** Windows/macOS 平台测试覆盖拖动吸附、多显示器、DPI、全屏、点击穿透、图标手柄恢复。

- [ ] 写窗口层 mock、位置约束、显示器移除、点击穿透恢复和手柄可关闭测试。
- [ ] 运行平台测试，确认 RED。
- [ ] 实现平台浮层 adapter、逻辑坐标存储、右键入口和图标化手柄。
- [ ] 在两平台 smoke 流程验证手柄、托盘、设置页至少保留一个恢复入口。
- [ ] 提交 `feat(platform): add desktop pet overlays and edge handle`。

## Task 5：实现宿主渲染器与三个内置宠物资源

**Files:** `crates/ui-slint/src/pet_renderer.rs`、`crates/ui-slint/ui/pet.slint`、`plugins/official/pets/{waterman,nova,pixel}/`、`assets/pets/builtin/*`、资源 golden 测试。

**Why:** 将声明式状态先映射为低资源静态关键帧，再在资源预算证据允许时启用帧动画，并交付 Waterman、Nova、Pixel 三个首发宠物。

**Impact/Compatibility:** 只允许宿主安全资源子集；Waterman 原图必须先通过版权/许可证检查，不能引用用户图片绝对路径。

**Verification:** `cargo test -p ui-slint pet_renderer`、静态关键帧截图、关闭/静态/动画三场景资源报告和 12→6→静态降级测试通过；无法确认版权时 Waterman 使用原创替代资源并保留相同 manifest。

- [ ] 写静态关键帧、缺失状态回退、减少动画、深浅主题、最大合法资源、版权资产和“预算未通过时禁止启用帧动画”测试。
- [ ] 运行 renderer 测试确认 RED。
- [ ] 先实现静态关键帧、淡入淡出、声音默认关闭和三套资源 manifest；静态资源报告 GREEN 后才实现完整帧动画与 12→6→静态降级，未达标则静态路径即为交付形态。
- [ ] 运行截图快照、关闭/静态/动画内存与 CPU 采样、降级测试和 `cargo test`，确认 GREEN。
- [ ] 提交 `feat(pet): ship built-in Waterman Nova and Pixel pets`。

## Task 6：实现 Pet Action Shelf 与动作权限解析

**Files:** `crates/ui-slint/src/pet_action_shelf.rs`、`crates/core-domain/src/action.rs`、`crates/search/src/registry.rs`、`crates/plugin-runtime/src/action_bridge.rs`、测试夹具。

**Why:** 让宠物提供实用快捷入口，同时复用现有 command/Quicklink/Snippet/插件命令权限。

**Impact/Compatibility:** 宠物建议最多 3 项，用户固定最多 5 项；不自动固定、不复制权限、不增加新的动作执行管线。

**Verification:** `cargo test -p ui-slint pet_action_shelf`；鼠标、方向键、数字键、Enter、Escape 和失效目标测试通过。

- [ ] 写固定项上限、推荐项不自动固定、目标禁用/卸载、权限拒绝、working/success/error 映射测试。
- [ ] 运行动作测试确认 RED。
- [ ] 实现宿主 Shelf、`PetActionResolver`、ActionPanel 扩展和目标能力权限调用。
- [ ] 重跑测试并测量打开反馈 ≤100 ms，确认 GREEN。
- [ ] 提交 `feat(pet): add host-owned action shelf and command bindings`。

## Task 7：实现设置、右键菜单、快捷键和恢复路径

**Files:** `crates/storage/src/pet_settings.rs`、`crates/ui-slint/ui/settings-pet.slint`、`crates/ui-slint/src/pet_settings_view.rs`、平台快捷键/托盘 adapter、UI 测试。

**Why:** 把显示/隐藏、边缘手柄、快捷键、点击穿透、大小、透明度、声音和启动策略做成可发现、可恢复的设置。

**Impact/Compatibility:** 不在桌面永久显示“显示/隐藏”文案；关闭手柄和快捷键后仍保留托盘/设置恢复入口。

**Verification:** UI 自动化覆盖右键、设置、Tooltip、键盘焦点、托盘恢复和最后恢复路径保护。

- [ ] 写设置默认值、修饰键冲突、手柄关闭、右键隐藏、点击穿透和恢复入口测试。
- [ ] 运行 settings UI 测试确认 RED。
- [ ] 实现宿主设置页、图标手柄、可配置全局快捷键和恢复保护。
- [ ] 在 Windows/macOS 进行键盘、屏幕阅读器、深色、高对比度和 200% 文本验证。
- [ ] 提交 `feat(pet): add configurable visibility and recovery controls`。

## Task 8：扩展现有确定性原型与 Figma manifest

**Files:** `prototype/index.html`、`prototype/app.js`、`prototype/styles.css`、`prototype/v1.css`、`prototype/README.md`、`prototype/figma-sync-manifest.json`、`tests/prototype/prototype.spec.js`、`work/board-PET-*.png`。

**Why:** 在生产实现前提供可审阅的 A1 桌面宠物、右键菜单、配置页和 Pet Action Shelf 证据。

**Impact/Compatibility:** 保留现有 L/P/M、C/B/F/Q/W/D/T/G/R 路由和 35 个 V1 画板；新增 PET 分区，不修改生产 UI 技术选型。

**Verification:** `node tests/prototype/run.cjs` 输出现有 35 个 V1 画板并增加 PET 状态；无控制台错误、内部溢出或资源失败。

- [ ] 先写 PET 路由、A1 显示/隐藏、右键菜单、Action Shelf、设置和 200%/reduced-motion 测试，确认 RED。
- [ ] 运行 Playwright 记录 RED 输出。
- [ ] 实现 Waterman 预览、图标手柄、配置控件、快捷动作和异常状态画板。
- [ ] 运行 Playwright、截图和人工视觉检查，确认旧路由回归 GREEN。
- [ ] 提交 `design: add desktop pet prototype states and action shelf`。

## Task 9：补齐跨平台、安全、性能和发布验收

**Files:** `tests/e2e/desktop_pet.rs`、`tests/security/desktop_pet.rs`、`benches/desktop_pet.rs`、`docs/08-quality-release.md`、`docs/10-figma-prototype.md`、发布检查报告。

**Why:** 让桌面宠物在资源、窗口、权限、无障碍和性能边界内可发布，而不是只完成视觉演示。

**Impact/Compatibility:** 只增加宠物验收矩阵，不降低既有 Plugin Host、默认删除和平台发布门槛。

**Verification:** `cargo test --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、宠物 E2E、安全扫描、Criterion/固定参考机采样和 Playwright 全量通过。

- [ ] 写资源 fuzz、Host 崩溃、卸载清理、显示器热插拔、Narrator/VoiceOver、关闭/静态/动画 RSS/CPU/FPS 与 12→6→静态降级验收。
- [ ] 运行全量检查，确认 RED 项目可追踪到具体任务。
- [ ] 修复测试暴露的问题，保持宿主和契约的单一所有者。
- [ ] 生成 Windows/macOS 核心报告、截图证据、版权记录和 Figma manifest 校验。
- [ ] 提交 `test: verify desktop pet security accessibility and performance`。

## Repair Track

- 包解析修复点：`plugin-manager` 是 `kind` 和资源验证的唯一所有者。
- 状态修复点：`PetController` 是可见性、单活动宠物、恢复和状态转移的唯一所有者。
- 动作修复点：`PetActionResolver` 是目标解析和权限委托的唯一所有者。
- 窗口修复点：平台 adapter 只实现系统窗口行为，不复制领域状态。
- 视觉修复点：`PetRenderer` 和官方 Slint 组件渲染声明式状态，不下放任意 UI 权限。

## Retirement Track

- 保留普通 `command` 包路径和旧清单默认值，删除触发条件是所有已支持旧版本均通过迁移/回归测试且主版本窗口结束。
- 不创建宠物专用后台进程或脚本 fallback；若声明式状态机不足，必须新建经过评审的能力和权限契约。
- 不在平台 adapter、插件包或原型中保留重复的显示/隐藏状态所有者。
- 现有 L/P/M 与 V1 画板继续回归；PET 画板只新增状态，不替代旧证据。

## Risks

- Waterman 版权未确认：实现前必须有许可证记录，否则替换资源而不阻塞协议、状态机和 UI 实现。
- Windows/macOS 浮层 API 未在当前仓库落地：先用 mock 契约和本地原型验证，再进入平台 adapter。
- 宠物动画资源过大：优先降帧、静态化和尺寸限制，不提高宿主内存预算。
- 用户关闭所有恢复入口：设置必须阻止该组合，保留托盘或设置页恢复。
- Figma Starter 配额：本地原型、截图和 manifest 作为源真值，远端同步不成为生产实现前置条件。

## Plan Self-review

- 设计规格每项验收均映射到至少一个任务和明确文件。
- 普通插件兼容、权限隔离、默认删除、无障碍、性能和版权门槛均有验证路径。
- 每项任务包含 Write test → RED → Minimal code → GREEN → Commit 五步。
- 未引入新的 fallback、重复所有者或未定义的协议字段。
- 生产实现前置依赖为 Task 1/2 契约，原型 Task 8 可并行但不能成为生产契约所有者。

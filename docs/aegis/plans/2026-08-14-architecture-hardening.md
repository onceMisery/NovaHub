# NovaHub 低内存架构加固计划

## Goal

在不引入 Tauri/WebView 主 Shell 的前提下，修订 NovaHub 架构基线，补齐 Slint 可行性探针、全进程资源预算、渲染与启动关键路径、Wasmtime/WIT 版本和插件超时语义。

## Architecture

保留 `novahub-app` + 按需共享 `novahub-plugin-host`、Rust + Slint、WIT 官方 View 和宿主 capability broker。此次只收紧所有权、失败路径与验收口径，不新增运行时或第三进程类型。

## Tech Stack

当前 MVP 的并发实现以 `std::thread`、有界 `mpsc` 和 Slint Timer 为准；Tokio 仅在网络或结构化异步 I/O 进入范围并完成内存基线评估后再引入。

Markdown 架构文档；当前 MVP 技术栈为 Rust、Slint、SQLite、低层 `nucleo-matcher`、Wasmtime Component Model、WIT 和 Protobuf。并发执行由标准库线程、有界 `mpsc` 和 Slint Timer 负责；Tokio 不属于当前 MVP 依赖。

## Baseline/Authority Refs

- `docs/03-system-architecture.md`
- `docs/04-plugin-platform.md`
- `docs/05-ui-ux-design.md`
- `docs/08-quality-release.md`

## Compatibility Boundary

- WIT 继续是插件 ABI 唯一权威，官方 View 不引入 DOM/HTML 语义。
- 主 Shell 不引入 Tauri/WebView；无障碍失败先由 Slint 语义桥或局部原生控件处理。
- Plugin Host 保持按需共享与空闲回收；独立进程只由可复现隔离问题触发。
- 既有插件包、数据删除和 Windows/macOS 平台边界不变。

## Verification

使用 UTF-8 读取全部受影响 Markdown，通过 `rg` 检查旧 Tauri fallback、资源预算、WIT 版本、双超时、索引关键路径和宠物分期，并运行 `git diff --check`。

## Ripple Signal Triage

变更影响架构、UI、插件协议、数据、发布、路线图、V1 规格与两个实施计划；不扩展协议所有者或产品范围。采纳结论直接进入权威架构与增量基线，不保留重复评审文档作为并行来源。

## Task 1：修订架构与插件契约

**Files:** `docs/03-system-architecture.md`、`docs/04-plugin-platform.md`。

**Why:** 把资源预算、渲染后端、启动关键路径、模块所有权、版本和超时语义提升为架构约束。

**Impact/Compatibility:** 不改变进程拓扑和 WIT 所有权，只补充可验证边界。

**Verification:** `rg -n "进程树|渲染后端|计算预算|IO deadline|package.*版本" docs/03-system-architecture.md docs/04-plugin-platform.md` 命中全部约束。

- [ ] 写检索断言并确认新约束当前缺失。
- [ ] 运行断言，确认 RED。
- [ ] 最小修改架构与插件协议文档。
- [ ] 重跑断言，确认 GREEN。
- [ ] 审阅差异，不在本任务自动提交。

## Task 2：修订资源、UI、数据、发布与路线图

**Files:** `docs/02-mvp-requirements.md`、`docs/05-ui-ux-design.md`、`docs/06-data-search-builtins.md`、`docs/08-quality-release.md`、`docs/09-roadmap-and-final-form.md`、`docs/11-v1-popular-tools-and-modern-experience.md`。

**Why:** 让低内存目标以完整场景衡量，并将 Slint 探针和非 Tauri 失败路径写入权威文档。

**Impact/Compatibility:** 70 MiB 指标从“无 Host 主进程”收紧为“无插件、无更新任务时完整进程树”；新增插件活动场景，不放宽原上限。

**Verification:** 对 `docs/02`、`docs/05`、`docs/06`、`docs/08`、`docs/09`、`docs/11` 运行 `rg -n "完整进程树|GPU|Phase 0|磁盘预算"` 命中新增约束；对 `docs/01` 至 `docs/11` 运行 `rg -n "Tauri 作为局部替代"` 无命中。

- [ ] 写预算和 fallback 检索断言并确认 RED。
- [ ] 运行断言，记录当前文档不一致。
- [ ] 最小修改六份权威文档。
- [ ] 重跑断言，确认 GREEN。
- [ ] 审阅差异，不在本任务自动提交。

## Task 3：同步规格、实施计划与基线

**Files:** `docs/aegis/specs/2026-08-11-novahub-design.md`、`docs/aegis/specs/2026-08-13-v1-popular-tools-design.md`、`docs/aegis/plans/2026-08-13-v1-design-and-implementation.md`、`docs/aegis/plans/2026-08-14-desktop-pet-plugin-implementation.md`、`docs/aegis/baseline/2026-08-14-architecture-hardening-baseline.md`、`docs/aegis/INDEX.md`。

**Why:** 防止权威架构、已确认规格和可执行计划出现不同验收口径。

**Impact/Compatibility:** 原始初始基线保留；新增加固基线记录已采纳决策。

**Verification:** 检查 Task 2/4/5/8、宠物 Task 5 与新基线均包含对应验收项。

- [ ] 写规格和计划一致性断言并确认 RED。
- [ ] 运行断言，定位遗漏任务。
- [ ] 同步规格、计划、基线和索引。
- [ ] 重跑断言，确认 GREEN。
- [ ] 审阅差异，不在本任务自动提交。

## Task 4：全量文档验证

**Files:** 本计划列出的全部 Markdown。

**Why:** 发现残留冲突、错误路径、乱码和空白问题。

**Impact/Compatibility:** 只验证，不修改评审原文。

**Verification:** `git diff --check` 成功；UTF-8 无 BOM；关键检索断言全部成功。

- [ ] 运行跨文档约束脚本。
- [ ] 检查旧 fallback 仅存在于评审历史引用中。
- [ ] 检查 UTF-8 BOM 与乱码标记。
- [ ] 运行 `git diff --check` 并确认 GREEN。
- [ ] 汇总已覆盖范围、未验证实现和残余风险。

## Repair Track

- 根因：架构目标正确，但资源预算只覆盖无 Host 主进程，路线图仍保留不精确的 Tauri fallback，部分评审动作未进入实施任务。
- 权威所有者：系统边界由 `docs/03`，插件契约由 `docs/04`，发布验收由 `docs/08`，V1 执行由现有规格和计划拥有。
- 最小修复：只补充约束、测试场景和退出条件。

## Retirement Track

- 退休对象：“Tauri 作为 Slint 无障碍局部替代”的旧 fallback。
- 保留边界：复杂编辑器未来仍可在真实需求和独立资源预算成立后评审受限 WebView Host，但不得替换核心 Shell 或改变 WIT View 所有权。

## Risks

- 当前没有生产实现，所有资源目标仍是假设。
- Slint/Wasmtime 的具体依赖版本必须由 Task 1/2 创建工作区时按当时稳定版本锁定，本文不虚构版本号。
- 原生无障碍桥失败时必须重新进行架构评审，不能静默降低无障碍验收。

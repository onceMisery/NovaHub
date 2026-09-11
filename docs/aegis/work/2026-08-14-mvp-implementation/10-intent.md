# NovaHub MVP 实施意图

## Requested Outcome

按已确认的 NovaHub 架构方案交付可编译、可测试、可逐步运行的 MVP：Rust + Slint 宿主、内置能力、按需 Plugin Host、WIT/WASM 插件、默认删除和 Windows/macOS 资源/无障碍验收路径。

## Current Scope

当前从 V1 实施计划 Task 1 开始，优先建立 Rust workspace、领域契约、架构边界测试和 CI 门槛，然后按 Task 2 至 Task 12 推进。现有 HTML 原型只作为设计验收输入，不作为生产实现。

## Non-goals

- 不引入 Tauri/WebView 主 Shell、JavaScript 插件运行时或竞品插件兼容运行时。
- 不实现云同步、账号、市场、任意脚本、常驻第三方插件或 Linux。
- 不覆盖用户已有的文档修改、评审删除和未相关的工作区文件。

## Baseline Refs

- `docs/03-system-architecture.md`
- `docs/04-plugin-platform.md`
- `docs/05-ui-ux-design.md`
- `docs/08-quality-release.md`
- `docs/aegis/baseline/2026-08-14-architecture-hardening-baseline.md`
- `docs/aegis/plans/2026-08-13-v1-design-and-implementation.md`

## Impact

会新增 Rust workspace、应用/Host、crates、WIT、Protobuf、测试和性能基准；契约所有权保持 WIT、宿主 UI、Plugin Host、storage、platform 的单一所有者，不新增隐式 fallback。

## Initial Risks

- 绿地仓库没有现成 Rust 实现，依赖版本和 Slint/Wasmtime API 需要以实际编译结果为准。
- Windows/macOS 平台 API 不能在当前单平台环境完全验证，跨平台测试需要后续实机或 CI 证据。
- MVP 范围较大，必须每个切片保持可编译和可回滚，不能以文档完成替代运行证据。

# TaskIntentDraft：NovaHub V1 确定性原型扩展

## 请求结果

执行 `docs/aegis/plans/2026-08-13-v1-design-and-implementation.md` 的 Task 3，扩展本地高保真原型以覆盖 V1 热门能力与现代化体验。

## 范围

- 保留现有 L/P/M 10 个确定性状态。
- 新增 C/B/F/Q/W/D/T/G/R 画板族与可达交互。
- 更新组件页、README、Figma 同步清单和自动化验证。
- 生成桌面截图并验证深浅主题、键盘、缩放、减少动画与溢出。

## 非目标

- 不创建 Rust 工作区，不实现 Slint、WIT、Provider 或插件运行时。
- 不直接运行或兼容 uTools/Raycast 插件。
- 不提交 Git，不覆盖既有设计资产。

## BaselineReadSetHint

- `docs/05-ui-ux-design.md`
- `docs/10-figma-prototype.md`
- `docs/11-v1-popular-tools-and-modern-experience.md`
- `docs/aegis/specs/2026-08-13-v1-popular-tools-design.md`
- `docs/aegis/plans/2026-08-13-v1-design-and-implementation.md`
- `prototype/index.html`、`styles.css`、`app.js`、`components.html`、`figma-sync-manifest.json`

## ImpactStatementDraft

原型路由、视觉组件、交互状态、同步清单和测试证据发生扩展。生产架构、协议所有权、数据模型和兼容边界不变。现有路由必须作为回归基线继续工作。

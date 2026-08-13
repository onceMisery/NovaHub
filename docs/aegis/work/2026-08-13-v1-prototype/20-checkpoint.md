# TodoCheckpointDraft

## 当前状态

- 当前任务：完成 NovaHub V1 热门工具确定性原型与设计实施交付。
- 活跃切片：完成候选验证与交付记录整理。
- 已完成：35 个 V1 画板、10 个旧版兼容画板、Command Center 键盘链路、Translation 新路由、双主题、减少动态效果、真实 200% 字号、Figma 同步清单与画板截图。
- 本地待完成：无。
- 外部阻塞：Figma Starter MCP 调用额度耗尽，远端 `01 Components` 与 `02 Prototype` 尚未同步。
- 后续：配额恢复后按 `prototype/figma-sync-manifest.json` 同步，不重新设计；生产代码实现按 `docs/aegis/plans/2026-08-13-v1-design-and-implementation.md` 另行推进。

## ResumeStateHint

本地源真值为 `prototype/`、`tests/prototype/`、`work/board-*.png` 和 `prototype/figma-sync-manifest.json`。恢复 Figma 同步时先读取 manifest，保持三页 Starter 文件结构，不修改已验收的路由、画板 ID 和可访问性规则。

## DriftCheckDraft

- 仍服务原始意图：是，覆盖现代化 NovaHub 原型、uTools/Raycast 热门能力和实施计划。
- 兼容边界：只迁移公开配置、Quicklinks、Snippets 与插件清单，不执行竞品插件或源脚本。
- 新增所有者/后备路径：无。
- 退休轨：旧 L/P/M 路由仅作为明确兼容回归保留；V1 主路径由 C/B/F/Q/W/D/T/G/R 路由拥有。
- 决策：本地完成候选；Figma 远端同步保持 external-blocked。

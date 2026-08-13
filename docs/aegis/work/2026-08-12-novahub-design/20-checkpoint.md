# Todo Checkpoint

## Completed

- 产品定位、MVP、架构、插件平台、安全、数据、质量与发布方案
- Phase 0–4 路线图、项目展望、最终形态和退出条件
- NovaHub “三星汇聚成 Nova”品牌资产与平台导出规范
- Figma 文件创建与计划归属：`NovaHub Desktop Prototype` / `WyTYbVDv3Ws0hAxEl8kytl`
- Figma 三页骨架：`00 Foundations`、`01 Components`、`02 Prototype`
- Figma Foundations 文档根节点 `7:2`
- 本地高保真交互原型、15 类官方组件验收页和 10 个确定性画板路由
- Figma 同步 manifest、13 张视觉证据、浏览器状态/尺寸/资源/溢出验证
- UTF-8 无 BOM、Markdown 链接、旧名称、SVG XML、JavaScript 和 whitespace 检查

## Active Slice

将已验证的品牌、组件和 10 个核心画板同步到现有 Figma `01 Components` 与 `02 Prototype` 页面，并执行 Figma 截图审计。

## Blocked On

- Figma Starter MCP 调用额度已耗尽。
- 当前会话的工具注册表未暴露任何 Figma MCP 工具。

两项均属于外部工具状态。本地同步输入已经固定，不需要重新设计或扩展产品范围。

## ResumeStateHint

1. 读取 `work/figma-state-novahub-20260812.json`。
2. 读取 `prototype/figma-sync-manifest.json`。
3. 加载 `figma-use`、`figma-generate-design`、`figma-generate-library`。
4. 先只读扫描 fileKey `WyTYbVDv3Ws0hAxEl8kytl`，确认页面和节点未被用户修改。
5. 同步 `assets/brand/novahub-icon-specimen.svg` → `00 Foundations / Brand`。
6. 同步 `prototype/components.html` → `01 Components`。
7. 按 manifest 串行同步 10 个画板 → `02 Prototype`，再建立 9 条流程连线。
8. 逐页运行 metadata 与 screenshot 审计，并回写所有节点 ID。

## DriftCheckDraft

- 范围：仍服务完整方案、路线图、品牌和原型目标
- 兼容：HTML 原型仅作为设计源真值，生产技术仍为 Rust + Slint
- 所有权：WIT、宿主 UI、Plugin Host、storage 和 platform 所有权未变化
- 新 fallback：无；本地交付包是同步输入，不是第二套生产 UI
- 决策：blocked；等待 Figma MCP 工具重新暴露且 Starter 调用额度恢复

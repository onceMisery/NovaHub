# Evidence Bundle Draft

## Files

- `docs/01-product-and-principles.md` 至 `docs/10-figma-prototype.md`
- `CONTEXT.md`
- `docs/aegis/specs/2026-08-11-novahub-design.md`
- `docs/aegis/baseline/2026-08-12-initial-baseline.md`

## Checks

- 所有 Markdown 文件 UTF-8 无 BOM：通过
- `git diff --check`：通过
- Markdown 相对链接扫描：通过
- 旧产品名扫描：通过

## Remaining Evidence

- Figma 文件 URL：`https://www.figma.com/design/WyTYbVDv3Ws0hAxEl8kytl`
- Figma 页面：`00 Foundations` (`0:1`)、`01 Components` (`6:4`)、`02 Prototype` (`6:5`)
- Foundations 根节点：`7:2`
- 可运行原型：`prototype/index.html`、`prototype/styles.css`、`prototype/app.js`
- 品牌图标：`assets/brand/novahub-app-icon.svg`、`novahub-mark.svg`、`novahub-symbol-mono.svg`
- 浏览器验收：启动器 `720×496`、翻译窗 `760×568`、插件管理 `960×680`；Action Panel、错误/结果、卸载与 Toast 流程通过；DOM 溢出 0，控制台错误 0
- 视觉截图：`work/prototype-*.png`、`work/brand-prototype-*.png`、`work/novahub-icon-specimen.png`
- 组件验收页：`prototype/components.html`；5 个分区、10 个组件族、15 类官方组件，无组件内部 overflow
- 确定性画板：`prototype/figma-sync-manifest.json`；L1–L4、P1–P3、M1–M3 共 10 个 URL 均通过状态与尺寸验证
- 画板截图：`work/board-L1.png` 至 `work/board-M3.png`；交付总览 `work/novahub-prototype-delivery.png`
- Figma handoff：`work/novahub-figma-handoff-20260813-v2.zip`，29 个条目，SHA-256 `D8F893D93A9D5EC8662C378EE77857CA7024FC87FA10FA5F489225E7F5ADFD05`

## External Limitation

Figma Starter MCP 调用额度已耗尽，组件页、Prototype 页和品牌图标尚未同步到 Figma。该项保持未完成，不作为完整 Figma 原型通过的证据。
后续会话还需要 Figma MCP 工具重新暴露；同步输入已固定，不需要重新设计。

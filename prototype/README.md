# NovaHub 原型捕获说明

本目录是 NovaHub 的高保真交互源真值，也是在 Figma Starter MCP 配额恢复后同步 `01 Components` 与 `02 Prototype` 的确定性输入。当前同时保留 10 个旧版兼容画板，并提供覆盖 V1 14 项能力的 35 个现代化状态画板。

## 运行

从仓库根目录启动静态服务器：

```powershell
python -m http.server 4173 --directory .
```

入口：

- 交互原型：`http://127.0.0.1:4173/prototype/`
- 组件验收：`http://127.0.0.1:4173/prototype/components.html`

## 确定性画板路由

| 画板 | URL | 目标窗口 |
|---|---|---|
| L1 空查询 | `?view=launcher&state=idle` | 720 × 496 |
| L2 搜索结果 | `?view=launcher&state=results&query=code` | 720 × 496 |
| L3 Action Panel | `?view=launcher&state=actions&query=code` | 720 × 496 |
| L4 深色启动器 | `?view=launcher&state=actions&query=code&theme=dark` | 720 × 496 |
| P1 翻译表单 | `?view=translator&state=form` | 760 × 568 |
| P2 翻译结果 | `?view=translator&state=result` | 760 × 568 |
| P3 请求错误 | `?view=translator&state=error` | 760 × 568 |
| M1 插件列表 | `?view=plugins&state=list` | 960 × 680 |
| M2 插件详情 | `?view=plugins&state=detail` | 960 × 680 |
| M3 卸载确认 | `?view=plugins&state=uninstall` | 960 × 680 |

每个 URL 直接进入指定状态，不依赖计时、剪贴板权限或前一页面操作，因此适合自动截图和 Figma 页面捕获。

## V1 现代化画板

| 分区 | 确定性状态 | 目标窗口 |
|---|---|---:|
| Command Center | `?view=command&state=idle`、`results`、`preview`、`actions`、`partial` | 760 × 560 |
| Clipboard | `?view=clipboard&state=history`、`preview`、`sensitive`、`clear` | 840 × 640 |
| File Search | `?view=files&state=results`、`filters`、`preview`、`offline` | 840 × 640 |
| Quicklinks & Snippets | `?view=library&state=list`、`parameters`、`edit` | 840 × 640 |
| Window Manager | `?view=windows&state=layouts`、`displays`、`permission` | 760 × 560 |
| Developer Toolkit | `?view=developer&state=json`、`json-error`、`encoding`、`color`、`qr` | 880 × 640 |
| Translation | `?view=translation&state=input`、`result`、`error` | 760 × 568 |
| Gallery | `?view=gallery&state=discover`、`detail`、`permissions`、`uninstall` | 960 × 680 |
| Migration | `?view=migration&state=source`、`scan`、`review`、`result` | 960 × 680 |

URL 格式为 `?view=<分区>&state=<状态>`。可追加 `theme=dark` 验证深色主题，追加 `scale=200` 验证 200% 文本状态。

## Figma 同步结构

Starter 文件限制为三个页面：

```text
00 Foundations
01 Components
02 Prototype
```

- `assets/brand/novahub-icon-specimen.svg` → `00 Foundations / Brand`
- `prototype/components.html` → `01 Components`
- 上表 10 个旧版兼容画板和 35 个 V1 画板 → `02 Prototype`
- 画板间连线按 `docs/10-figma-prototype.md` 的 L/P/M 流程建立
- `figma-sync-manifest.json` 是机器可读同步清单，包含现有 Figma 页面 ID、画板 URL、尺寸、目标分区、证据截图和流程连线。

## 验收边界

- HTML/CSS/JS 只用于设计验证，不是 NovaHub 的生产 UI 技术选型。
- 生产实现仍为 Rust + Slint，插件返回官方声明式 View 模型。
- 字体在 Web 原型中使用平台系统字体；Figma 无法稳定使用平台字体时使用 Inter。
- 不创建 Code Connect，直到 Slint 组件源码真实存在。

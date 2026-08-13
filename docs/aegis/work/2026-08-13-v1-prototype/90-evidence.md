# EvidenceBundleDraft

## 自动化证据

- 浏览器验收命令：设置桌面依赖 `NODE_PATH` 与 Chrome 路径后，运行 `node tests/prototype/run.cjs`。
- 最新结果：退出码 0，`{"boards":35,"errors":[]}`。
- 覆盖范围：35 个 V1 确定性路由、4 个旧版兼容入口、Command Center 上下选择/动作面板/Escape、dark、reduced-motion、200% 正文字号、列表图标间距、内部水平与垂直溢出、控制台与页面错误。
- Manifest：`prototype/figma-sync-manifest.json` schemaVersion 2，登记 35 个 V1 画板及对应 evidence 路径。
- 截图：`work/board-C1.png` 至 `work/board-R4.png` 共 35 张；200% 暗色人工复核图为 `work/v1-R3-dark-200.png`。

## 调试证据

- 启动顺序：将 `applyPrototypeRoute()` 移至 V1 常量和渲染器初始化之后，消除 C1 的 TDZ 启动失败。
- 路由映射：Command Center 元数据统一使用 `idle/results/preview/actions/partial` 状态键。
- Translation：注册 `translation/input|result|error`，生成 T1–T3，同时保留旧 `translator/*` 回归。
- 焦点所有权：Command Center 局部重渲染后重新聚焦新的 `#v1CommandSearch`，键盘链路持续可用。
- 真实 200% 缩放：字号由 `.text-scale-200` 规则控制；根窗口网格行允许 92px header 与 58px footer 自然展开，48px 图标列同步扩展并保留至少 8px 文本间距。

## 未覆盖与外部边界

- Figma Starter MCP 配额耗尽，无法在本轮证明远端三页文件已同步。
- 本轮交付是设计原型与实施计划，不包含 Rust/Slint/WIT 生产实现。

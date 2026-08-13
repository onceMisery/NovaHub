# NovaHub Figma 原型规格与交付说明

## 1. 目标

原型用于验证 NovaHub 的启动器交互、官方插件 View 和插件管理生命周期，并作为后续 Slint 组件实现的视觉基线。它不是营销展示稿，也不引入文档之外的新产品能力。

## 2. Figma 文件结构

```text
00 Foundations
01 Components
02 Launcher
03 Plugin View
04 Plugin Manager
05 Flows & Notes
```

### 00 Foundations

- 品牌图标：App Icon、透明品牌标记和单色系统标记，统一采用“三星汇聚成 Nova”几何
- 语义颜色变量：浅色/深色两种 Mode
- spacing：4/8/12/16/24/32
- radius：4/6/8
- icon size：16/20/24/32
- typography：Caption 12、Label 13、Body 14、Title 16、Heading 20
- elevation：Launcher、Popover 两级轻阴影

### 01 Components

最小组件集：Search Field、Result Row、Section Label、Key Hint、Icon Button、Text Button、Primary Button、Danger Button、Toast、Navigation Header、Action Panel Row、Form Field、Toggle、Permission Row、Plugin List Item、Dialog。

重复元素必须是组件或实例，不能复制成无关联图层。组件状态至少覆盖 default、hover、selected/focused、disabled、loading 和 error 中实际适用的状态。

## 3. 启动器画板

### L1：空查询

- 画板背景模拟桌面，但不使用渐变或装饰光斑
- 720 × 496 启动器窗口，搜索框聚焦
- 最近使用和固定项共 5–6 行
- 底部展示 `↑↓ 选择`、`↵ 打开`、`Ctrl/Cmd K 操作`

### L2：搜索结果

- 查询词“code”
- 应用、文件和命令混合排序
- 第一项选中，右侧展示主动作快捷键
- 一项展示次要状态，例如“插件”或“文件”

### L3：动作面板

- 基于 L2 叠加右侧 Action Panel
- 包含打开、打开所在位置、复制路径和危险动作分区
- 键盘焦点与主列表选择保持可辨识

交互：L1 输入 → L2；L2 `Ctrl/Cmd K` → L3；L3 Escape → L2；L2 Escape → L1/隐藏说明。

## 4. 插件官方视图画板

### P1：翻译表单

- Navigation Header：返回、插件图标、“翻译”标题
- FormView：源语言、目标语言、可见标签的多行输入
- 主动作“翻译”，次动作“清空”
- 底部权限状态说明只展示域名范围，不展示技术实现文字

### P2：翻译结果

- 保留输入摘要，显示翻译文本与复制动作
- 右侧或底部 Action Panel 与启动器语义一致
- 展示成功 Toast，不抢占焦点

### P3：插件错误

- 标准错误 View：原因“请求超时”、恢复动作“重试”、次动作“返回”
- 不暴露堆栈、WIT 或 Wasmtime 内部术语

交互：P1 提交 → P2；P2 复制 → Toast；错误路径 → P3；P3 重试 → P1 loading/P2。

## 5. 插件管理画板

### M1：已安装列表

- 960 × 680 设置窗口
- 左侧导航，插件项选中
- 右侧插件列表显示图标、名称、版本、命令数量、启用状态
- 顶部搜索和“从本地安装”命令

### M2：插件详情

- 插件名称、发布者、版本、命令列表
- 权限按网络、剪贴板、文件分组
- 禁用与卸载清晰分离，卸载为危险动作
- 不使用嵌套卡片；详情按全宽分隔段落组织

### M3：卸载确认

- 明确插件名称和删除范围：代码、设置、缓存、凭据、权限
- 明确外部文件和远端数据不在删除范围
- 主按钮“卸载并删除数据”，次按钮“取消”

交互：M1 选插件 → M2；M2 卸载 → M3；M3 取消 → M2；确认 → M1 + 成功 Toast。

## 6. 原型内容语言

原型默认使用简体中文，并在快捷键中根据画板注释同时说明 Windows/macOS 修饰键差异。所有文字使用 NovaHub，不出现历史名。

## 7. 视觉验收

- 无文字截断、重叠、零宽文本或越界
- 启动器、插件 View 和设置窗口的尺寸与规范一致
- 浅色模式至少完整覆盖三个主流程；深色模式覆盖启动器关键画板
- 主/次/危险动作层级清晰，颜色不是唯一状态表达
- 组件实例可追溯到 Foundations/Components
- 字体使用 Figma 可用的系统近似字体；若跨平台文件无法稳定使用 Segoe UI/SF Pro，使用 Inter 作为原型替代并在说明中标注，代码实现仍采用系统字体
- 完成后分别截图 Launcher、Plugin View、Plugin Manager，检查信息层级和可读性

## 8. 实现映射

| Figma | Slint/协议 |
|---|---|
| Search Field | 宿主 SearchInput |
| Result Row | 宿主 SearchResultRow |
| Form View | `form-view` WIT 类型 |
| Action Panel | `action-panel` 公共结构 |
| Permission Row | Plugin Manager capability 展示 |
| Dialog | 宿主确认语义，不允许插件自定义 |

Figma 组件名与未来 Slint 组件名保持语义一致，但不创建 Code Connect 映射，直到实际组件源码存在。

## 9. 当前设计资产与 Starter 限制

- Figma 文件：[NovaHub Desktop Prototype](https://www.figma.com/design/WyTYbVDv3Ws0hAxEl8kytl)
- Starter 计划最多创建 3 个页面，实际结构压缩为 `00 Foundations`、`01 Components`、`02 Prototype`；原计划的 Launcher、Plugin View、Plugin Manager 与 Flows 在 `02 Prototype` 中作为独立分区，不削减状态。
- `00 Foundations` 已创建，根节点为 `7:2`；页面包含 Light/Dark 语义色、字体、间距、圆角与浮层阴影文档。
- 当前 Figma 变量 API 不允许创建变量，且 Starter MCP 调用额度已耗尽；因此变量绑定、组件页和 Prototype 页仍待配额恢复后同步。
- 可运行的高保真交互原型位于 `prototype/`，是配额恢复前的视觉与交互源真值。它覆盖启动器空查询/结果/Action Panel、翻译表单/结果/错误、插件列表/详情/禁用/卸载确认和深色启动器。
- 组件验收页位于 `prototype/components.html`，覆盖规格中的 15 类官方组件与实际适用状态，可直接捕获到 Figma 的 `01 Components` 页面。
- `prototype/README.md` 定义 10 个确定性状态 URL；每个 URL 可直接进入指定画板，无需依赖前序交互或计时，作为 `02 Prototype` 的同步契约。
- `prototype/figma-sync-manifest.json` 固定 Figma 页面 ID、画板名称、URL、尺寸、目标分区、截图证据和交互连线，供工具恢复后自动同步。
- 品牌图标位于 `assets/brand/`，采用“三星汇聚成 Nova”的统一几何；App Icon、透明标记和单色系统标记已在本地原型中验证，待同步到 Figma Foundations。

上述限制是设计工具套餐能力，不改变产品技术方案、原型范围或 Slint 实现边界。

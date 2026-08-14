# NovaHub MVP 桌面宠物插件设计规格

## 状态

本规格已由用户确认。MVP 采用桌面边缘常驻方案、声明式宠物插件、可配置的图标化边缘唤回手柄、右键显示/隐藏，以及宿主管理的 Pet Action Shelf。本文是实施计划的权威输入，不表示功能已经实现。

## Goal

为 NovaHub MVP 增加可日常使用、可扩展且安全隔离的桌面宠物能力。宠物能够在桌面边缘显示、隐藏、拖动和响应状态，并通过快捷动作调用 NovaHub 能力；用户可以安装或制作自己的宠物插件，且不会因此获得任意窗口、脚本、输入监听或后台常驻能力。

## Approved Decisions

- 主形态为桌面边缘常驻宠物，不依附 Command Center。
- 显示和隐藏是宿主设置与右键菜单动作，不在桌面永久展示文字按钮。
- 隐藏后可以保留图标化边缘唤回手柄；手柄、快捷键均可关闭或修改。
- 插件形式复用 `.novahub-plugin`，新增 `kind = "desktop-pet"`。
- MVP 使用声明式资源和状态机，不允许宠物插件自定义窗口或常驻逻辑。
- 宠物可以建议快捷动作，用户可以固定最多 5 个动作，权限按目标能力独立校验。
- 首发内置 Waterman 水人、Nova 星灵和 Pixel 小方。
- 同时只激活一个宠物。

## Authority Refs

- `docs/02-mvp-requirements.md`：MVP 工作流、性能与稳定性门槛。
- `docs/03-system-architecture.md`：进程、模块和平台所有权。
- `docs/04-plugin-platform.md`：插件包、WIT、权限、安装和卸载。
- `docs/05-ui-ux-design.md`：视觉令牌、交互、键盘与无障碍。
- `docs/07-security-privacy.md`：不可信资源、权限和本地数据边界。
- `docs/08-quality-release.md`：测试、性能与发布门槛。

## Scope

MVP 包含：

- 单个活动宠物的桌面透明浮层。
- 显示、隐藏、拖动、边缘吸附、多显示器位置记忆和点击穿透。
- 图标化边缘唤回手柄、右键管理菜单和可配置全局快捷键。
- Pet Action Shelf，最多 5 个用户固定动作。
- 声明式宠物包安装、预览、启用、切换、禁用和卸载。
- 三个内置宠物和用户本地导入流程。
- 深浅主题、高对比度、减少动画、200% 文本和跨 DPI 支持。

## Non-goals

- 聊天对话、AI 推理、养成数值、云端同步或账号体系。
- 自动读取屏幕内容、窗口标题、键盘输入或鼠标轨迹。
- 任意脚本、进程启动、WebView、原始 Socket 或后台定时器。
- 多宠物同时运行、物理碰撞模拟、跨设备状态同步。
- 让宠物插件直接创建窗口、控制 z-order 或注册全局快捷键。

## Architecture

### Host Components

`PetController` 是桌面宠物状态的唯一所有者，位于 `novahub-app`：

- 管理当前宠物、可见性、暂停状态和用户配置。
- 协调 `PetOverlay`、`PetRenderer`、`PetActionResolver` 与平台适配器。
- 将声明式状态事件映射为资源和动画，不依赖插件后台进程。
- 在动作执行、插件卸载、显示器变化和应用退出时执行恢复策略。

`PetOverlay` 由平台层实现透明无边框浮层：

- Windows 使用受控的 Win32 窗口样式、DPI 与工作区 API。
- macOS 使用受控的 NSPanel/NSWindow 行为、屏幕与辅助功能语义。
- 平台层只接收宿主领域模型，不解析插件包或动作目标。

`PetRenderer` 使用 Slint 渲染当前关键帧、过渡和 Action Shelf。插件不能提交任意 Slint、HTML、SVG 脚本或坐标树。

`PetActionResolver` 将快捷动作解析到 NovaHub canonical command ID，并复用既有命令注册表、权限裁决与执行管线。

### Plugin Host Boundary

纯声明式宠物不启动 `novahub-plugin-host`。若宠物包同时声明普通插件命令，只有用户执行该命令时才按现有 WIT 会话启动 Plugin Host。宠物可见、待机动画和宿主状态反馈不构成插件后台运行。

## Ownership Map

| 对象 | 唯一所有者 | 插件可提供 | 插件不可控制 |
|---|---|---|---|
| 可见性与活动宠物 | `PetController` | 默认建议 | 强制显示、隐藏或切换 |
| 浮层窗口与位置 | `platform/*` + `PetController` | 推荐尺寸和锚点 | 任意坐标、z-order、窗口样式 |
| 动画播放 | `PetRenderer` | 声明式状态资源与时间线 | 自定义渲染代码、后台循环 |
| 快捷动作固定项 | 用户设置 + `PetActionResolver` | 最多 3 个推荐动作 | 强制固定、绕过权限 |
| 宠物包生命周期 | Plugin Manager | 清单、资源、可选命令 | 自删除、自更新、保留卸载数据 |
| 全局快捷键与边缘手柄 | 宿主设置 | 无 | 注册或覆盖快捷键 |

## Package Contract

包继续使用 `.novahub-plugin` 归档和现有签名、哈希、Zip 安全与原子安装流程。

```text
com.novahub.waterman-1.0.0.novahub-plugin
├── novahub.toml
├── pet.json
├── assets/
│   ├── idle.webp
│   ├── interact.webp
│   ├── working.webp
│   ├── success.webp
│   └── error.webp
├── LICENSE
└── signature.ed25519
```

清单增量：

```toml
manifest_version = 1
id = "com.novahub.waterman"
name = "Waterman"
version = "1.0.0"
plugin_api = ">=1.1, <2.0"
kind = "desktop-pet"
platforms = ["windows", "macos"]

[pet]
descriptor = "pet.json"
```

纯声明式包省略 `entry`。若同一包提供普通命令，可以声明 `entry = "plugin.wasm"` 和 `[[commands]]`；WASM 仍不能修改宠物窗口，只能通过现有命令会话返回官方 View 或动作结果。

`pet.json` 示例：

```json
{
  "schemaVersion": 1,
  "petId": "waterman",
  "displayName": "Waterman 水人",
  "preferredSize": { "width": 160, "height": 192 },
  "anchor": { "x": 0.5, "y": 1.0 },
  "states": {
    "idle": { "asset": "assets/idle.webp", "fps": 12, "loop": true },
    "interact": { "asset": "assets/interact.webp", "fps": 18, "loop": false },
    "working": { "asset": "assets/working.webp", "fps": 18, "loop": true },
    "success": { "asset": "assets/success.webp", "fps": 18, "loop": false },
    "error": { "asset": "assets/error.webp", "fps": 18, "loop": false }
  },
  "recommendedActions": [
    { "id": "command-center", "target": "builtin/command-center" },
    { "id": "clipboard", "target": "builtin/clipboard-history" }
  ]
}
```

所有路径相对于包根目录，禁止绝对路径、父级跳转、符号链接逃逸和重复文件覆盖。显示名称、描述和动作标题经过长度与控制字符校验。

## State Model

宿主拥有确定性状态机：

```text
hidden -> entering -> idle
idle -> interact -> idle
idle -> working -> success | error -> idle
idle -> sleeping -> idle
any -> suspended | hidden
suspended -> idle | hidden
```

- `hidden`：浮层不渲染；可选边缘手柄仍由宿主显示。
- `entering`：宿主入场过渡；减少动画时直接进入 `idle`。
- `interact`：单击或宿主认可的互动事件。
- `working`：从 Pet Action Shelf 启动动作后等待结果。
- `success` / `error`：宿主根据动作结果触发，播放一次后回到 `idle`。
- `sleeping`：长时间无互动时的低帧状态，不唤醒 Plugin Host。
- `suspended`：用户暂停、系统省电、全屏应用策略或资源压力触发。

缺少非必需状态资源时回退 `idle` 静态关键帧；缺少 `idle` 或无法解码核心资源时拒绝安装。

## Pet Action Shelf

单击宠物打开宿主渲染的紧凑动作栏。首屏最多 5 个固定项，支持方向键、数字键、Enter 与 Escape；更多动作进入标准 Action Panel。

允许的目标类型：

- NovaHub 内置 canonical command ID。
- 用户已有的 Quicklink 或 Snippet ID。
- 已安装插件的 canonical command ID。
- 宠物切换与宠物设置等宿主动作。

宠物包最多建议 3 个动作。安装过程只展示建议，不自动固定。用户固定项存储在宿主设置命名空间，不写入宠物插件存储。

动作执行流程：

```text
用户选择动作
-> PetActionResolver 解析目标
-> 检查目标存在、启用状态与权限
-> 通过既有命令执行管线调用
-> PetController 接收 working/success/error 事件
-> PetRenderer 更新声明式状态
```

宠物不继承目标的文件、剪贴板、网络或凭据权限。目标失效时动作项显示不可用原因，并提供“编辑快捷动作”恢复入口。

## Visibility And Interaction

- 右键宠物打开宿主管理菜单：打开快捷动作、切换宠物、宠物设置、暂停互动、隐藏宠物。
- 桌面不永久显示“显示/隐藏”文字。
- 可选边缘唤回手柄只显示熟悉的图标；Tooltip 和无障碍名称为“显示 <宠物名>”。
- 设置允许关闭边缘手柄、修改或禁用全局快捷键、选择启动时是否显示。
- 手柄关闭后始终保留系统托盘和设置页恢复路径。
- 点击穿透启用后宠物不接收指针事件，恢复路径同上。
- 拖动结束后吸附最近工作区边缘，并按显示器 ID 和逻辑坐标记忆位置。
- 显示器移除、分辨率或 DPI 改变后将宠物约束回当前可用工作区。
- 宠物默认不抢键盘焦点；打开 Action Shelf 后才建立可访问焦点域，Escape 恢复到不抢焦点状态。

## Settings

设置页新增“桌面宠物”分区：

- 当前宠物与已安装宠物选择。
- 大小、透明度、显示器和位置重置。
- 始终置顶、点击穿透、声音、动画强度。
- 显示边缘唤回手柄。
- 全局显示/隐藏快捷键。
- 启动 NovaHub 时显示。
- 暂停互动与恢复默认设置。
- 导入本地宠物包与打开插件详情。

配置由宿主持久化。宠物插件只能读取宿主传入的渲染上下文，不直接访问其他宠物或全局设置。

## Built-in Pets

| 宠物 | 资源目的 | 默认气质 | 验证重点 |
|---|---|---|---|
| Waterman 水人 | 用户提供的 `waterman.jpg` 衍生资源 | 专注、平静 | 位图裁切、深色融合、状态过渡 |
| Nova 星灵 | NovaHub 原创矢量/帧资源 | 品牌默认、轻快 | 主题适配、降级与卸载回退 |
| Pixel 小方 | 原创像素帧动画 | 工具感、活跃 | 帧动画、颜色配置和性能上限 |

`waterman.jpg` 是已确认的设计输入。正式分发前必须建立可分发版权或许可证记录；若无法确认，则保留 Waterman 交互设计并替换为 NovaHub 原创视觉资产。实施目标路径为 `assets/pets/builtin/waterman/`，不引用用户图片目录的绝对路径。

## Resource And Performance Budgets

- 同时只激活一个宠物。
- 宠物包解压后最大 25 MiB。
- 单张图片最大 4096 x 4096，解码后像素和字节预算由宿主再次限制。
- 单个动画状态最多 240 帧，默认不超过 30 FPS。
- 闲置建议 12 FPS；系统省电或长时间空闲时降至静态或 6 FPS。
- 宠物功能相对关闭状态的增量 RSS P95 不超过 20 MiB。
- 宠物 idle CPU P95 低于单逻辑核心 1%。
- Action Shelf 打开反馈不晚于 100 ms。
- 声音默认关闭；单个音频资源最长 10 秒，并限制格式、采样率和总字节数。

预算必须在 Windows 和 macOS 参考机实测。无法达到预算时优先降帧、降低解码尺寸和静态化，不启动额外进程规避测量。

## Security And Privacy

- 宠物资源与所有第三方插件资源一样不可信，安装前执行归档、路径、压缩比、解码器与资源预算校验。
- SVG 仅允许宿主安全子集或在构建/安装时栅格化；禁止脚本、外部引用、字体和滤镜逃逸。
- 声音必须由用户启用；宠物不能访问麦克风、摄像头或系统音频。
- 宠物不能读取屏幕内容、活动窗口标题、输入事件、剪贴板历史或文件路径。
- 右键菜单、边缘手柄、Action Shelf 和设置页均由宿主绘制，插件不能伪造系统权限提示。
- 未签名宠物只允许开发者模式安装，并持续展示开发状态。
- 卸载删除包、资源缓存、宠物设置、动作建议和插件数据；用户原始导入文件不在删除范围。

## Error Handling

| 异常 | 用户结果 | 恢复策略 |
|---|---|---|
| 安装包或核心资源损坏 | 拒绝安装并指出具体资源 | 保持当前宠物不变 |
| 非核心状态资源缺失 | 使用 `idle` 静态关键帧 | 插件详情显示诊断 |
| 动作目标已卸载/禁用 | 动作置灰并说明原因 | 提供编辑固定动作入口 |
| 动作执行失败 | 播放 `error` 或静态错误状态并显示 Toast | 可重试，不关闭宠物 |
| 当前宠物被卸载 | 切换到 Nova 星灵 | 清理原宠物状态与缓存 |
| 渲染器或资源解码失败 | 隐藏问题资源并保持主程序可用 | 回退 Nova 星灵或静态图 |
| 当前显示器移除 | 移动到主显示器可用区域 | 保存新的有效位置 |
| 所有直接恢复入口关闭 | 系统托盘与设置页仍保留恢复动作 | 不允许删除最后恢复路径 |

## Accessibility And Platform Behavior

- 宠物图像提供宠物名称和当前状态的可访问描述，但 idle 动画不持续播报。
- 边缘手柄是标准按钮，命中区域至少 36 x 36 逻辑像素，具有 Tooltip、名称、焦点环和按下状态。
- Action Shelf 使用标准菜单/列表语义，焦点顺序与视觉顺序一致。
- 颜色不是状态的唯一表达；成功和错误同时使用动作结果、图像状态与可访问通知。
- 200% 文本缩放只放大菜单、Tooltip 和设置文本，不按比例放大宠物位图。
- 减少动画时禁用位移和循环动画，使用静态关键帧与淡入淡出。
- 高对比度模式由宿主重新着色边缘、焦点和菜单表面，不能依赖宠物资源自身对比度。
- Windows/macOS 保持相同领域状态和设置语义，窗口层级、托盘/菜单栏与快捷键表示适配平台习惯。

## Verification Matrix

### Contract And Security

- 清单 `kind`、`pet.json` schema、路径和资源预算单元测试。
- Zip Slip、符号链接、重复文件、压缩炸弹、解码炸弹与畸形图片测试。
- 声明式宠物不启动 Plugin Host 的进程级测试。
- 宠物推荐动作不能自动固定或继承权限的契约测试。
- 卸载默认删除本地拥有数据并保留用户原始包的测试。

### State And Actions

- 完整状态转移、缺失状态回退和减少动画分支测试。
- 内置命令、Quicklink、Snippet、插件命令和失效目标测试。
- working/success/error 结果到宠物状态的确定性映射测试。
- 同时只激活一个宠物和切换事务测试。

### Desktop UX

- 显示/隐藏、右键菜单、图标手柄、快捷键、系统托盘恢复。
- 拖动吸附、多显示器移除、DPI 变化、虚拟桌面和全屏应用。
- 点击穿透开启后的可恢复性。
- Action Shelf 的鼠标、方向键、数字键、Enter、Escape 和屏幕阅读器路径。
- 浅色、深色、高对比度、减少动画与 200% 文本。

### Performance

- 关闭宠物与开启各内置宠物的 RSS、CPU 和 GPU 对比。
- 6/12/18/30 FPS、不同尺寸和最大合法资源的压力测试。
- Action Shelf 首帧延迟和动作执行反馈延迟。
- 长时间 idle、睡眠/唤醒、显示器热插拔和 Plugin Host 崩溃稳定性。

## Acceptance

1. 用户能安装、预览、选择、配置、显示、隐藏和卸载声明式宠物包。
2. Waterman、Nova 星灵和 Pixel 小方通过同一公开契约运行，无内置特权后门。
3. 桌面不永久显示“显示/隐藏”文案；右键菜单负责隐藏，图标化边缘手柄可配置。
4. 关闭边缘手柄和全局快捷键后，系统托盘或设置页仍能恢复宠物。
5. Pet Action Shelf 最多显示 5 个用户固定动作，插件建议不能自动固定或绕过目标权限。
6. 声明式宠物可见和待机期间不启动 Plugin Host，不引入第三方后台常驻。
7. 宠物资源损坏、动作失败、显示器变化、卸载或渲染失败不影响 NovaHub 主能力。
8. Windows 与 macOS 的核心流程、无障碍、主题、减少动画、文本缩放和资源预算通过验收。

## Compatibility And Retirement

- 现有普通插件默认 `kind = "command"`，包解析器必须对未显式声明 `kind` 的旧包保持兼容。
- `desktop-pet` 是新包类型，不改变普通插件 WIT 和官方 View 语义。
- 不新增宠物专用常驻进程、WebView 或脚本运行时。
- 若未来需要可编程宠物行为，必须以真实需求和资源数据证明声明式状态机不足，并通过新的权限与生命周期规格；不得把 MVP 的可选 `plugin.wasm` 命令入口扩展为隐式后台循环。

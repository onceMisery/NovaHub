# NovaHub 初始架构基线

## 项目结构

当前为绿地仓库，已有 README、产品/技术设计文档和 Aegis 设计记录；目标实现结构见 `docs/03-system-architecture.md`。

## 技术栈

Rust、Slint、Tokio、Wasmtime Component Model、WIT、SQLite、Protobuf、nucleo；Windows 与 macOS 原生平台适配。

## 所有权映射

WIT → 插件契约；`novahub-app` → UI/内置能力；Plugin Host → WASM 执行；storage → SQLite；platform crates → OS API；SDK → WIT 薄封装。

## 契约清单

插件包清单、WIT world、官方 View/Event 模型、capability 权限、IPC Protobuf、SQLite schema 和版本范围。

## 依赖方向

稳定领域类型 → 协议/应用服务 → 宿主实现；平台、Slint、Wasmtime 细节不能反向进入核心领域或成为插件契约来源。

## 测试系统

单元、契约、集成、跨平台 UI/无障碍、性能、压力、安全 fuzz、依赖审计和发布安装测试。

## 构建与发布

固定 Rust 工具链，Windows/macOS 签名构建，staging/健康检查/原子切换/回滚更新流程。

## 已知反模式

主进程加载插件动态库、插件任意 DOM、无界缓存、插件常驻后台、静默新增权限、递归全盘扫描 fallback、复制 WIT 类型定义。

## 兼容边界

MVP 仅 Rust/WASM 与官方声明式 UI；不保证外部文件和远端数据随插件卸载删除；未来 SDK 必须适配同一 WIT 语义。

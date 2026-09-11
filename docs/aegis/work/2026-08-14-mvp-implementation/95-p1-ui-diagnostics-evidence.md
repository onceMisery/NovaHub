# P1 UI 与诊断接入证据

更新时间：2026-08-16

## 已验证

1. 插件列表项在宿主快照中最多保留 100 条，Slint Shell 仅绑定 6 个稳定槽位。
2. 上一页、下一页操作只改变宿主偏移量；点击项会使用偏移量恢复完整项 ID。
3. 诊断页面的预览、失败筛选、清空和导出均由主进程处理，插件没有文件系统写入入口。
4. 诊断预览和导出复用 `DiagnosticExportOptions`，事件容量和失败筛选保持统一。
5. 破坏性系统命令通过宿主 ActionPanel 提供 Confirm/Cancel，pending action 不会被失焦或窗口关闭路径绕过。

## 必要验证结果

- `cargo fmt --all -- --check`：通过
- `cargo clippy -p novahub-ui-slint -p novahub-app --all-targets --offline -- -D warnings`：通过
- `cargo test -p novahub-ui-slint -p novahub-app --offline -- --test-threads=1`：通过（App 52、主程序 20、Slint 13）
- `cargo run -p xtask --offline -- release-check`：通过
- `git diff --check`：通过
- UTF-8 无 BOM 检查：通过

## 待补证据

Windows/macOS 参考机上的 RSS、Private Working Set、GPU、冷启动、无障碍以及 Host 崩溃回收测量仍属于发布前验证项。它们不改变 Rust + Slint Shell、WIT 单一 ABI 和按需 sibling Plugin Host 的架构约束。

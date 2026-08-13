# Baseline Governance

## 1. Architecture Defect

确认基线本身存在错误、缺口或矛盾时，先修正基线，再让实现对齐。

## 2. Architecture Drift

实现偏离已确认且正确的基线时，回到基线的最简单路径；不能为了匹配漂移而反向改基线。

## 3. Baseline Check Protocol

非平凡变更前读取最新基线、所有权映射、契约清单、依赖方向和已知反模式，并报告 aligned / minor drift / material drift。

## 4. Architecture Review

每个阶段检查所有权完整性、模块边界、契约变更、级联依赖、依赖方向、旧路径退出和净复杂度。

## 5. Hard Boundaries

- 本文件是本项目 Aegis 工作区的治理约束。
- 基线是证据，不替代用户对设计的确认。
- 设计规格记录方案，不自动授予实现完成资格。

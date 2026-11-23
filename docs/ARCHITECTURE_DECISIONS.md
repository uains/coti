# 架构决策记录

本文件记录会改变 crate 边界、依赖方向、缓存失效或数据模型的架构决策。设计基线仍以 `DESIGN.md` 为准；本文件只记录实施过程中对未定细节的确认，以及必要的偏差、理由和迁移方案。

## ADR-0001：采用 virtual Cargo Workspace，GPUI 延后接入

- **状态：** Accepted
- **日期：** 2026-11-23
- **影响范围：** 根 `Cargo.toml`、`crates/md-app`、后续 `crates/md-render`

### 背景

实施计划要求建立七 crate 的 Workspace，并坚持“非 UI crate 不依赖 GPUI”。原仓库是单包 `coti`，根 `Cargo.toml` 直接依赖 GPUI，但没有实际 UI 代码；如果在阶段 0 就编译 GPUI，会让文档模型、Markdown、编辑器和布局阶段的构建被 UI 工具链与平台依赖拖慢。

### 决策

1. 根 `Cargo.toml` 改为 virtual workspace，使用 `resolver = "3"`，不保留根 package。
2. 应用入口迁移到 `crates/md-app`：包名 `md-app`，二进制名保留产品名 `coti`。
3. 阶段 0～7 的 active dependency graph 不包含 GPUI；`md-render` / `md-app` 在阶段 8 接入。
4. 接入 GPUI 时固定使用原脚手架 `Cargo.lock` 记录的 revision：`f4fe0be1f8c7974db07e01c5cd05bee0cf4eb8d9`，避免重新解析到不同版本。

### 理由

- 保持 `md-core` 到 `md-layout` 的构建快速、可 Headless 测试，直接落实 AC-09 的依赖边界。
- 阶段 0 的验收目标是 Workspace 可解析、可构建，不是验证 GPUI 平台 API。
- 固定 revision 可以保留原平台选型，同时不把 GPUI 的编译成本提前到每一步。

### 后果

- `Cargo.lock` 会移除当前未被引用的 GPUI 及其传递依赖，文件显著缩小；这是预期结果，不是依赖丢失。
- 阶段 8 重新引入 GPUI 时，需要重新生成锁文件并验证 Windows/macOS/Linux 构建。
- 如果后续决定提前验证 GPUI，只需在阶段 0.2 把 workspace 依赖接入 `md-render`/`md-app`，不需要改变 Workspace 结构。

### 迁移

无数据迁移。阶段 8 按上述 revision 把 `gpui`、`gpui_platform` 加入 `[workspace.dependencies]`，并由 `md-render`、`md-app` 引用。

# 变更记录

本文件按实施步骤记录代码、文档与工程配置的变更。每个小步骤一个条目，包含变更内容、验证结果、设计说明和下一步；阶段级进度在 `IMPLEMENTATION.md` 中汇总。

## 2026-11-23 · 步骤 0.1：创建 Cargo Workspace

**状态：** 已完成，等待确认后进入 0.2。

### 变更

- 根 `Cargo.toml` 从单包清单改为 virtual workspace，`resolver = "3"`，当前成员为 `crates/md-app`。
- 原根包 `coti` 的应用入口迁移到 `crates/md-app`：包名 `md-app`，可执行文件名保留 `coti`。
- 删除根 `src/`（原 `lib.rs` 为空，`main.rs` 仅占位输出）。
- 新增 `docs/ARCHITECTURE_DECISIONS.md`，记录 ADR-0001：Workspace 结构与 GPUI 延后接线。
- `Cargo.lock` 重新生成；移除当前未引用的 GPUI 传递依赖，锁文件从 6043 行缩减为 6 行。

### 验证

- `cargo metadata --no-deps`：成功解析。
- `cargo build --workspace`：成功，无警告。
- `cargo test --workspace`：成功（当前无测试）。
- `cargo run -p md-app`：输出 `Coti: 日常书写，日结为札`。

### 设计说明

- 阶段 0～7 不把 GPUI 放入 active dependency graph，避免非 UI 阶段被平台工具链阻塞；GPUI revision 固定在 ADR-0001 中，阶段 8 接入。
- 应用包名使用文档约定的 `md-app`，二进制名保留产品名 `coti`，避免后续安装包与启动命令改名。
- 当前 Workspace 只有 `md-app` 一个成员；0.2 补齐其余六个 crate 并建立依赖边。

### 下一步

- 0.2 创建七个 crate，明确目录、职责和依赖关系。

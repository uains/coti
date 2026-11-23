# 开发指南

## 环境要求

- Rust stable。仓库通过 `rust-toolchain.toml` 安装 `rustfmt`、`clippy`；Edition 为 2024。
- Windows 需要 MSVC Build Tools；macOS 需要 Xcode Command Line Tools；Linux 需要常规 C 工具链。
- 阶段 0～7 不需要 GPUI 或图形系统依赖；阶段 8 接入 GPUI 后再补充平台要求。

## 快速开始

```powershell
cargo run -p md-app
```

当前 `md-app` 只输出启动信息；真正的窗口在阶段 9 接入。

## 质量门禁

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
./scripts/check-layering.ps1
```

Linux / macOS 使用：

```bash
bash scripts/check-layering.sh
```

CI 在 Ubuntu、Windows、macOS 三平台运行同一组检查，见 `.github/workflows/ci.yml`。

## Workspace 边界

```text
md-core
  ├── md-document
  │     ├── md-markdown
  │     ├── md-editor
  │     └── md-layout
  └── md-render ── md-app
```

- `md-core`～`md-layout` 不得依赖 GPUI。
- `md-render` 是唯一允许依赖 GPUI 的渲染层；`md-app` 负责装配。
- `md-app` 可依赖全部下层 crate；下层不得反向依赖 `md-app`。
- 新增跨 crate 类型前先查 `docs/TYPE_OWNERSHIP.md`。
- 新增库错误按 `docs/ERROR_HANDLING.md` 使用 `thiserror`，应用边界才用 `anyhow`。

## 提交与变更记录

每个小步骤：

1. 实现代码与测试。
2. 运行 fmt、clippy、test、layering。
3. 在 `docs/CHANGELOG.md` 追加一节，记录变更、验证、设计说明和下一步。
4. 单独 git commit。

每个阶段结束：

1. 更新 `docs/IMPLEMENTATION.md` 的“实施进度”一节。
2. 汇总阶段验收结果与已知问题。
3. 停下来等待确认，再进入下一阶段。

## 文档索引

- `docs/DESIGN.md`：设计基线。
- `docs/IMPLEMENTATION.md`：实施计划与进度。
- `docs/ARCHITECTURE_DECISIONS.md`：架构决策记录。
- `docs/TYPE_OWNERSHIP.md`：公共类型归属。
- `docs/ERROR_HANDLING.md`：错误处理规范。
- `docs/OBSERVABILITY.md`：日志与 tracing 规范。
- `docs/TESTING.md`：测试规范。
- `docs/CHANGELOG.md`：逐步骤变更记录。

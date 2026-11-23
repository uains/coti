# 错误处理规范

本文件落实 `IMPLEMENTATION.md` 步骤 0.4。目标是让错误**可分类、可定位、可追踪**，同时避免底层库把应用层错误模型带进核心 API。

## 分层策略

| 层级 | 策略 |
|---|---|
| 库 crate（`md-core`～`md-render`） | 使用 `thiserror` 定义有类型的错误枚举；公开 API 不返回 `anyhow::Error` |
| 应用边界（`md-app`） | 使用 `anyhow::Result` 聚合错误，并在文件、窗口、命令边界补充 context |
| 测试 | 断言错误变体、`Display` 文本和 source 链；不依赖易变的完整字符串 |

## 库错误规则

1. 每个有失败可能的 crate 定义独立错误枚举，例如 `TextError`、`DocumentError`、`PositionError`、`MarkdownError`、`EditorError`、`LayoutError`、`RenderError`。
2. 错误枚举加 `#[non_exhaustive]`，允许后续增加变体而不破坏下游 match。
3. 下层错误通过 `#[error("{0}")]` + `#[from]` 上抛：Display 只保留根因文本，`source()` 仍返回被包装的下层错误，保留完整链路。
4. 变体字段携带定位信息，例如 offset、范围、BlockId 的 index/generation、revision；不要只返回一个字符串。
5. 不在库内部 `unwrap()`；只有类型系统或前置校验已证明不可失败时才使用 `expect()`，并写明原因。
6. 库不记录日志代替返回错误；日志由调用边界或 `tracing` span 补充上下文。

## 应用边界规则

- `md-app` 的公开入口返回 `anyhow::Result<()>`。
- 在文件 IO、主题加载、窗口创建等边界使用 `.context("...")` / `.with_context(|| ...)`。
- 用户可见错误与内部错误分开：前者转为状态栏 / 对话框文案，后者进入 tracing。
- 不要把 `anyhow::Error` 作为库 crate 的公开返回类型。

## 错误演进

- 不改变既有变体的语义；需要新语义时新增变体。
- 错误文本可以改进，但测试只断言关键分类和关键字段。
- 新增 crate 间依赖时，优先用 `#[from]` 传递底层错误，不复制错误类型。

## 当前实现

- `md-core`：`TextError`（越界、非 UTF-8 边界、非法范围、非法 UTF-16）。
- `md-document`：`DocumentError`、`PositionError`。
- `md-markdown`：`MarkdownError`。
- `md-editor`：`EditorError`。
- `md-layout`：`LayoutError`。
- `md-render`：`RenderError`。
- `md-app`：`anyhow` 边界；`run()` 返回 `anyhow::Result<()>`。

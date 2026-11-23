# 可观测性规范

本文件落实 `IMPLEMENTATION.md` 步骤 0.5，并对应 `DESIGN.md` §14.1。当前阶段只建立日志基础设施；具体 span 在实现编辑事务、布局和渲染时补齐。

## 入口

- 应用入口 `md_app::run()` 调用 `md_app::logging::init()`。
- 过滤级别从 `RUST_LOG` 读取；未设置时默认 `info`。
- 示例：
  - `RUST_LOG=info cargo run -p md-app`
  - `RUST_LOG=md_editor=debug,md_layout=trace cargo run -p md-app`

## 分层约定

| 层 | 记录内容 |
|---|---|
| `md-app` | 启动/退出、窗口生命周期、文件打开保存结果、主题加载失败、用户可见错误 |
| `md-editor` | Intent 分派、命中的 `RuleId`、事务提交/回滚、History 栈变化 |
| `md-layout` | `layout.sync` 耗时、重测块数、脏块数、可见范围 |
| `md-render` | 帧耗时、绘制行数、资源加载失败 |
| `md-markdown` | 解析/序列化失败、保真降级、dirty 节点数量 |

`md-core` 与 `md-document` 的底层纯函数默认不记录日志，避免热路径分配；需要定位时由上层 span 携带输入摘要。

## 字段与级别

- `error`：用户操作失败或数据可能损坏，必须可定位。
- `warn`：可恢复的降级、主题校正、资源失败。
- `info`：应用生命周期和文件级操作。
- `debug`：每个 Intent 的 RuleId、Ctx 摘要、事务结果。
- `trace`：布局/测量/映射的细粒度过程，默认关闭。

关键 span 建议字段：

- `intent`：`rule_id`、`doc_revision`、`selection_count`、`elapsed_us`。
- `transaction`：`operation_count`、`dirty_blocks`、`committed`。
- `layout.sync`：`relaid_blocks`、`visible_blocks`、`elapsed_us`。
- `render.frame`：`painted_lines`、`elapsed_us`。

## 隐私与性能

- 默认不记录文档正文；只记录长度、块数、offset 范围等摘要。
- 需要正文诊断时使用 `trace`，并由开发者显式开启。
- 避免在 `enabled!` 为 false 时构造昂贵字段；高频路径先判断再格式化。
- 日志初始化失败不得阻止应用启动；当前实现忽略重复初始化错误。

# 测试规范

本文件落实 `IMPLEMENTATION.md` 步骤 0.6，并对应 `DESIGN.md` §5 的四层测试体系。测试必须能脱离 GPUI 和用户环境独立运行。

## 命令

```powershell
# 全 Workspace
cargo test --workspace

# 单个 crate
cargo test -p md-core

# 单个测试
cargo test -p md-editor stale_revision -- --nocapture

# 文档测试
cargo test --workspace --doc

# 后续属性测试与基准
cargo test --workspace --release
cargo bench
```

## 四层测试

| 层级 | 位置 | 当前状态 | 后续 |
|---|---|---|---|
| Unit | 各 crate 的 `#[cfg(test)] mod tests` | 已建立 | 随每个数据结构/算法补齐 |
| Integration | `crates/md-app/tests/` 与各 crate 的 `tests/` | 已建立 `app_smoke` | 阶段 4 起增加 Markdown 往返夹具 |
| Property | 核心 crate 的 `tests/property_*.rs` | 待阶段 1 引入 `proptest` | 文本边界、DocPos、Undo、Layout 等价 |
| Benchmark | `benches/` | 待阶段 7/11 | 输入延迟、布局、滚动 |

## 当前测试清单

| crate | 测试数 | 覆盖内容 |
|---|---:|---|
| `md-core` | 3 | `TextError` 的 Display 字段 |
| `md-document` | 3 | 错误转发、source、BlockId/Position 错误文本 |
| `md-markdown` | 2 | 解析错误、文档错误转发 |
| `md-editor` | 3 | 过期 revision、非法 Intent、文档错误转发 |
| `md-layout` | 3 | 几何错误、缺失块布局、文档错误转发 |
| `md-render` | 2 | 资源错误、布局错误转发 |
| `md-app` | 2 unit + 2 integration | 日志初始化幂等、入口返回、跨层 source 链、真实二进制启动 |

## 约定

1. 测试名描述“行为 + 条件 + 预期”，例如 `stale_revision_reports_both_revisions`。
2. 断言错误分类和关键字段，不对完整 UI 文案做脆弱断言；用户文案变化时只改必要测试。
3. 集成测试不访问网络、不写用户目录；需要文件系统时使用临时目录。
4. 测试不得依赖 GPUI；UI 相关测试在阶段 8 之后单独建立。
5. 每个新 RuleId 至少三条用例，使用 `name / rule / before / intent / after` 夹具。
6. 随机测试使用固定 seed 时可复现；失败输出必须包含 seed 和最小化输入。

## 阶段 0 验收

- 七个 crate 均有可执行测试。
- `cargo test --workspace` 全部通过。
- `md-app` 集成测试真实启动 `coti` 二进制并检查输出。
- 后续属性测试框架在阶段 1 接入，不在本阶段伪造属性测试。

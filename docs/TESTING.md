# 测试规范

本文件落实 `IMPLEMENTATION.md` 步骤 0.6，并对应 `DESIGN.md` §5 的四层测试体系。测试必须能脱离 GPUI 和用户环境独立运行。

## 命令

```powershell
# 全 Workspace
cargo test --workspace

# 单个 crate
cargo test -p md-core

# 属性测试
cargo test -p md-core --test text_properties

# 单个测试
cargo test -p md-editor stale_revision -- --nocapture

# 文档测试
cargo test --workspace --doc

# 后续基准
cargo bench
```

## 四层测试

| 层级 | 位置 | 当前状态 | 后续 |
|---|---|---|---|
| Unit | 各 crate 的 `#[cfg(test)] mod tests` | 已建立 | 随每个数据结构/算法补齐 |
| Integration | `crates/md-app/tests/` 与各 crate 的 `tests/` | 已建立 `app_smoke` | 阶段 4 起增加 Markdown 往返夹具 |
| Property | `crates/md-core/tests/`、`crates/md-document/tests/` | 10 个文本/文档属性 | 阶段 3 扩展 DocPos、Selection，阶段 5/6 扩展 Undo |
| Benchmark | `benches/` | 待阶段 7/11 | 输入延迟、布局、滚动 |

## 当前测试清单

| crate | 测试数 | 覆盖内容 |
|---|---:|---|
| `md-core` | 32 unit + 7 property | 文本偏移/范围、UTF-8 边界、grapheme、UTF-16、修改原子性、几何、属性往返 |
| `md-document` | 47 unit + 3 property | Inline、Block、Arena generation/staging、Document 结构、Validator、快照 |
| `md-markdown` | 2 | 解析错误、文档错误转发 |
| `md-editor` | 3 | 过期 revision、非法 Intent、文档错误转发 |
| `md-layout` | 3 | 几何错误、缺失块布局、文档错误转发 |
| `md-render` | 2 | 资源错误、布局错误转发 |
| `md-app` | 2 unit + 2 integration | 日志初始化幂等、入口返回、跨层 source 链、真实二进制启动 |

## 当前属性测试

`crates/md-core/tests/text_properties.rs` 使用 `proptest` 覆盖：

- `to_utf16` / `from_utf16` 往返。
- 每个 UTF-8 字符边界的 UTF-8 ↔ UTF-16 offset 往返。
- 从 0 开始按 grapheme 前进能到达文本末尾。
- 从末尾按 grapheme 后退能回到 0。
- 用自身切片替换自身是恒等操作。
- 在合法边界插入再删除插入段可恢复原文。
- 在合法边界 `split_off` 后 `concat` 可恢复原文。

`crates/md-document/tests/document_properties.rs` 覆盖：

- 随机插入根段落，`Document::validate` 始终成立。
- 随机删除根块，根级始终保留至少一个 Paragraph。
- 随机移动根块，原 `BlockId` 保留且 `validate` 始终成立。

## 约定

1. 测试名描述“行为 + 条件 + 预期”，例如 `stale_revision_reports_both_revisions`。
2. 断言错误分类和关键字段，不对完整 UI 文案做脆弱断言；用户文案变化时只改必要测试。
3. 集成测试不访问网络、不写用户目录；需要文件系统时使用临时目录。
4. 测试不得依赖 GPUI；UI 相关测试在阶段 8 之后单独建立。
5. 每个新 RuleId 至少三条用例，使用 `name / rule / before / intent / after` 夹具。
6. 随机测试使用固定 seed 时可复现；失败输出必须包含 seed 和最小化输入。
7. 属性测试生成小规模输入（≤64 个 char），避免 CI 时间不可控。

## 阶段 0～2 验收

- 七个 crate 均有可执行测试。
- `cargo test --workspace` 全部通过。
- `md-app` 集成测试真实启动 `coti` 二进制并检查输出。
- `md-core` 属性测试覆盖文本边界、grapheme、UTF-16 和修改往返。
- `md-document` 属性测试覆盖根级插入/删除/移动后的结构不变式。
- 后续属性测试在阶段 3 扩展到 DocPos/Selection，阶段 5/6 扩展到 Undo，不伪造未实现模块的属性测试。


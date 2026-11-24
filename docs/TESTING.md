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
| Property | `crates/md-core/tests/`、`crates/md-document/tests/` | 13 个文本/文档/位置属性 | 阶段 5/6 扩展 Undo、History |
| Benchmark | `benches/` | 待阶段 7/11 | 输入延迟、布局、滚动 |

## 当前测试清单

| crate | 测试数 | 覆盖内容 |
|---|---:|---|
| `md-core` | 32 unit + 7 property | 文本偏移/范围、UTF-8 边界、grapheme、UTF-16、修改原子性、几何、属性往返 |
| `md-document` | 70 unit + 6 property | Inline、Block、Arena、Document、DocumentIndex、DocPos/Position、Selection、Validator |
| `md-markdown` | 10 unit + 4 integration | 解析器、Serializer、200 篇夹具逐字节保真/重解析/稳定性 |
| `md-editor` | 89 unit + 4 property | Intent、Editor、规则、Operation/inverse、Transaction、PositionMapping、History、IME、Clipboard |
| `md-layout` | 19 | 测量、Inline/Block Layout、Hit-test、选区、增量布局、5000 次等价 |
| `md-render` | 12 | 主题、Paint 分层、Shaping、输入映射、滚动、Image 资源、RenderView |
| `md-app` | 13 unit + 5 flow + 2 smoke | 配置、文件 IO、原子保存、最近文件、窗口、菜单、大纲/搜索/折叠/状态/导出 |

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

`crates/md-document/tests/position_properties.rs` 覆盖：

- 合法 `DocPos` 至少有一种 Affinity 可无损往返。
- 每个叶子的所有合法局部 offset 都能映射回对应 `DocPos`。
- 多选区集合倒序排序后通过 `SelectionSet::validate`。
- 夹具包含空段落、中文、Atom、多层 Quote、嵌套 List、Table/TableCell。

## Markdown 夹具

`crates/md-markdown/tests/fixtures/markdown/` 包含 200 个合成夹具（20 类 × 10 变体）。`tests/fixtures.rs` 验证：

- 夹具数量 ≥200。
- 每个夹具 `parse → validate → serialize` 逐字节相等（AC-01 过渡门槛）。
- dirty canonical 输出可重新 parse 且 validate。
- dirty canonical 二次序列化稳定（幂等）。
- 夹具覆盖 CommonMark/GFM、中文、Emoji、表格、脚注、数学、Frontmatter、HTML、嵌套容器。

已知限制：这些是合成语料，不是外部真实 Markdown；阶段 11 前仍需真实语料复验。

## 约定

1. 测试名描述“行为 + 条件 + 预期”，例如 `stale_revision_reports_both_revisions`。
2. 断言错误分类和关键字段，不对完整 UI 文案做脆弱断言；用户文案变化时只改必要测试。
3. 集成测试不访问网络、不写用户目录；需要文件系统时使用临时目录。
4. 测试不得依赖 GPUI；UI 相关测试在阶段 8 之后单独建立。
5. 每个新 RuleId 至少三条用例，使用 `name / rule / before / intent / after` 夹具。
6. 随机测试使用固定 seed 时可复现；失败输出必须包含 seed 和最小化输入。
7. 属性测试生成小规模输入（≤64 个 char），避免 CI 时间不可控。

## 阶段 0～4 验收

- 七个 crate 均有可执行测试。
- `cargo test --workspace` 全部通过。
- `md-app` 集成测试真实启动 `coti` 二进制并检查输出。
- `md-core` 属性测试覆盖文本边界、grapheme、UTF-16 和修改往返。
- `md-document` 属性测试覆盖根级插入/删除/移动、DocPos/Affinity 往返和多选区排序。
- `md-markdown` 200 篇合成夹具通过 pristine 逐字节相等、dirty 重解析与稳定性。
- 后续属性测试在阶段 5/6 扩展到 Undo/History，不伪造未实现模块的属性测试。

## 阶段 5（进行中）

- 5.1 已完成：Move 规则、grapheme/Atom/跨叶光标。
- 5.2～5.3 已完成：Operation/inverse、Transaction 快照回滚、ChangeSet。
- 5.4～5.5 第一批已完成：PositionMapping、InsertText/DeleteBackward 规则。
- 当前 `md-editor` 89 unit + 4 property + 2 undo-stability；`md-document` +1 随机不变式；`md-markdown` +1 模型闭合；`md-layout` 19 + 2 ignored scale；`md-render` 12；`md-app` 20；总计 291 个测试通过。Windows Release 可执行文件 `dist/coti.exe` 与 GUI smoke 已验证。
- Enter/DeleteForward/DeleteRange/Tab/List/Quote/Table 规则尚未完成；当前不对阶段 5 做整体验收。


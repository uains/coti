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

## ADR-0002：`DocPos` 归属 `md-document`，不下沉到 `md-core`

- **状态：** Accepted
- **日期：** 2026-11-23
- **影响范围：** `md-core`、`md-document`、阶段 1～3 的实现顺序

### 背景

`IMPLEMENTATION.md` 步骤 1.1 在 `md-core` 下写到“定义 TextOffset、TextRange、DocPos 等基础类型”；但 `DESIGN.md` §2.2 将 `DocPos` 明确列为 `md-document` 的职责，§4 也在文档模型章节定义 `DocPos`、`Affinity` 和 `Position`。两者存在归属冲突。

### 决策

1. `md-core` 只拥有文本与几何原语：`TextOffset`、`TextRange`、`TextBuf`、`Point`、`Size`、`Rect`、`EdgeInsets` 及其错误类型。
2. `DocPos`、`Affinity`、`Position`、`BlockGapPosition` 全部归 `md-document`。
3. 阶段 1 不实现 `DocPos`；阶段 2～3 在 `md-document` 中实现。
4. 完整类型清单以 `TYPE_OWNERSHIP.md` 为准。

### 理由

- `DocPos` 的取值语义依赖叶子块长度、Block Separator 和 Affinity，不是纯文本偏移；把它放进 `md-core` 会让底层 crate 反向依赖文档概念。
- 与 `DESIGN.md` 的 crate 职责划分一致，避免后续为“共享坐标”制造循环依赖。
- `DocPos` 在物理上仍是轻量 newtype，放在 `md-document` 不会带来性能或分层成本。

### 后果

- 实施文档 1.1 中 `DocPos` 的表述按本 ADR 修正理解；阶段 1 验收不包含 `DocPos`。
- 阶段 1 的文本 API 仍使用 `TextOffset`/`usize` 字节偏移；阶段 2～3 再引入文档坐标。
- `TYPE_OWNERSHIP.md` 是新增类型的准入清单，后续不得在 `md-core` 复制 `DocPos`。

### 迁移

无数据迁移。当前尚无实现代码。

## ADR-0003：`TextOffset` / `TextRange` 使用 newtype

- **状态：** Accepted
- **日期：** 2026-11-23
- **影响范围：** `md-core`，以及后续所有文本/文档 API

### 背景

文本偏移、长度、计数和 `DocPos` 都是 `usize`。如果全部使用裸 `usize`，很容易把字节偏移、字符数、UTF-16 码元数或块索引混用，而且编译器无法发现。

### 决策

1. `TextOffset` 使用 `#[repr(transparent)]` 的 `usize` newtype。
2. `TextRange` 保存 `TextOffset` 的 start/end，使用半开区间 `[start, end)`。
3. 提供 `From<usize>` / `From<TextOffset> for usize` 以及 `std::ops::Range<usize>` 转换，边界代码可以低摩擦接入。
4. `TextRange` 允许暂时构造为 reversed；所有切片/修改操作必须先经过 `TextBuf::validate_range`。
5. `TextBuf` 内部持有 `String`，只暴露 `&str`；禁止暴露 `&mut String`。
6. 用户级移动仍按 grapheme cluster 计算，不能对 `TextOffset` 直接 `+1/-1` 代替字符移动。

### 理由

- 类型系统可以阻止“把长度当偏移”“把 UTF-16 码元数当 UTF-8 字节数”这类错误。
- `#[repr(transparent)]` 不增加运行时开销，转换成本为零。
- 允许 reversed 的 `TextRange` 使构造不 panic；错误在操作边界以 `TextError::InvalidRange` 返回，符合库错误规范。

### 后果

- 跨 crate API 会多一些 `.get()` / `.into()`；这是有意的显式边界。
- 阶段 2/3 的 `DocPos` 仍使用独立 newtype，不与 `TextOffset` 混用。
- 后续属性测试必须覆盖非法范围、非字符边界和 UTF-16 中间位置。

## ADR-0004：Arena 暂存语义与 Document 验证门槛

- **状态：** Accepted
- **日期：** 2026-11-23
- **影响范围：** `NodeArena`、`Document`、阶段 5 Transaction/History

### 背景

Undo 需要恢复被删除节点的原 `BlockId`；如果删除时立即递增 generation 并回收槽位，旧 ID 就会失效，inverse 操作无法恢复。另一方面，构建嵌套文档时必须能先创建空容器、再追加子块，因此结构原语不能每一步都要求整棵树合法。

### 决策

1. `NodeArena` 支持 live / staged / free 三态。
2. `stage` 只把节点移出 live 视图，保留槽位与 generation；`restore` 用原 ID 恢复；只有 `release_staged` 才递增 generation 并回收。
3. generation 达到 `u32::MAX` 后槽位不再回收，避免回绕后旧 ID 重新有效。
4. `Document` 的结构原语（`insert_block`/`remove_block`/`move_block`）只检查局部前置条件，允许短暂的不完整状态；`Document::validate` 是发布 revision 前的统一门槛。
5. 阶段 5 的 Transaction 必须在提交前执行 `validate`，失败回滚；History 负责 staged 节点的恢复或释放。

### 理由

- 将“结构构造”和“提交验收”分开，避免为了构造嵌套树而放宽最终不变量。
- staged 节点保留原 ID，是 Undo 完整恢复文档与选区的前提。
- 统一 `validate` 入口比在每个结构原语里重复部分整树检查更容易维护。

### 后果

- `Document::remove_block` 当前直接 `free`，是阶段 2 的简化路径；阶段 5 必须改用 staging 或 inverse snapshot。
- 调用方在两次结构修改之间可能观察到暂时非法的文档；只有 `validate` 成功后才能对外发布。
- 每个新结构原语仍需做局部前置检查，不能把明显错误延迟到最终校验。

## ADR-0005：`BlockNode.source` 暂缓到阶段 4 前决定

- **状态：** Accepted（待阶段 4 前复审）
- **日期：** 2026-11-23
- **影响范围：** `BlockNode`、`md-markdown`、SourceHint

### 背景

`DESIGN.md` 的 `BlockNode` 示例包含 `source: SourceHint`，但 `SourceHint` 在 crate 职责中属于 `md-markdown`，而 `md-document` 不能依赖 `md-markdown`。在未实现解析器前，直接选择拆分或下沉都可能造成错误抽象。

### 决策

- 阶段 2 的 `BlockNode` 只包含 `id`、`parent`、`kind`，不含 `source` 字段。
- 阶段 4 实现 SourceHint/Serializer 前，必须重新评估并选择：
  1. `md-document` 定义轻量 `SourceSpan`，`md-markdown` 用 `BlockId -> StyleHints` 侧表。
  2. 将完整 SourceHint 移入 `md-document`（需接受 Markdown 风格提示进入文档模型）。
  3. 其他能保持 crate 单向依赖的方案。

### 理由

- 当前阶段不需要 source 元数据，提前引入会限制后续解析器的设计。
- 保持 `md-document` 不依赖 Markdown 语法，避免破坏 AC-09 的依赖边界。

### 后果

- 阶段 4 前必须补充 ADR 或修订本条，不能直接沿用 `BlockNode` 示例。
- 节点拆分/合并/移动时 SourceHint 的继承规则尚未落地，属于阶段 4 的设计任务。

## ADR-0006：DocumentIndex 归属与根级 Gap 表示

- **状态：** Accepted
- **日期：** 2026-11-24
- **影响范围：** `Document`、`DocumentIndex`、`BlockGapPosition`、`Selection`

### 背景

DocumentIndex 是派生缓存，但 Selection、Layout 和 Editor 都需要与同一份文档结构一致的索引。根级插入点没有父 BlockId；`BlockGapPosition` 需要一种明确表示。表格单元格选择如果只有 `(row, column)`，无法定位所属表格。

### 决策

1. `Document` 持有 `DocumentIndex`。
2. 每次结构修改后重建索引；`Document::validate` 同时校验索引一致性。
3. `BlockGapPosition.parent` 使用 `Option<BlockId>`，`None` 表示根级间隙。
4. `SelectionKind::TableCells` 额外保存 `table: BlockId`。
5. 第一版索引采用全量重建，不做增量维护。

### 理由

- 单一所有者避免 Document、Editor、Layout 各自持有不同版本的索引。
- 重建优先保证正确性；10 万字目标下，结构修改频率远低于文本输入，后续可用基准决定是否增量更新。
- `Option<BlockId>` 比哨兵 id 更不易与真实 Arena 索引混淆。
- 表格选择必须知道 table，才能在 Table 的 `aligns/rows` 上做边界校验。

### 后果

- 每次 `insert_block`/`remove_block`/`move_block` 为 O(n) 索引重建；阶段 7 性能测试若证明不足，再引入增量维护。
- `Node` / `TableCells` 选择的 anchor/head 是占位值，不参与其语义校验。
- 索引陈旧由 `Document::validate` 发现；事务提交前必须调用。
- 后续 Layout/Selection 必须通过 Document 的公开索引访问，不得各自重建一份长期缓存。

## ADR-0007：SourceHint 移入 `md-document`

- **状态：** Accepted
- **日期：** 2026-11-24
- **影响范围：** `BlockNode`、`md-document`、`md-markdown`、阶段 4～9

### 背景

ADR-0005 将 `BlockNode.source` 暂缓到阶段 4。Editor 在编辑时必须维护每个节点的 source 状态；如果完整 `SourceHint` 留在 `md-markdown`，`md-editor` 就必须依赖 Markdown 或通过侧表回调同步，容易产生陈旧元数据。

### 决策

1. `SourceHint`、`StyleHints`、`HeadingStyle`、`ListStyleHint`、`HardBreakStyle` 移入 `md-document`。
2. `BlockNode` 增加 `source: SourceHint`。
3. `md-markdown` 只负责解析并写入 SourceHint、序列化时读取；不拥有第二份节点元数据。
4. `SourceHint` 的 Markdown 风格字段集中在 `source` 模块；`md-document` 不解析 Markdown。

### 理由

- 单一数据源：Editor、Parser、Serializer 读写同一个 SourceHint，避免侧表与节点生命周期不同步。
- 未编辑保真、dirty 重排和 Undo 都可以直接依赖节点上的元数据。
- 让 `md-document` 拥有“源文本元数据”类型，是有限度的边界让步；不引入 Markdown 解析逻辑。

### 后果

- `md-document` 的职责说明需要承认 source metadata；`BlockNode` 不再是无 Markdown 概念的纯语义节点。
- `md-markdown` 不得再定义自己的 SourceHint；如发现字段不足，先扩展 `md-document::StyleHints`。
- SourceHint range 必须通过 `Document::validate` 校验；pristine 范围必须落在 UTF-8 边界内。
- 后续 split/join/move 的 SourceHint 继承规则在阶段 5 实现。

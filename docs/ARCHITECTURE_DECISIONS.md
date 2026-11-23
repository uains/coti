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

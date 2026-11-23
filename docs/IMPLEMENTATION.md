# 本地 Markdown 编辑器实施计划

> **项目目标：** 基于 Rust + GPUI 实现一个可在本地独立运行的 Markdown
> 桌面编辑器。\
> **设计依据：**`DESIGN.md`（项目最终设计规范）。\
> **文档性质：**
> 实施路线、任务拆分与阶段验收标准；不代表相关功能已经实现。\
> **维护原则：**
> 总体设计与实施状态分开维护。架构决策发生变化时，记录理由、影响模块、缓存失效范围和迁移计划。

------------------------------------------------------------------------

## 1. 项目目标与交付范围

### 1.1 最终交付目标

交付一个本地优先的 Markdown 桌面应用，具备以下能力：

-   使用 Rust 实现独立于 UI 的文档模型和编辑内核。
-   使用 GPUI 实现桌面窗口、交互和视觉呈现。
-   在排版后的文档中直接编辑，采用编辑态即阅读态的交互方式。
-   支持本地 Markdown 文件的新建、打开、编辑、保存和另存为。
-   支持常见 Markdown 与 GFM 结构。
-   支持跨块光标、选区、撤销重做和中文输入法。
-   支持长文档增量布局和可见区域虚拟化。
-   支持主题、搜索、大纲、折叠、命令面板和图片加载。
-   提供可安装、可独立运行的桌面应用，而不只是 Cargo 示例或 UI Demo。

### 1.2 首个正式交付版本的功能范围

  模块       交付能力
  ---------- -----------------------------------------
  文档结构   标题、段落、列表、引用、代码块、表格
  行内格式   加粗、斜体、删除线、下划线、链接、高亮
  特殊内容   图片、脚注、数学源码、HTML、Frontmatter
  编辑交互   光标、选区、多选区、跨块编辑
  输入       中文 IME、剪贴板、快捷输入
  历史       Undo / Redo、连续输入合并
  文件       新建、打开、保存、另存为、导出
  阅读       大纲、搜索、折叠、专注模式
  布局       自动换行、长文虚拟化、增量布局
  外观       深色、浅色、主题配置、字体配置

范围约束：

-   数学公式采用源码展示。
-   HTML 块采用可折叠 Raw 源码展示。
-   表格单元格暂不支持嵌套块。
-   HTML 粘贴属于后续扩展，不作为首个正式交付版本的完成条件。
-   其他已知限制以 `DESIGN.md` 为准，不在实施中默默扩大或改变承诺。

------------------------------------------------------------------------

## 2. 架构基线与工程边界

### 2.1 已确定的核心架构决策

实施过程中遵循设计文档已确定的决策：

1.  **Block 节点采用 Arena。** 使用稳定节点身份和 Generation
    管理节点生命周期。
2.  **Inline 内容采用 Flat Run。** 普通文本以连续 Run 存储，Marks
    表达行内格式，Atom 表达不可拆分的原子内容。
3.  **DocPos 是唯一权威坐标。** Selection、IME、搜索、剪贴板、跨块范围和
    Layout 映射统一使用 DocPos。

不在实现过程中随意替换上述决策。如确需变更，必须先记录架构决策及迁移方案。

### 2.2 Crate 划分

建议 Workspace：

``` text
markdown-editor/
├── Cargo.toml
├── Cargo.lock
├── crates/
│   ├── md-core/
│   ├── md-document/
│   ├── md-markdown/
│   ├── md-editor/
│   ├── md-layout/
│   ├── md-render/
│   └── md-app/
├── assets/
│   ├── fonts/
│   ├── themes/
│   └── icons/
├── tests/
├── benches/
└── docs/
    ├── DESIGN.md
    ├── IMPLEMENTATION.md
    ├── ARCHITECTURE_DECISIONS.md
    └── TESTING.md
```

  ---------------------------------------------------------------------------------------------------------
  Crate                               职责
  ----------------------------------- ---------------------------------------------------------------------
  `md-core`                           文本、字符边界、基础几何和通用基础类型

  `md-document`                       Arena、Block、Inline、Document、DocumentIndex、DocPos、Selection

  `md-markdown`                       Markdown Parser、Builder、Serializer、SourceHint

  `md-editor`                         Intent、Rule、Operation、Transaction、PositionMapping、History、IME

  `md-layout`                         文本测量、换行、布局树、Hit-test、光标和选区几何、虚拟化

  `md-render`                         GPUI Element、Shaping、Paint、交互呈现和资源绘制

  `md-app`                            应用装配、窗口、文件 IO、菜单、快捷键、主题和偏好设置
  ---------------------------------------------------------------------------------------------------------

### 2.3 依赖边界

-   非 UI Crate 不依赖 GPUI。
-   `md-render` 负责 GPUI 适配，不直接拥有编辑规则。
-   `md-editor` 通过受控接口修改 Document，不允许 UI
    回调直接修改底层节点。
-   Layout 提供几何结果，Render 消费几何结果。
-   Document 只保存语义 Marks；Layout 消费解析后的样式。
-   主题绘制字段变化不得无故触发文本重新测量或布局。

------------------------------------------------------------------------

## 3. 总体实施路线

项目分为 12 个阶段，按依赖关系自底向上实施。

  阶段   模块                   核心产物
  ------ ---------------------- --------------------------------------------
  0      工程基础设施           Workspace、Crate、CI、测试规范
  1      `md-core`              文本与基础几何设施
  2      `md-document`          Arena、Block、Inline、Document
  3      位置和选区系统         DocumentIndex、DocPos、Position、Selection
  4      `md-markdown`          Parser、Builder、SourceHint、Serializer
  5      `md-editor`            Intent、Rule、Operation、Transaction
  6      History、IME、剪贴板   Undo / Redo、Composition、输入适配
  7      `md-layout`            测量、换行、几何映射、增量布局
  8      `md-render`            GPUI 渲染、Shaping、Paint、交互
  9      `md-app`               本地桌面应用和文件 IO
  10     产品功能完善           搜索、大纲、折叠、图片、导出
  11     稳定性与发布           性能、平台验证、安装包和最终验收

### 3.1 关键里程碑

  里程碑   对应阶段   里程碑结果
  -------- ---------- ----------------------------------------
  M1       0～3       可测试的文档模型、位置和选区系统
  M2       4          Markdown 解析与源码保真序列化
  M3       5～6       可脱离 GPUI 运行的 Headless 编辑内核
  M4       7          可测量、换行、定位和增量更新的布局引擎
  M5       8～9       可以实际使用的本地桌面编辑器
  M6       10～11     首个正式交付版本功能完整、性能达标并可发布

**M3 是关键质量门槛：** 在进入 GPUI
集成之前，必须能通过自动化测试完成文档编辑、选区映射、撤销重做和 IME
状态相关的核心流程。

------------------------------------------------------------------------

## 4. 分阶段实施任务

每个阶段由多个小步骤组成。小步骤的具体接口和代码结构，应在进入该步骤时结合当前代码进一步细化。

### 阶段 0：工程基础设施

**目标：** 建立可独立开发、测试和持续集成的 Rust Workspace。

  步骤   实施内容               验收标准
  ------ ---------------------- --------------------------
  0.1    创建 Cargo Workspace   Cargo 可正常解析和构建
  0.2    创建七个 Crate         目录、职责和依赖关系明确
  0.3    定义公共基础类型规范   类型归属清晰，无重复定义
  0.4    配置统一错误处理       错误可分类、可追踪
  0.5    配置日志与 tracing     关键操作可记录
  0.6    配置单元、集成测试     测试可独立运行
  0.7    配置 CI、fmt、clippy   检查流程可重复执行

**阶段验收：**

-   七个 Crate 均可编译。
-   非 UI Crate 的依赖树中没有 GPUI。
-   Workspace 依赖方向符合架构边界。
-   每个 Crate 具有基础测试。
-   格式、静态检查和测试命令可稳定执行。

### 阶段 1：`md-core` 文本基础设施

**目标：** 提供统一、可靠的文本边界和基础几何类型。

  步骤   实施内容
  ------ -----------------------------------------------
  1.1    定义 TextOffset、TextRange、DocPos 等基础类型
  1.2    实现 TextBuf
  1.3    实现 UTF-8 字节边界验证
  1.4    实现 Grapheme Cluster 边界查询
  1.5    实现前后 Grapheme 移动
  1.6    实现 UTF-8 与 UTF-16 转换
  1.7    实现文本插入、删除和替换
  1.8    实现文本区间拆分和合并
  1.9    定义 Point、Rect、Size、EdgeInsets
  1.10   完善属性测试和边界测试

建议接口草案：

``` rust
pub struct TextBuf {
    // 内部封装 UTF-8 文本
}

impl TextBuf {
    pub fn len_bytes(&self) -> usize;

    pub fn is_char_boundary(&self, offset: usize) -> bool;

    pub fn next_grapheme_boundary(
        &self,
        offset: usize,
    ) -> Option<usize>;

    pub fn prev_grapheme_boundary(
        &self,
        offset: usize,
    ) -> Option<usize>;

    pub fn replace(
        &mut self,
        range: std::ops::Range<usize>,
        text: &str,
    ) -> Result<(), TextError>;
}
```

**阶段验收：**

-   UTF-8 边界检查正确。
-   中文、英文、Emoji、组合字符测试通过。
-   Grapheme 移动符合用户感知字符语义。
-   UTF-16 转换双向一致。
-   上层模块不能绕过 TextBuf 的边界检查。

### 阶段 2：`md-document` 文档模型

**目标：** 实现编辑器的核心数据结构，并保证节点身份、父子关系和 Inline
内容操作正确。

#### 2.1 NodeArena

  步骤    实施内容
  ------- -------------------------
  2.1.1   定义 BlockId
  2.1.2   定义 NodeSlot
  2.1.3   实现 NodeArena
  2.1.4   实现节点分配
  2.1.5   实现 Generation 校验
  2.1.6   实现节点释放和 FreeList
  2.1.7   实现事务暂存节点
  2.1.8   实现节点恢复
  2.1.9   实现节点回收策略

注意：Arena 节点释放与 History 生命周期耦合。Undo
仍可能需要节点快照时，不得提前回收相应资源。

#### 2.2 Block 数据模型

  步骤     实施内容
  -------- --------------------------------------
  2.2.1    定义 BlockNode
  2.2.2    定义 BlockKind
  2.2.3    实现 Paragraph
  2.2.4    实现 Heading
  2.2.5    实现 CodeBlock、HtmlBlock、MathBlock
  2.2.6    实现 Quote
  2.2.7    实现 List、ListItem
  2.2.8    实现 Table、TableRow、TableCell
  2.2.9    实现 ImageBlock、ThematicBreak
  2.2.10   实现父子关系约束

#### 2.3 Inline 数据模型

  步骤     实施内容
  -------- --------------------------
  2.3.1    定义 InlineContent
  2.3.2    定义 Run
  2.3.3    定义 Marks
  2.3.4    定义 MarkFlags
  2.3.5    定义 LinkRef
  2.3.6    定义 AtomKind
  2.3.7    实现 Text Run 插入和删除
  2.3.8    实现 Run Split / Merge
  2.3.9    实现 Atom 插入和删除
  2.3.10   实现 Inline Normalizer

#### 2.4 Document

-   实现 Document Root。
-   实现节点查询接口。
-   实现容器子节点查询。
-   实现受控结构修改接口。
-   实现 Document 快照和恢复所需的数据结构。
-   实现 Document Validator。

**阶段验收：**

-   所有 BlockKind 均可正确创建。
-   容器与叶子节点规则正确。
-   Arena 槽位复用后旧 ID 失效。
-   移动节点不改变 BlockId。
-   Inline Run 规范化正确。
-   随机结构操作后 Document 不变式成立。

### 阶段 3：DocumentIndex、DocPos 和 Selection

**目标：** 建立全文唯一坐标体系，为跨块编辑、IME、搜索、剪贴板和 Layout
提供统一位置语义。

#### 3.1 DocumentIndex

-   建立 `leaf_order`。
-   建立 `block_locations`。
-   建立 `parent_index`。
-   实现 Fenwick Tree。
-   实现叶子长度前缀和。
-   实现 DocPos 到叶子的快速定位。
-   实现索引重建。
-   实现索引一致性验证。

第一版先在事务提交后重建索引，验证正确性后再优化增量维护。

#### 3.2 DocPos 与 Position

-   实现 DocPos。
-   实现 Affinity。
-   实现 Position。
-   实现 BlockGapPosition。
-   实现叶子块长度计算。
-   实现虚拟 Block Separator。
-   实现 Separator 两端解析。
-   实现 DocPos 到 Position 转换。
-   实现 Position 到 DocPos 转换。
-   实现非法位置检测。

文档长度基线：

``` text
DocLength =
    sum(leaf_content_len)
    + max(leaf_count - 1, 0)
```

重点测试：

-   空文档。
-   空 Paragraph。
-   两个相邻空块。
-   中文文本。
-   Atom。
-   多层 Quote。
-   嵌套 List。
-   表格单元格。
-   Separator 的 Upstream / Downstream。

#### 3.3 Selection

-   实现 Anchor。
-   实现 Selection。
-   实现 Range。
-   实现 Node Selection。
-   实现 TableCells Selection。
-   实现 SelectionSet。
-   实现多选区排序。
-   实现 Selection 合法性校验。
-   实现 Selection 与 PositionMapping 的关联。

**阶段验收：**

对于可无损表示的位置，验证：

``` rust
to_doc_pos(
    resolve_position(doc_pos, affinity)
) == doc_pos
```

同时通过随机位置生成、边界转换和多选区属性测试。

### 阶段 4：`md-markdown` 解析与序列化

**目标：** 建立 Markdown 源码与内部 Document
模型之间的可靠转换，并实现设计要求的源码保真策略。

#### 4.1 Parser 与 Builder

1.  确定 pulldown-cmark 版本及方言配置。
2.  配置 CommonMark。
3.  配置 GFM 扩展。
4.  使用事件流解析。
5.  使用 `into_offset_iter()` 获取源码范围。
6.  实现 Markdown Builder。
7.  构建 Block Arena。
8.  构建 Inline Flat Run。
9.  解析 Marks。
10. 解析 Atom。
11. 保存 SourceHint。
12. 处理 YAML Frontmatter。
13. 处理脚注、数学和 HTML。

#### 4.2 Serializer

1.  实现 Text Writer。
2.  实现 Inline Writer。
3.  实现 Block Writer。
4.  实现 Marks 输出。
5.  实现列表序号计算。
6.  实现代码围栏输出。
7.  实现表格输出。
8.  实现 Raw 内容输出。
9.  实现 SourceHint pristine 输出。
10. 实现 dirty 节点重新序列化。
11. 实现容器重缩进。
12. 实现不可表示 Marks 的降级与提示。

**阶段验收：**

-   准备至少 200 篇真实 Markdown 文件。
-   未编辑文档实现 100% 逐字节相等。
-   执行模型闭合测试：

``` rust
serialize(parse(source)) == source
```

``` rust
parse(serialize(document)) ≅ document
```

-   执行随机模型闭合测试。
-   明确记录容器内空段落等已知保真限制，不得把限制隐藏在测试中。

### 阶段 5：`md-editor` 编辑内核

**目标：**
将用户操作转换为可验证、可撤销、可映射的文档事务。此阶段应保持
Headless，不依赖 GPUI。

#### 5.1 Intent 系统

实现的 Intent 类型至少覆盖：

-   InsertText
-   InsertNewline
-   DeleteBackward
-   DeleteForward
-   Move
-   ToggleMark
-   SetBlockKind
-   ToggleList
-   ChangeIndent
-   Paste
-   Undo
-   Redo
-   ImeSetMarked
-   ImeCommit
-   ImeCancel

具体类型可在接口设计阶段结合最终规则表调整。

#### 5.2 Rule Table

按照设计文档规则体系实现：

-   Enter Rules。
-   Backspace Rules。
-   DeleteForward Rules。
-   Tab / Shift+Tab Rules。
-   DeleteRange Rules。
-   Node Selection Rules。
-   多选区 Rules。
-   List Rules。
-   Quote Rules。
-   Table Rules。
-   Raw Block Rules。

要求：

-   每个 Rule 拥有独立 RuleId。
-   Enter、Backspace、DeleteForward 采用有序规则表。
-   不以大量嵌套 `match` 代替明确的规则优先级。
-   每条 Rule 至少配备 3 个测试用例。

#### 5.3 Operation

实现文档编辑原语：

-   ReplaceText。
-   SplitTextRun。
-   MergeTextRuns。
-   SetTextMarks。
-   InsertInline。
-   RemoveInline。
-   InsertBlock。
-   RemoveBlock。
-   MoveBlock。
-   SplitBlock。
-   JoinBlocks。
-   SetBlockKind。
-   SetNodeAttribute。

每个 Operation 必须能产生对应 inverse。

#### 5.4 PositionMapping

实现：

-   TextInsert Mapping。
-   TextDelete Mapping。
-   Run Split / Merge Mapping。
-   Inline Insert / Remove Mapping。
-   Block Insert / Remove Mapping。
-   Subtree Remove Mapping。
-   Block Split / Join Mapping。
-   Block Move Mapping。
-   Normalizer Mapping。

#### 5.5 Transaction

实现：

1.  Base Revision 检查。
2.  Preflight。
3.  Operation 执行。
4.  Inverse 收集。
5.  PositionMapping 组合。
6.  SourceHint 更新。
7.  Normalizer。
8.  DocumentIndex 更新。
9.  Validator。
10. Commit。
11. Rollback。
12. ChangeSet 发布。

**阶段验收：**

-   任意事务失败不能留下半修改状态。
-   Document、Selection、Index、History 保持一致。
-   节点移动不被错误映射为删除。
-   随机 Intent 全部 Undo 后恢复原文档和选区。
-   每个 RuleId 至少三个测试用例。

### 阶段 6：History、IME、剪贴板与 InputRules

**目标：**
让编辑内核具备可靠的真实文本输入能力，重点保障中文输入法和撤销语义。

#### 6.1 History

-   HistoryEntry。
-   Undo Stack。
-   Redo Stack。
-   Forward Operations。
-   Inverse Operations。
-   连续输入合并。
-   删除操作合并。
-   Selection 前后状态恢复。
-   IME Composition 作为一条历史记录。
-   History 内存预算。
-   Save Point。
-   节点快照生命周期管理。

默认历史记录上限建议为 1000 条，并设置可配置字节预算。

#### 6.2 IME

-   Composition 状态。
-   Marked Range。
-   Selected Range。
-   临时预编辑文本。
-   ImeSetMarked。
-   ImeCommit。
-   ImeCancel。
-   跨块预编辑。
-   UTF-16 坐标转换。
-   Layout 光标矩形查询。

#### 6.3 剪贴板

-   `text/plain`。
-   `text/markdown`。
-   `text/html` 输出。
-   Markdown 优先粘贴。
-   Plain Text 按空行拆分。
-   复制选区。
-   跨块复制。
-   多块粘贴。

HTML 粘贴属于后续扩展，不列入首个正式交付版本的完成条件。

#### 6.4 InputRules

-   `#` 标题。
-   `-` 列表。
-   `>` 引用。
-   围栏代码块。
-   常用格式快捷输入。

**阶段验收：**

-   中文拼音、五笔和手写输入验证。
-   候选切换和 Esc 取消。
-   跨块 Composition。
-   连续输入 Undo 合并。
-   IME Commit 一次 Undo 完整撤销。
-   多格式复制粘贴正确。

### 阶段 7：`md-layout` 布局引擎

**目标：**
将文档模型转换为稳定的布局几何，支持文本测量、自动换行、Hit-test、选区绘制和增量布局。

#### 7.1 测量系统

-   LayoutEnv。
-   ResolvedStyle。
-   MeasureStyle。
-   PaintStyle。
-   TextMeasurer。
-   FakeMeasurer。
-   Cluster Advances。
-   Line Metrics。
-   文本宽度缓存。
-   测量缓存失效。

#### 7.2 Inline Layout

-   Grapheme Cluster 分组。
-   Marks 样式解析。
-   行内 Atom 测量。
-   HardBreak。
-   SoftBreak。
-   自动换行。
-   CJK 禁则。
-   行尾悬挂空白。
-   软换行与中文混排。

#### 7.3 Block Layout

-   Paragraph。
-   Heading。
-   CodeBlock。
-   Quote。
-   List。
-   Table。
-   Image。
-   Raw Block。
-   ThematicBreak。

#### 7.4 几何映射

-   `position_at_point`。
-   `caret_rect`。
-   `rects_for_range`。
-   `selection_rects`。
-   `block_outer`。
-   `closest_x`。
-   `visible`。
-   `total_height`。

#### 7.5 增量布局与虚拟化

-   LayoutTree。
-   BlockLayout。
-   Layout Cache。
-   Layout Invalidation。
-   脏块重新布局。
-   可见区域计算。
-   ±1 屏预布局。
-   非可见块估算高度。
-   视口锚定。
-   全量布局接口。

**阶段验收：**

-   FakeMeasurer 结果确定。
-   Hit-test 和 caret 映射符合设计约束。
-   增量 Layout 与全量 Layout 逐字段相等。
-   随机编辑 5000 次验证增量等价。
-   可见范围布局正确。
-   滚动时不触发无意义的全文重排。

### 阶段 8：`md-render` GPUI 渲染层

**目标：** 将 Layout 结果绘制为可交互的桌面文档视图。GPUI
层负责呈现和输入适配，不负责编辑规则。

#### 8.1 GPUI 基础集成

-   创建 GPUI Application。
-   创建 Window。
-   创建编辑器 View。
-   实现自定义 Element。
-   实现 request_layout。
-   实现 prepaint。
-   实现 paint。

#### 8.2 Shaping

-   字体加载。
-   Font Fallback。
-   Glyph Shaping。
-   Cluster 与 Glyph 映射。
-   Shaping Cache。
-   中英文混排。
-   代码字体。
-   行内样式绘制。

#### 8.3 Paint 层级

按照设计文档的层级组织绘制：

1.  块背景和边框。
2.  装饰底层。
3.  选区。
4.  Glyph。
5.  装饰顶层。
6.  图片和原子元素。
7.  光标。
8.  浮层。

#### 8.4 交互

-   鼠标点击定位。
-   鼠标拖动选区。
-   双击选词。
-   键盘事件映射 Intent。
-   快捷键。
-   滚轮滚动。
-   平滑滚动。
-   自动滚动到光标。
-   光标闪烁。
-   选区绘制。
-   IME 平台输入适配。

#### 8.5 资源

-   异步图片加载。
-   图片解码。
-   图片占位。
-   图片加载失败处理。
-   资源完成后的布局失效。
-   拖放图片和文件。

**阶段验收：**

-   能够通过鼠标和键盘完成真实编辑。
-   GPUI Paint 不直接修改 Document。
-   Layout 独立提供光标、选区和 Hit-test 几何。
-   纯滚动不引起不必要的 request_layout。
-   光标闪烁只重绘光标区域。

### 阶段 9：`md-app` 桌面应用

**目标：** 将编辑器内核和渲染层装配为可独立运行的本地桌面应用。

#### 9.1 应用基础

-   Application 启动流程。
-   主窗口。
-   Window 生命周期。
-   编辑器状态管理。
-   应用配置管理。
-   最近打开文件。

#### 9.2 文件 IO

-   新建 Markdown。
-   打开 `.md` 文件。
-   打开 `.markdown` 文件。
-   UTF-8 文件读取。
-   文件内容解析。
-   保存文件。
-   另存为。
-   保存失败提示。
-   外部文件修改检测。
-   未保存状态提示。
-   关闭窗口前保存确认。
-   原子保存，避免中断写入导致文件损坏。

本地文件保存以 Serializer 输出为准；保存成功后更新 Save Point。

#### 9.3 菜单和快捷键

-   文件菜单。
-   编辑菜单。
-   格式菜单。
-   视图菜单。
-   主题菜单。
-   帮助菜单。
-   全局快捷键。
-   命令面板。

#### 9.4 主题和偏好设置

-   TOML 主题读取。
-   默认主题"墨"。
-   浅色主题"纸"。
-   高对比主题。
-   字体配置。
-   版心宽度。
-   每行字数。
-   行高。
-   主题热重载。
-   配置持久化。

**阶段验收：**

能够独立启动应用，打开本地 Markdown
文件，编辑后保存，退出应用，再次打开文件时内容完整一致。

### 阶段 10：产品功能完善

**目标：** 完成首个正式交付版本的主要阅读、导航和编辑体验。

  功能         实施任务
  ------------ --------------------------------------
  文档大纲     解析 Heading、构建大纲、点击跳转
  搜索         全文查找、匹配高亮、上一个/下一个
  折叠         标题折叠、Quote 折叠、List 折叠
  图片         路径处理、预览、异步加载
  表格         表格编辑、单元格选择、Tab 导航
  专注模式     隐藏干扰元素、突出激活块
  打字机滚动   激活块保持在视口中线
  命令面板     命令搜索、快捷操作
  状态栏       字数、块数、阅读时长、DocPos
  导出         Markdown、纯文本及设计支持的输出形式

**阶段验收：**

-   完成首个正式交付版本的功能范围。
-   所有功能通过集成测试。
-   不破坏已有编辑内核不变式。
-   所有功能边界和已知限制有明确说明。

### 阶段 11：性能、稳定性和正式发布

**目标：** 验证正确性、性能和跨平台可用性，完成可交付的桌面版本。

#### 11.1 正确性

-   200 篇真实 Markdown 往返测试。
-   10000 次随机模型闭合测试。
-   10000 次随机 Undo 完备性测试。
-   10000 次随机 Document 不变式测试。
-   5000 次增量布局等价测试。
-   全部 RuleId 测试覆盖。

#### 11.2 性能

-   10 万字文档输入。
-   5MB 文档加载。
-   10 万行文档滚动。
-   大量图片加载。
-   连续输入和删除。
-   长文搜索。
-   布局缓存命中率。
-   内存占用。
-   GPU 绘制负载。

#### 11.3 视觉与体验

-   主题对比度检测。
-   标记显露零布局抖动。
-   窄窗口适配。
-   中文排版。
-   光标位置精度。
-   选区边界。
-   IME 预编辑显示。
-   滚动锚定。
-   代码块横向滚动。

#### 11.4 发布

-   Windows 构建。
-   macOS 构建。
-   Linux 构建。
-   Release 优化。
-   安装包制作。
-   应用图标。
-   版本信息。
-   用户配置目录。
-   日志目录。
-   默认主题和字体资源。
-   安装、启动、打开、保存、退出全流程测试。

**阶段验收：**

-   发布构建可启动。
-   本地文件完整读写。
-   关键验收指标通过。
-   安装、升级或替换、启动和退出流程经过验证。
-   发布说明记录已知限制。

------------------------------------------------------------------------

## 5. 测试与质量管理

项目不采用"全部编码完成后再集中测试"的方式。每个小步骤都应包含实现和验证。

### 5.1 四层测试体系

  测试层级           测试内容                     执行时机
  ------------------ ---------------------------- ------------------------
  Unit Test          单个数据结构和算法           每个小步骤
  Integration Test   多模块协同                   每个子模块完成
  Property Test      随机输入、随机操作、不变式   核心模块完成后持续运行
  Benchmark          输入、布局、渲染和滚动       性能模块及发布前

### 5.2 质量验收基线

下表保留设计文档的验收 ID 和目标，作为项目质量门槛。

  ------------------------------------------------------------------------------------------------
  验收 ID                 指标                    门槛
  ----------------------- ----------------------- ------------------------------------------------
  AC-01                   未编辑保真              至少 200 篇真实 Markdown 逐字节相等，100%

  AC-02                   模型闭合                `parse(serialize(doc)) ≅ doc`，随机 10000 次通过

  AC-03                   撤销完备                随机 Intent 全部 Undo 后文档与选区恢复，10000
                                                  次通过

  AC-04                   不变式                  随机序列后 INV 全成立，10000 次通过

  AC-05                   增量等价                增量 Layout 与全量 Layout 逐字段相等，随机编辑
                                                  5000 次

  AC-06                   规则覆盖                每条 RuleId 至少 3 条测试用例

  AC-07                   输入延迟                10 万字文档，Intent → layout.sync P95 \< 1ms

  AC-08                   滚动                    10 万行虚拟化滚动 ≥60fps

  AC-09                   分层                    非 UI Crate 依赖树无 GPUI

  AC-10                   IME                     中文拼音 /
                                                  五笔、候选切换、取消、跨块预编辑无异常

  AC-11                   对比度                  全部主题达到设计文档对比度门槛；校正后复检通过

  AC-12                   零抖动                  随机光标移动 5000 次，标记显露前后 LayoutTree
                                                  哈希不变

  AC-13                   热重载                  仅修改绘制字段时，全量重排次数为 0
  ------------------------------------------------------------------------------------------------

性能指标必须在明确的硬件、操作系统、构建模式和文档规模下测量。AC-07 的
1ms 指标针对 `Intent → layout.sync`，不能误解为所有 GPUI 绘制和 GPU
提交都必须在 1ms 内完成。

### 5.3 规则夹具

每个编辑行为用例声明：

``` text
name / rule / before / intent / after
```

简单场景使用带光标的 Markdown；涉及空块和结构的场景使用树
DSL，避免空行歧义。

建议记号：

-   光标：`|`
-   选区：`[|…|]`
-   树
    DSL：`P{}`、`H1{}`、`Q{}`、`UL{}`、`OL{}`、`LI{}`、`LI[x]{}`、`CB{}`、`TBL{}`、`TC{}`、`HR{}`

覆盖矩阵至少包括：

-   Enter：块类别 × P0/Pm/Pn/Empty × 父容器 × 折叠 / 选区。
-   Backspace：同上，并增加前一块 Inline / Raw / Atomic / None。
-   Tab：规则 × Tab / Shift+Tab × 折叠 / 单选区 / 跨项选区。
-   IME：不同输入法、候选切换、取消、跨块预编辑。
-   UI：窄窗口断点、gutter 显隐、标记 overlay 边界、主题热重载。

------------------------------------------------------------------------

## 6. 风险控制

  -----------------------------------------------------------------------------
  风险                    可能后果                 控制措施
  ----------------------- ------------------------ ----------------------------
  Arena 提前回收          Undo 恢复失败、陈旧 ID   节点暂存与 History
                                                   生命周期协同

  DocPos 边界不清         光标跳动、跨块删除错误   统一坐标转换与属性测试

  SourceHint 失效不正确   Markdown 源码被意外改写  保守 dirty 策略

  PositionMapping 不完整  编辑后光标位置错误       每个 Operation 提供 Mapping

  IME 与普通编辑混合      中文输入重复、丢字       独立 Composition 状态

  增量布局不一致          长文显示错乱             始终与全量布局进行对照

  GPUI 侵入核心模块       无法独立测试编辑内核     严格限制 Crate 依赖方向

  过早优化                开发复杂度失控           先保证正确，再进行性能优化
  -----------------------------------------------------------------------------

项目执行时坚持：

1.  不为了快速看到界面而跳过文档模型。
2.  不把编辑行为直接写在 GPUI 事件回调中。
3.  测试未通过时不进入依赖该能力的下一阶段。
4.  先保证正确性，再进行性能优化。
5.  对设计文档已有的限制保持透明，不擅自扩大首个正式交付版本的范围。

------------------------------------------------------------------------

## 7. 项目文档与进度管理

建议持续维护以下文档：

``` text
docs/
├── DESIGN.md
│   └── 总体架构与模块边界
├── IMPLEMENTATION.md
│   └── 总体实施计划与进度
├── ARCHITECTURE_DECISIONS.md
│   └── 架构决策记录
├── MODULE_DESIGN/
│   ├── MD_CORE.md
│   ├── MD_DOCUMENT.md
│   ├── MD_MARKDOWN.md
│   ├── MD_EDITOR.md
│   ├── MD_LAYOUT.md
│   ├── MD_RENDER.md
│   └── MD_APP.md
├── TESTING.md
│   └── 测试规范与验收结果
└── CHANGELOG.md
    └── 版本变更记录
```

每完成一个阶段，更新 `IMPLEMENTATION.md`，至少记录：

-   已完成步骤。
-   当前实现接口。
-   测试结果。
-   已知问题。
-   下一步任务。
-   与设计文档不一致的地方。

设计基线和实施进度必须分开维护。代码已经写出不等于已经通过验收。

### 7.1 架构变更治理

-   Rule ID、验收 ID 一经发布不得复用或改义；废弃时标记 deprecated。
-   新编辑行为必须先更新规则，再实现代码与测试。
-   架构决策变更必须记录决策理由、影响 Crate、缓存失效范围和迁移计划。
-   UI 数值可通过主题或偏好调整，但不得破坏 Layout / Render / Style
    不变量。
-   总体设计文档定义设计基线，不作为已实现状态的证明。

------------------------------------------------------------------------

## 8. 开发推进流程

项目采用逐步实施、逐步验收的推进方式，不一次性生成大量未经验证的代码。

### 8.1 每个小步骤的固定流程

1.  **详细设计：** 说明目标、数据结构、接口、依赖关系和边界条件。
2.  **编写实现：** 给出完整 Rust 代码、模块路径和必要依赖配置。
3.  **编写测试：** 提供单元测试、边界测试和必要的属性测试。
4.  **本地验证：** 运行 `cargo test`、`cargo check`、`cargo clippy`
    等命令，并检查结果。
5.  **代码审查与验收：** 检查实现是否符合设计、是否存在遗漏和潜在问题。
6.  **更新进度并停止：**
    验收通过后更新实施记录，停在当前步骤，等待进入下一步。

### 8.2 推进规则

-   不一次性生成大量文件，避免难以验证。
-   不同时实现多个尚未稳定的模块。
-   不在前置接口不稳定时强行推进后续模块。
-   发现架构问题时先修复设计和实现，再继续。
-   已完成并通过测试的工作不重复实现。
-   进入每个新步骤前，先核对当前代码和既有验收状态。

------------------------------------------------------------------------

## 9. 工作量估算

以下为任务拆分数量估算，不是固定工期，也不代表所有步骤复杂度相同。

  阶段       模块                           预计小步骤
  ---------- -------------------- --------------------
  0          工程基础设施                         5～8
  1          `md-core`                           8～12
  2          `md-document`                      20～28
  3          DocPos / Selection                 12～18
  4          `md-markdown`                      12～18
  5          `md-editor`                        25～35
  6          History / IME                      12～18
  7          `md-layout`                        20～28
  8          `md-render`                        20～28
  9          `md-app`                           15～22
  10         产品功能                           12～20
  11         性能和发布                         15～25
  **总计**   **完整项目**           **约 176～260 步**

建议将工作量分成三个重心阶段：

-   前期：文档模型、位置系统、Markdown
    解析和编辑事务，优先保证编辑语义正确。
-   中期：History、IME、Layout、Render，让编辑器真正可用。
-   后期：桌面应用、完整功能、性能优化和发布。

上述比例属于项目规划建议，不是设计文档中的硬性指标。

------------------------------------------------------------------------

## 10. 最终交付验收清单

正式发布前，逐项确认：

### 核心架构

-   [ ] 七个 Crate 职责明确。
-   [ ] 非 UI Crate 不依赖 GPUI。
-   [ ] Block Arena、Flat Run、DocPos 核心决策得到落实。
-   [ ] Document、Index、Selection 不变式通过测试。

### Markdown 与编辑内核

-   [ ] 200 篇真实 Markdown 未编辑保真测试通过。
-   [ ] 模型闭合随机测试通过。
-   [ ] Enter、Backspace、Delete、Tab 等规则覆盖完成。
-   [ ] Transaction 具备原子提交和失败回滚。
-   [ ] PositionMapping 覆盖所有编辑 Operation。
-   [ ] Undo / Redo 完整恢复文档和选区。
-   [ ] 中文 IME 测试通过。

### Layout 与 Render

-   [ ] 自动换行和中英文混排正常。
-   [ ] 光标和选区几何映射正确。
-   [ ] 增量布局与全量布局等价。
-   [ ] 长文档虚拟化正常。
-   [ ] 标记显露不改变布局几何。
-   [ ] 绘制字段热重载不触发全量重排。

### 桌面应用

-   [ ] 可独立启动。
-   [ ] 可新建、打开、编辑、保存和另存为 Markdown 文件。
-   [ ] 未保存状态和保存失败提示正确。
-   [ ] 可恢复最近打开文件等应用状态。
-   [ ] 主题和偏好设置可持久化。
-   [ ] 搜索、大纲、折叠和命令面板可用。

### 发布

-   [ ] 目标平台 Release 构建成功。
-   [ ] 安装包可正常安装和启动。
-   [ ] 默认资源和主题完整。
-   [ ] 文件读写、退出和再次打开流程通过。
-   [ ] 已知限制和发布说明完整。

------------------------------------------------------------------------

## 11. 项目执行原则与最终目标

本项目的重点不是尽快实现一个能够输入 Markdown
的界面，而是先建立可靠的编辑内核。

实施过程中应坚持：

-   **架构先行：** 以 `DESIGN.md` 为设计基线。
-   **自底向上：** 从文本、文档模型和坐标系统开始。
-   **正确性优先：** 先实现可验证的正确行为，再优化性能。
-   **Headless 优先：** 编辑内核先脱离 GPUI 独立测试。
-   **逐步验收：** 每个小步骤完成后停下来验证。
-   **进度可追踪：** 通过独立实施文档记录真实完成状态。

最终交付目标是：

> 一个拥有独立编辑内核、Markdown
> 源码保真、中文输入可靠、长文档性能良好，并且可以日常使用的本地
> Markdown 桌面编辑器。

**建议的下一步：**
将现有代码与本实施计划逐项核对，更新已完成状态，确定当前准确的实施位置，然后从下一个未验收的小步骤继续。

------------------------------------------------------------------------

## 12. 实施进度

> 本节记录真实实现状态，与设计基线分开维护。`DESIGN.md` 的定义不因本节勾选而改变；代码已写出不等于已经通过阶段验收。

### 阶段 0：工程基础设施（已完成，已确认）

**完成日期：** 2026-11-23

| 步骤 | 状态 | 产物 |
|---|---|---|
| 0.1 创建 Cargo Workspace | 完成 | virtual workspace、`md-app` 应用入口、`docs/CHANGELOG.md` |
| 0.2 创建七个 Crate | 完成 | `md-core`～`md-render` 六个新 crate + `md-app`；单向依赖边 |
| 0.3 定义公共基础类型规范 | 完成 | `docs/TYPE_OWNERSHIP.md`、ADR-0002 |
| 0.4 配置统一错误处理 | 完成 | 各库 `thiserror` 错误、`md-app` 的 `anyhow` 边界、`docs/ERROR_HANDLING.md` |
| 0.5 配置日志与 tracing | 完成 | `md-app::logging`、`RUST_LOG` 支持、`docs/OBSERVABILITY.md` |
| 0.6 配置单元、集成测试 | 完成 | 20 个测试、`app_smoke` 集成测试、`docs/TESTING.md` |
| 0.7 配置 CI、fmt、clippy | 完成 | Workspace lints、`rust-toolchain.toml`、三平台 CI、分层检查脚本、`docs/DEVELOPMENT.md` |

#### 当前接口与结构

- Workspace 成员：`crates/*`，七个 crate；`md-app` 同时产出 lib 与 `coti` bin。
- 依赖方向：`md-core → md-document → {md-markdown, md-editor, md-layout} → md-render → md-app`。
- 公共错误：`TextError`、`DocumentError`、`PositionError`、`MarkdownError`、`EditorError`、`LayoutError`、`RenderError`；应用边界为 `anyhow::Result`。
- 日志：`md_app::logging::init()`，默认 `info`，从 `RUST_LOG` 读取过滤级别。
- GPUI：阶段 0～7 不进入 active dependency graph；revision 固定在 ADR-0001。

#### 验证结果

- `cargo fmt --all -- --check`：通过。
- `cargo clippy --workspace --all-targets --locked -- -D warnings`：通过。
- `cargo test --workspace`：20 个测试通过。
- `cargo run -p md-app`：输出启动信息，退出码 0。
- `./scripts/check-layering.ps1`：`layering: OK (no GPUI below md-render)`。
- CI workflow 已写入 `.github/workflows/ci.yml`，尚未在 GitHub 远端实际执行；本地等价检查已通过。

#### 已知问题与偏差

- GPUI 延后到阶段 8；`Cargo.lock` 当前不含 GPUI。阶段 8 必须重新锁定并验证三平台构建（ADR-0001）。
- `IMPLEMENTATION.md` 1.1 与 `DESIGN.md` 2.2 对 `DocPos` 归属冲突，按 `md-document` 处理（ADR-0002）；阶段 1 不实现 `DocPos`。
- 空模块阶段使用 `use <crate> as _;` 维持依赖边；模块实现后应删除这些临时标记。
- 属性测试与基准测试尚未接入；计划在阶段 1 引入 `proptest`，阶段 7/11 引入基准。
- 日志时间戳目前为 UTC；本地时区显示留待发布阶段处理。

#### 下一步

- 阶段 1：`md-core` 文本基础设施。建议从 1.1 开始：`TextOffset`、`TextRange`，按 ADR-0002 不包含 `DocPos`；完成后再进入 `TextBuf` 与 UTF-8/grapheme 工具。

### 阶段 1：`md-core` 文本基础设施（已完成，等待确认）

**完成日期：** 2026-11-23

| 步骤 | 状态 | 产物 |
|---|---|---|
| 1.1 定义 TextOffset、TextRange | 完成 | newtype、半开区间、`Range<usize>` 转换 |
| 1.2 实现 TextBuf | 完成 | 内部 `String`、受控访问、显示与转换 |
| 1.3 UTF-8 字节边界验证 | 完成 | `is_char_boundary`、`validate_offset`、`validate_range`、`slice` |
| 1.4 Grapheme Cluster 边界查询 | 完成 | `is_grapheme_boundary`，基于 `unicode-segmentation` |
| 1.5 前后 Grapheme 移动 | 完成 | `next_grapheme_boundary`、`prev_grapheme_boundary` |
| 1.6 UTF-8 与 UTF-16 转换 | 完成 | `to_utf16`、`from_utf16`、双向 offset 转换 |
| 1.7 文本插入、删除、替换 | 完成 | `insert`、`remove`、`replace`，失败不改缓冲区 |
| 1.8 文本区间拆分与合并 | 完成 | `split_off`、`append`、`append_buf`、`concat`、`TextRange::split_at` |
| 1.9 Point、Rect、Size、EdgeInsets | 完成 | `f32` 几何原语、包含/相交/并集/inset/平移 |
| 1.10 属性测试与边界测试 | 完成 | 7 个 `proptest` 属性 + CJK/Emoji/组合字符边界测试 |

#### 当前接口

- `md_core::{TextOffset, TextRange, TextBuf, TextError, TextResult}`。
- `md_core::{Point, Size, Rect, EdgeInsets}`。
- 文本 API：边界验证、切片、插入/删除/替换、拆分/合并、grapheme 导航、UTF-8/UTF-16 转换。
- 依赖：`thiserror`、`unicode-segmentation`；dev-dependency：`proptest`。
- `DocPos` 尚未实现，按 ADR-0002 留在 `md-document`。

#### 验证结果

- `cargo fmt --all -- --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo test --workspace`：56 个测试通过。
  - `md-core` 32 个单元测试 + 7 个属性测试。
- 属性测试覆盖 UTF-16 往返、offset 往返、grapheme 全量遍历、替换/插入删除/split-concat 往返。
- `./scripts/check-layering.ps1`：通过；`md-core` 依赖树仍无 GPUI。

#### 已知问题与偏差

- `TextBuf` 基于 `String`，插入/删除为 O(n)；正确性优先，长文性能优化在布局/编辑阶段用基准决定是否引入 rope。
- `TextRange` 允许暂时 reversed；调用方必须使用 `TextBuf::validate_range` 或 `TextRange::is_valid`，不能直接切片。
- `GraphemeCursor` 当前使用完整文本、`chunk_start = 0`；尚未做分块增量。
- `Rect::inset` 可产生负尺寸，几何层不 clamp；布局层必须显式处理。
- Windows 工作区仍有 Git 的 LF→CRLF 提示；提交内容为 LF，不影响构建。

#### 下一步

- 阶段 2：`md-document`。建议按 2.1 NodeArena/Generation/FreeList、2.2 BlockKind、2.3 InlineContent/Run/Marks、2.4 Document/Validator 四批实施，最后做随机结构不变式测试。

---

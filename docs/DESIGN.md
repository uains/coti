# Markdown 编辑器总体设计

> **产品形态：** Rust + GPUI 桌面端、单栏 WYSIWYG Markdown 编辑器  
> **文档状态：** 最终设计规范  
> **用途：** 定义产品范围、系统架构、文档模型、编辑语义、布局渲染、界面主题及质量验收要求。  
> **实现状态：** 本文描述目标设计，不代表相关功能已经实现或通过测试。具体依赖版本、GPUI 平台 API 和性能数据须在工程实施阶段验证。  
>
> 规范用语：MUST / MUST NOT / SHOULD / MAY 分别表示必须、禁止、应当、可以。

---

## 0. 核心架构原则

编辑器采用以下统一架构：

| 领域 | 最终方案 | 主要目的 |
|---|---|---|
| Block 存储 | Block Arena + Generation 化 BlockId | 稳定节点身份、支持结构编辑和安全回收 |
| Inline 存储 | Flat Run + Marks + Atom | 高效处理连续文本、格式范围和原子内容 |
| 全文位置 | DocPos 唯一权威线性坐标 | 统一编辑、选区、IME、搜索和布局位置 |
| 编辑执行 | Intent → Rule → Operation → Transaction | 使编辑行为可测试、可撤销且具备原子性 |
| 布局 | 独立 Layout Engine + TextMeasurer 端口 | 将文档语义与文字测量、几何布局分离 |
| UI | GPUI 仅位于 Render / App 边界 | 保证核心模型和编辑内核可 Headless 测试 |

### 0.1 Block Arena

- Block 层采用 Arena 存储。
- `BlockId` 使用 `index + generation`，用于检测槽位复用后的陈旧引用。
- Document Root 由 Document 自身管理，不分配 BlockId。
- Paragraph、Heading、CodeBlock、Blockquote、List、ListItem、Table、TableRow、TableCell 等块级结构节点拥有独立 BlockId。
- ListItem、TableRow、TableCell 等结构容器也拥有独立 BlockId。
- Inline Text、Link、Image、InlineMath、HardBreak、SoftBreak、FootnoteRef 等不分配 BlockId。
- 节点移动保留原 BlockId；移动不能被映射成节点内容已删除。
- 子树删除、暂存、恢复和槽位回收必须与 Transaction / History 生命周期协同。
- Arena 不向 Editor、Layout、Render 暴露可任意修改的内部存储；上层通过 Document 受控接口访问。

### 0.2 Inline Flat Run + Marks + Atom

```rust
pub struct InlineContent {
    pub runs: Vec<Run>,
    pub len: usize,
}

pub enum Run {
    Text {
        text: TextBuf,
        marks: Marks,
    },
    Atom {
        kind: AtomKind,
        marks: Marks,
    },
}

pub struct Marks {
    pub flags: MarkFlags,
    pub link: Option<LinkRef>,
}
```

- 普通 Text Run 不分配稳定 ID。
- Atom 不分配独立 ID，通过所在 BlockId 和局部 Offset 定位。
- Link 在编辑语义上是可作用于文本区间的格式属性，在数据结构上由 `LinkRef` 表达。
- CODE 可以与其他 Marks 组合；内部模型不强制互斥。
- Atom 的 Marks 规则单独定义；具体哪些组合对某类 Atom 有意义，由 Atom 语义和 Serializer 规则校验。
- 相邻 Marks 完全相同的 Text Run 应合并；空 Text Run 应删除。
- 规范化不得改变文本与 Atom 的逻辑序列。

### 0.3 DocPos 权威坐标

- DocPos 是全文唯一权威线性坐标。
- 文本局部偏移以 UTF-8 字节为单位。
- Atom 占 1 个位置单位。
- 相邻叶子块之间的 Block Separator 占 1 个位置单位。
- 最后一个叶子块之后不增加尾部分隔符。
- 容器节点不直接贡献长度。
- Block Separator 是结构边界，不是普通文本 `\\n`，也不是 HardBreak Atom。
- 光标移动按 Grapheme Cluster 计算，不得对 DocPos 直接执行简单的 `+1/-1` 作为用户级字符移动。
- UTF-16 转换集中在 md-core 的文本坐标工具中，供 IME 和平台边界调用。
- `Affinity` 用于表达块边界、软换行等位置歧义。

---

## 1. 产品目标、范围与非目标

### 1.1 产品定位

这是一个以长文写作为核心的桌面 Markdown
编辑器。编辑态即阅读态：用户直接在排版后的文档中输入，Markdown
结构由内部文档模型维护，源码标记按需显露。默认界面是一张安静、连续的"纸"，而不是
IDE 式多面板工作台。

核心目标：

1.  编辑可靠：光标、选区、中文输入法、撤销重做和跨块编辑行为确定且可测试。
2.  源码保真：未编辑文档保存时逐字节保留原文；局部编辑尽量沿用原有标记风格。
3.  架构隔离：模型、编辑器和布局可脱离 GPUI 做 headless 测试；UI
    框架只存在于渲染与应用装配边界。
4.  长文性能：目标规模为 10 万字、5MB、10 万行；可见区域优先布局与绘制。
5.  纸面优先：界面 chrome
    不与正文争夺注意力；主题和标记显露不得造成布局抖动。

### 1.2 首个正式交付范围

-   桌面端单栏 WYSIWYG Markdown 编辑器。
-   CommonMark + GFM
    常用能力：标题、段落、强调、删除线、链接、图片、列表、任务列表、引用、代码块、表格、脚注、数学源码块、HTML
    源码块、YAML frontmatter。
-   中文 IME、剪贴板多格式、撤销重做、搜索、折叠、图片异步加载。
-   未编辑文档 `serialize(parse(source)) == source` 逐字节相等。
-   10 万字文档输入链路目标 P95 \< 1ms（Intent 到 `layout.sync`
    完成，不含 GPU 提交）。
-   10 万行文档虚拟化滚动目标 ≥ 60fps。

### 1.3 首个正式交付版本的非目标

-   双栏源码/预览模式、移动端、实时协同、插件系统。
-   完整 HTML 浏览器级渲染；HTML 块以可折叠 Raw 源码展示。
-   数学公式排版；首个正式交付版本以等宽源码展示。
-   表格单元格内嵌套块（列表、代码块等）。
-   完整 bidi / 双向文字精确定位、完整无障碍认证。
-   任意容器内部空段落的无损往返（明确列为已知限制）。

------------------------------------------------------------------------

## 2. Workspace 与 crate 架构

### 2.1 Crate 划分

``` text
crates/
  md-core       几何基础类型、TextBuf、UTF-8/grapheme/UTF-16 工具
  md-document   Arena 节点模型、DocPos、Position、Selection、索引、不变式
  md-markdown   Markdown parse / serialize、方言、SourceHint
  md-editor     Editor、Intent、Rule Table、Transaction、History、IME
  md-layout     纯几何布局、TextMeasurer 端口、双向位置映射、虚拟化
  md-render     GPUI Element、输入适配、shaping、ViewState、StyleResolver、资源
  md-app        窗口、菜单、命令面板、文件 IO、主题管理与应用装配
```

### 2.2 依赖方向

``` text
md-core
  ├── md-document ── md-markdown
  │        ├── md-editor
  │        └── md-layout
  └────────────────┴── md-render ── md-app
```

硬约束：

1.  `md-core / md-document / md-markdown / md-editor / md-layout`
    不得依赖 GPUI；CI 检查依赖树。
2.  `md-layout` 不认 keycode、颜色、GPUI 句柄或字体资源，只认
    `ResolvedStyle` 与测量端口。
3.  `md-render::paint` 不得访问 Document，不得修改模型状态。
4.  几何类型在 `md-core` 定义（`f32`）；仅在 `md-render` 边界转换为 GPUI
    类型。
5.  `md-document` 与 `md-layout` 不含视觉常量；所有视觉样式由
    StyleResolver 注入。

### 2.3 各层职责

  --------------------------------------------------------------------------------------------
  层                      负责                                         禁止
  ----------------------- -------------------------------------------- -----------------------
  md-core                 安全文本操作、字符边界、几何原语             文档结构、UI

  md-document             文档树、坐标、索引、选区类型、不变式         Markdown 语法、GPUI

  md-markdown             解析、序列化、源码风格提示                   编辑交互、绘制

  md-editor               Intent、规则分派、事务、History、IME         keycode、像素布局

  md-layout               换行、块几何、hit-test、位置映射、增量缓存   glyph shaping、颜色、UI

  md-render               GPUI 输入适配、shaping、paint、滚动与资源    直接修改 Document

  md-app                  生命周期、窗口、文件、菜单、主题装配         业务编辑规则
  --------------------------------------------------------------------------------------------

------------------------------------------------------------------------

## 3. Document 数据模型

### 3.1 总体结构

Document 采用 Block Arena + Inline Flat Run 的混合模型。

``` text
Document
├── root: Vec<BlockId>
├── arena: NodeArena
└── index: DocumentIndex

Block Arena
├── Paragraph(BlockId)
│   └── InlineContent(Vec<Run>)
├── Heading(BlockId)
│   └── InlineContent(Vec<Run>)
├── Blockquote(BlockId)
│   └── Vec<BlockId>
├── List(BlockId)
│   └── Vec<ListItem BlockId>
├── ListItem(BlockId)
│   └── Vec<BlockId>
└── Table(BlockId)
    └── TableRow BlockId
        └── TableCell BlockId
            └── InlineContent(Vec<Run>)
```

`root` 是根级块列表，不是 Arena 节点。根级节点的 `parent` 为
None。非根容器通过子节点 ID 持有子节点。

### 3.2 BlockId 与 Arena

逻辑类型：

``` rust
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockId {
    pub index: u32,
    pub generation: u32,
}
```

Arena 的概念结构：

``` rust
pub struct NodeArena {
    slots: Vec<NodeSlot>,
    free_list: Vec<u32>,
}

pub struct NodeSlot {
    generation: u32,
    node: Option<BlockNode>,
}

pub struct BlockNode {
    id: BlockId,
    parent: Option<BlockId>,
    kind: BlockKind,
    source: SourceHint,
}
```

约束：

-   `BlockId` 必须包含 generation；槽位复用后旧 ID 必须失效。
-   `Document` 是 Arena 的唯一所有者。
-   Arena 内部槽位和 free list 不得暴露给上层模块。
-   `get(BlockId)` 必须同时验证 index 和 generation。
-   无效或陈旧 ID 返回明确错误 / None，不得 panic。
-   节点移动仅修改父容器关系和 parent 字段，不改变节点 ID。
-   容器必须能通过受控接口读取其子节点。
-   非 root 节点必须且只能被一个父容器引用。
-   Arena 不允许存在不可达孤儿节点；事务执行中的暂存节点必须由明确的
    transaction-owned staging 机制管理。

建议 API（具体签名可在 Document 模块详细设计阶段调整）：

``` rust
impl Document {
    pub fn node(&self, id: BlockId) -> Option<&BlockNode>;
    pub fn parent(&self, id: BlockId) -> Option<BlockId>;
    pub fn children(&self, id: BlockId) -> Option<&[BlockId]>;
    pub fn root_children(&self) -> &[BlockId];
}
```

结构修改只通过受控 mutation API 或 Transaction 内部接口完成。不要向
Editor 暴露 `&mut NodeArena`。

### 3.3 BlockKind

采用叶子 / 容器互斥模型：

``` rust
pub enum BlockKind {
    // 行内叶子
    Paragraph(InlineContent),
    Heading {
        level: u8,
        content: InlineContent,
    },
    TableCell {
        content: InlineContent,
    },

    // Raw 叶子
    CodeBlock {
        lang: Option<String>,
        text: TextBuf,
        fence: FenceStyle,
    },
    HtmlBlock(TextBuf),
    MathBlock(TextBuf),
    FrontMatter(TextBuf),

    // 容器：只持 BlockId
    Quote(Vec<BlockId>),
    List {
        ordered: bool,
        start: u32,
        tight: bool,
        items: Vec<BlockId>,
    },
    ListItem {
        task: Option<bool>,
        children: Vec<BlockId>,
    },
    Table {
        aligns: Vec<Align>,
        rows: Vec<BlockId>,
    },
    TableRow {
        cells: Vec<BlockId>,
    },

    // 原子块
    ThematicBreak,
    ImageBlock {
        image: ImageRef,
        alt: String,
    },
}
```

容器类型约束：

-   `List.items` 只能指向 ListItem。
-   `Table.rows` 只能指向 TableRow。
-   `TableRow.cells` 只能指向 TableCell。
-   Quote 和 ListItem 的 children 按 首个正式交付版本的规则持有块级子节点。
-   TableCell 只支持 InlineContent，不支持嵌套块。
-   容器本身不贡献 DocPos 长度；其后代叶子按文档前序顺序贡献长度。

### 3.4 InlineContent、Run、Marks

``` rust
pub struct InlineContent {
    runs: Vec<Run>,
    len: usize,
}

pub enum Run {
    Text {
        text: TextBuf,
        marks: Marks,
    },
    Atom {
        kind: AtomKind,
        marks: Marks,
    },
}

pub struct Marks {
    flags: MarkFlags,
    link: Option<LinkRef>,
}

pub enum AtomKind {
    HardBreak,
    SoftBreak,
    Image(ImageRef),
    InlineMath(TextBuf),
    FootnoteRef(String),
}
```

Marks：

-   `MarkFlags` 至少包括
    `STRONG | EMPHASIS | STRIKE | UNDERLINE | CODE | HIGHLIGHT`。
-   `LinkRef` 保存链接 destination 与可选
    title；是否通过引用表去重由实现阶段决定。
-   Link 在行为上属于文本区间格式，在存储上由 `Marks.link` 表达。
-   CODE 可以与其他 Marks 组合；内部模型不因 Markdown
    某种语法方言的限制而丢失格式信息。
-   Serializer 根据目标方言决定输出嵌套、降级或提示，不得在 Document
    内部静默丢弃 Marks。

Atom 身份规则：

-   Atom 不分配 BlockId 或全局 InlineNodeId。
-   Atom 通过 `(BlockId, local_offset)` 定位。
-   Atom 在位置模型中占 1 个单位，且不可拆分。
-   若未来异步资源管理、对象级注释或外部引用确实需要稳定身份，再引入独立
    AtomId；不应将这项未来可能性变成首个正式交付版本的前置复杂度。

规范化规则：

1.  删除空 Text Run。
2.  合并相邻且 Marks 完全相同的 Text Run。
3.  不跨 Atom 合并 Text Run。
4.  不改变 Text 和 Atom 的逻辑顺序。
5.  重算 `InlineContent.len`。
6.  Marks 组合是否合法由类型语义和 Validator 检查；不得沿用"CODE
    必须清空所有其他 Marks"的旧限制。
7.  Atom 可用 Marks 由 AtomKind 规则定义；不要无条件将所有 Atom Marks
    清空。

### 3.5 文本与长度

-   `TextBuf` 封装 UTF-8 文本操作，禁止调用方裸切片绕过边界检查。
-   Text Run 的长度是 UTF-8 字节数。
-   Atom 的长度为 1。
-   `InlineContent.len` 是该 InlineContent 在 DocPos
    空间中的长度，即所有 Text UTF-8 字节长度与 Atom 单位之和。
-   TextOffset 必须是合法 UTF-8 字符边界。
-   Atom 内部不存在可定位的中间 offset。

------------------------------------------------------------------------

## 4. DocPos 与位置系统

### 4.1 DocPos 定义

``` rust
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DocPos(pub usize);

pub enum Affinity {
    Upstream,
    Downstream,
}

pub struct Position {
    pub block_id: BlockId,
    pub offset: usize,
    pub affinity: Affinity,
}
```

DocPos 是唯一权威全文坐标。`Position` 是通过 DocumentIndex
解析得到的结构化定位结果。

DocPos 是"UTF-8 字节为基础的混合坐标"，不是 Unicode
字符数，也不是纯文本字符串的字节下标：Text 使用 UTF-8 byte，Atom 和
Block Separator 各占一个逻辑单位。

### 4.2 叶子长度与 Block Separator

定义叶子块的内容长度：

``` text
leaf_content_len =
    InlineContent.len       // Paragraph / Heading / TableCell
    TextBuf.len_bytes()     // CodeBlock / HtmlBlock / MathBlock / FrontMatter
    1                       // ThematicBreak / ImageBlock 等单位置原子块
```

对于按文档前序排列的 N 个叶子块：

``` text
DocLength = sum(leaf_content_len[i]) + max(N - 1, 0)
```

也就是说：

-   每两个相邻叶子块之间有且只有一个逻辑 Block Separator。
-   最后一个叶子块之后不增加尾部分隔符。
-   空叶子块内容长度为 0，但它仍然是一个结构节点。
-   空文档的 DocLength 为 0；Document 根级至少保留一个空 Paragraph
    的不变式生效后，正常可编辑文档将有一个可定位叶子。

示例：

``` text
Paragraph A = "Hi"    UTF-8 长度 2
Paragraph B = "世界"  UTF-8 长度 6

DocPos 线性空间：
"Hi" + BlockSeparator + "世界"
长度 = 2 + 1 + 6 = 9
```

逻辑坐标示意：

``` text
0 1 2 | 3 4 5 6 7 8 9
H i   | 世 界
      ^
      separator 占据 [2, 3)
```

Block Separator：

-   是两个叶子块之间的结构边界。
-   不属于任何 InlineContent。
-   不等同于普通 `\n`。
-   不等同于 HardBreak / SoftBreak Atom。
-   删除它必须由结构规则执行，例如 JoinParagraph、JoinCompatibleBlocks
    或特定容器规则，而不是普通 Text 删除。
-   在跨块选区、全文搜索和剪贴板文本导出中，可根据调用场景将它呈现为换行，但不能因此改变
    Document 内部坐标语义。

### 4.3 合法边界与 Affinity

合法 DocPos 边界包括：

-   Text Run 内所有 UTF-8 字符边界。
-   Atom 前后边界，不包括 Atom 内部。
-   Block Separator 两端。
-   文档起点和文档末尾。

对于相邻叶子块 A、B：

-   A 内容末尾对应 Separator 起点。
-   B 内容起点对应 Separator 终点。
-   两者 DocPos 相差 1。
-   `Affinity::Upstream` 在歧义边界优先解析到前一叶子末尾。
-   `Affinity::Downstream` 在歧义边界优先解析到后一叶子开头。

对空叶子块同样适用：其内容长度为 0，起点与末尾相同；块间 Separator
仍提供相邻结构之间的线性间隔。

软换行、视觉换行可能让同一 DocPos 对应多个视觉 caret 位置，Affinity
用于区分视觉上游 / 下游位置。

### 4.4 Position 与结构位置

``` rust
pub struct Position {
    pub block_id: BlockId,
    pub offset: usize,
    pub affinity: Affinity,
}

pub struct BlockGapPosition {
    pub parent: BlockId,
    pub child_index: usize,
    pub affinity: Affinity,
}
```

-   `Position` 只定位叶子块内部或叶子块边界。
-   `BlockGapPosition` 表达容器子节点之间的结构插入点。
-   不把块级间隙伪装成普通文本位置。
-   DocPos 是全文线性位置；BlockGapPosition 是结构操作上下文，二者通过
    DocumentIndex / Editor policy
    关联，但不强行要求每个结构插入点都有唯一 DocPos。

转换 API：

``` rust
impl DocumentIndex {
    pub fn resolve_position(
        &self,
        document: &Document,
        pos: DocPos,
        affinity: Affinity,
    ) -> Result<Position, PositionError>;

    pub fn to_doc_pos(
        &self,
        document: &Document,
        position: Position,
    ) -> Result<DocPos, PositionError>;
}
```

转换必须验证：

-   BlockId 有效且指向叶子块。
-   offset 不超过叶子内容长度。
-   Text offset 是 UTF-8 字符边界。
-   Atom offset 只能落在 Atom 前后。
-   Separator 区域按 Affinity 解析到相邻叶子边界。
-   对无法无损表示的结构位置返回明确错误，不得静默猜测。

### 4.5 Grapheme、UTF-16 与平台坐标

-   DocPos 的内部文本单位为 UTF-8 byte。
-   用户级左右移动、Backspace、DeleteForward 按 Grapheme Cluster 计算。
-   `next_grapheme_boundary` / `prev_grapheme_boundary` 是 md-core
    的核心文本 API。
-   禁止把 Rust `char` 当作用户感知字符。
-   UTF-16 offset 转换集中在 md-core，Render 的平台输入适配调用它。
-   IME 的 marked range、selected range 在 Editor 内统一转换成
    DocPos；平台回调的 UTF-16 范围不得直接流入 Document。

------------------------------------------------------------------------

## 5. DocumentIndex 与不变式

### 5.1 DocumentIndex

``` rust
pub struct DocumentIndex {
    block_locations: HashMap<BlockId, BlockLocation>,
    leaf_order: Vec<BlockId>,
    length_index: FenwickTree,
    parent_index: HashMap<BlockId, BlockId>,
}
```

职责：

-   BlockId 到节点 / 父容器 / 子节点位置的定位。
-   维护完整前序叶子序列。
-   根据 DocPos 查询叶子块及其局部 offset。
-   根据 Position 查询 DocPos。
-   为 Layout、Selection、Search、Clipboard 提供结构化查询。
-   提供结构变更后的索引一致性校验。

策略：

-   初期允许在事务提交后重建索引，以降低正确性风险。
-   性能测量证明必要后，再对叶子内容长度和局部结构更新增量维护。
-   Fenwick Tree 维护叶子内容长度与叶子之间 Separator 的前缀和。
-   结构变更可以先重建 `leaf_order`，再重建 Fenwick
    Tree；不要为了过早优化引入复杂的多级增量状态。
-   索引是 Document 的派生缓存，不是文档内容的第二份真相。

### 5.2 不变式

每次成功 Transaction 提交后必须验证：

1.  容器只持 BlockId，叶子只持内容。
2.  容器子节点类型正确。
3.  Quote / ListItem 至少有一个子块；空时保留空 Paragraph。
4.  同一 Table 各行列数等于 aligns 长度。
5.  非 root 节点被父节点恰好引用一次，parent 一致，无环、无孤儿。
6.  InlineContent.len 等于所有 Run 长度之和。
7.  Text Run 非空；相邻相同 Marks 的 Text Run 已合并。
8.  Atom 的局部位置不可拆分。
9.  leaf_order 是完整前序叶子序列，Fenwick 前缀和正确。
10. 根级至少有一个块；完全空文档保留一个空 Paragraph。
11. 所有 Selection Anchor 落在合法边界。
12. BlockId generation 失效时返回 None，不得 panic。
13. SourceHint range 不得越界；pristine 状态必须与其源文本映射一致。

------------------------------------------------------------------------

## 6. Markdown 解析、源码保真与序列化

### 6.1 解析策略

使用 `pulldown-cmark` 事件流与 `into_offset_iter()`，Builder 构建
Arena，同时为节点写入 SourceHint：

``` rust
pub struct SourceHint {
    pristine: bool,
    original: Option<Arc<str>>,
    range: Option<Range<usize>>,
    style: StyleHints,
}
```

`StyleHints` 记录：

-   emphasis / strong 使用的 marker 字符。
-   fence 字符、长度、缩进和 info string。
-   列表 marker、ordered start、缩进、tight/loose 风格。
-   ATX / Setext 标题风格。
-   HardBreak 的源码表示。
-   其他会影响局部序列化的原始写法。

Parser 负责把 Markdown 语法转换为结构化模型；不得让 Render 或 Layout
解析屏幕文本来恢复文档结构。

### 6.2 SourceHint 的脏状态策略

SourceHint 是源码保真元数据，不是 Document 结构的替代品。

建议的初始失效规则：

-   修改叶子文本或 Marks：该叶子进入 dirty；其原始 range
    不再可直接作为整块输出。
-   仅修改容器子节点顺序 / 数量：容器进入
    dirty；未修改的子节点可在上下文允许时保留 pristine。
-   跨容器移动节点：旧父容器和新父容器 dirty；移动节点自身是否仍可
    pristine，取决于缩进、语法上下文和原始片段能否直接复用。
-   拆分节点：两个结果节点分别继承可证明有效的源码子范围；不能证明时标记
    dirty。
-   合并节点：合并结果通常 dirty；保留可用 StyleHints，但不伪造单一原始
    range。
-   列表结构变化：整个列表的 marker /
    序号可能变化，列表容器必须重新序列化；其他未受影响子项仍可尽可能复用内部源码片段。
-   父容器重缩进：子节点内容可以
    pristine，但输出时必须经过逐行重缩进处理。

实现时应避免仅靠单个 bool 表达所有来源状态；必要时引入明确的 SourceState
/ SourceSpan 映射。SourceHint 的精细数据结构在 md-markdown
详细设计阶段确定。

### 6.3 序列化原则

-   pristine 节点直接输出原文切片。
-   dirty 节点使用规范化 writer，但应沿用 StyleHints。
-   容器缩进变化时，对 pristine 子块逐行重缩进，行内内容保持原样。
-   未编辑文档必须满足 `serialize(parse(s)) == s`（逐字节）。
-   root 层必须满足 `parse(serialize(doc)) ≅ doc`（结构同构）。
-   列表结构发生变化时，整个列表的有序序号必须重算，即使其他项
    pristine。
-   表格单元格 HardBreak 序列化为 `<br>`；其他位置依 StyleHints
    使用两个空格或反斜杠。
-   下划线默认 `<u>…</u>`；高亮默认 `==…==`；严格 CommonMark
    模式下降级并提示。
-   连续 k 个块间空行解析为 k-1 个空 Paragraph；文档首尾 k 个空行解析为
    k 个空 Paragraph。空 Paragraph 序列化为一个额外空行。
-   容器内空段落暂不保证保真，属于明确限制，不得伪称已支持。
-   内部 Marks 的组合能力高于单一 Markdown 方言；Serializer
    必须明确处理不可表示组合，不得静默丢格式。

### 6.4 Markdown 方言矩阵

  能力                               首个正式交付版本行为
  ---------------------------------- --------------------
  CommonMark                         支持并保真
  GFM 表格、任务、删除线、自动链接   支持
  YAML frontmatter                   Raw 原样
  `$…$` / `$$…$$` 数学               解析，等宽源码展示
  脚注                               `[^label]`
  下划线                             `<u>…</u>`
  高亮                               `==…==`
  Wiki 链接                          可选开关
  HTML 块 / 内联                     原样保留，折叠展示

------------------------------------------------------------------------

## 7. Editor、Intent、Rule Table、Transaction 与 History

### 7.1 Editor 状态

``` rust
pub struct Editor {
    doc: Document,
    selection: SelectionSet,
    history: History,
    composition: Option<Composition>,
    pending_marks: Option<Marks>,
    config: EditorConfig,
    document_revision: u64,
    state_revision: u64,
}
```

-   `document_revision` 仅在成功的文档变更事务后递增。
-   `state_revision` 在 Selection、composition 或其他 Editor
    状态改变时递增。
-   Selection-only 更新不是文档事务，但应关闭当前 History 合并组。
-   Document 的唯一写入口是 Editor dispatch / 受控 Transaction。

### 7.2 Intent

Editor 只接收语义 Intent，不接收 keycode。至少包括：

`InsertText / InsertNewline / DeleteBackward / DeleteForward / Move / ToggleMark / SetBlockKind / ToggleList / ChangeIndent / Paste / Undo / Redo / ImeSetMarked / ImeCommit / ImeCancel`。

Render 负责平台 keymap 到 Intent 的映射。

### 7.3 Intent 与 Rule、Operation 的关系

采用分层处理链：

``` text
GPUI Input
  → Platform Input Adapter
  → Intent
  → Rule Dispatcher
  → Matched Rule
  → Command / Operation Builder
  → Transaction
      ├─ Preflight
      ├─ Apply Operations
      ├─ Build Inverses
      ├─ Build PositionMapping
      ├─ Update SourceHint
      ├─ Normalize
      ├─ Validate
      └─ Commit
  → ChangeSet
      ├─ DocumentIndex
      ├─ Layout Invalidation
      └─ History
```

职责：

-   Intent：用户想做什么。
-   Rule：在当前上下文中应该采取什么行为。
-   Command / Operation Builder：把规则结果转换为具体文档原语操作。
-   Operation：描述可执行、可逆的最小文档修改。
-   Transaction：保证原子性、Mapping、校验、提交与回滚。
-   ChangeSet：向 Index、Layout、History 等消费者发布确定的变化信息。

Rule 不得直接绕过 Transaction 任意修改 Arena。

### 7.4 Rule Table

行为规则必须以有序规则表实现，顺序即优先级；禁止用深层嵌套 match
代替规则表。

``` rust
pub struct Rule {
    id: RuleId,
    guard: fn(&Ctx) -> bool,
    apply: fn(&mut Transaction, &Ctx),
}
```

`Ctx`
至少包含当前叶子、块类别、父容器类别、位置类别、兄弟关系、列表深度、前后叶子、选区折叠态、Node
选中态、IME composing 状态。

全局分派顺序：

1.  composing 时不生成普通 Intent，按键交给 IME。
2.  Node 选中走 Node 专用规则。
3.  非折叠选区先
    DeleteRange，再以折叠态继续分派；删除和后续动作属于同一条
    History。Tab 跨多列表项选区除外；Backspace / DeleteForward
    删除后结束。
4.  多选区按 DocPos 倒序应用；含结构操作时禁用 History 合并。
5.  进入对应 Intent 的有序规则表。

### 7.5 Operation

Operation 必须是确定、可逆的文档原语描述。第一版至少覆盖：

``` rust
pub enum Operation {
    ReplaceText {
        block_id: BlockId,
        from: usize,
        to: usize,
        insert: String,
    },
    SplitTextRun { /* block / run position / offset / identity data */ },
    MergeTextRuns { /* block / adjacent run range */ },
    SetTextMarks { /* block / inline range / marks */ },
    InsertInline { /* block / index / run */ },
    RemoveInline { /* block / index / removed run */ },

    InsertBlock { /* parent / index / node snapshot */ },
    RemoveBlock { /* node / parent / index / subtree snapshot */ },
    MoveBlock { /* node / old parent/index / new parent/index */ },
    SplitBlock { /* source / split position / new node identity */ },
    JoinBlocks { /* left / right / resulting type */ },
    SetBlockKind { /* node / old kind / new kind */ },
    SetNodeAttribute { /* node / attribute / old and new value */ },
}
```

以上是操作类别示意，不是最终 API 签名。具体操作需覆盖
Table、List、Quote、Image 等结构。

约束：

-   每个 Operation 执行前必须 preflight，验证节点、父子类型、索引和
    offset。
-   apply 必须原子；失败不得留下半修改状态。
-   apply 返回 inverse、PositionMapping 原语和 ChangeSet。
-   Insert / Remove inverse 必须保存恢复原结构所需的节点快照与 ID。
-   Split 保留原节点一侧的 ID，另一侧分配新 ID；Join 的 ID
    保留策略必须固定并测试。
-   Move 保留节点 ID；Mapping 不得把移动节点内部位置标记为 deleted。
-   Text 替换按 UTF-8 边界校验。
-   空 Text Run 由 Normalizer 删除；Raw CodeBlock
    是否允许空内容由节点类型定义。

### 7.6 PositionMapping

PositionMapping
由按执行顺序组合的映射原语构成，不能只通过最终文档长度差推断。

至少覆盖：

-   TextInsert / TextDelete。
-   TextRunSplit / TextRunMerge。
-   InlineInsert / InlineRemove。
-   BlockInsert / BlockRemove。
-   RemoveSubtree。
-   BlockSplit / BlockJoin。
-   InlineContent 结构变更。
-   Normalizer 导致的相邻 Run 合并与空 Run 删除。
-   Block Move（保持节点内部位置语义）。

返回：

``` rust
pub struct MappedPosition {
    pub pos: DocPos,
    pub affinity: Affinity,
    pub deleted: bool,
}
```

对被删除范围内的位置，Mapping 必须定义确定的 fallback 与 deleted
标记。对节点移动，优先按稳定 BlockId 保留结构身份，再重新解析 DocPos。

NormalizationChange 必须能转换成对应 Mapping；Normalizer
不能悄悄改变位置而不通知 Editor。

### 7.7 Transaction 原子性

Transaction 捕获：

-   base document / state revision。
-   before_selection。
-   operations。
-   可选 after_selection。
-   History metadata。

执行顺序：

1.  检查 base revision。
2.  preflight 全部操作或按操作依赖执行可验证的 preflight。
3.  执行 Operation，收集 inverse 和 mapping。
4.  执行 Normalizer，收集 normalization mapping。
5.  更新 SourceHint。
6.  更新或重建 DocumentIndex。
7.  Validate Document 和 Selection。
8.  生成 ChangeSet。
9.  commit Document、Selection、revision 与相关状态。
10. 将 AppliedTransaction 提交给 History。

失败必须回滚整个事务，保持 Document、Selection、revision、Index、History
不变。

Undo / Redo 使用内部 TransactionExecutor 执行 inverse /
forward，不走普通 dispatch，避免递归记录 History。只有执行成功后才移动
History 栈。

### 7.8 Normalizer 与 Validator

Normalizer 负责规范化，不负责猜测用户意图：

-   删除空 Text Run。
-   合并相邻 Marks 相同的 Text Run。
-   修复由合法 Operation 产生的局部可规范化状态。
-   不删除显式空 Paragraph。
-   不隐式跨容器 Join。
-   不静默丢弃无法表示的 Marks。

Validator 只读检查：

-   Arena 与父子关系一致。
-   Block 类型与容器规则一致。
-   Table 行列数一致。
-   InlineContent 长度正确。
-   Run 规范化规则满足。
-   SourceHint range 有效。
-   DocumentIndex 与 Document 结构一致。
-   Selection / DocPos 位于合法边界。

### 7.9 Selection

``` rust
pub struct Anchor {
    pub pos: DocPos,
    pub affinity: Affinity,
}

pub struct Selection {
    pub anchor: Anchor,
    pub head: Anchor,
    pub kind: SelectionKind,
    pub goal_x: Option<f32>,
}

pub enum SelectionKind {
    Range,
    Node(BlockId),
    TableCells {
        start: CellPos,
        end: CellPos,
    },
}

pub struct SelectionSet {
    ranges: Vec<Selection>,
    primary: usize,
}
```

-   Text Selection 保留 anchor/head 方向。
-   `goal_x` 用于垂直移动时保留目标水平坐标；由 Layout 提供，Editor
    只透传。
-   水平移动或内容变更后清空 goal_x。
-   多选区按 DocPos 倒序应用。
-   Node Selection 用于块级原子对象和结构节点。
-   TableCells 是表格专用选择，不强行映射成普通线性范围。
-   Selection policy 属于 Editor，不把 selectability 写死在 Arena
    节点类型。

### 7.10 History

采用 Operation inverse 为核心的 Undo / Redo。

建议规则：

-   一个语义 Intent 对应一条 History 记录。
-   连续文本输入允许 coalesce。
-   仅同一叶子、位置连续、间隔 \<500ms、期间无结构操作且非 IME
    中间态的文本输入允许合并。
-   结构操作不合并。
-   一次 IME composition 作为一条 History 记录。
-   BackwardDelete / ForwardDelete
    可按相邻连续规则配置分组，但不得跨结构边界。
-   HistoryEntry 保存 document_id、forward、inverse（Undo
    执行顺序）、before/after selection、group、时间、revision
    和估算字节数。
-   新文档编辑清空 redo。
-   Selection-only 更新关闭当前 History 合并组。
-   未记录的外部文档修改使旧 inverse 失效，应清空或明确重置 History。
-   维护 save_point。
-   默认上限 1000 条并设字节预算；具体字节上限配置化。
-   超预算清理优先保留最近可撤销操作；内存估算必须包含节点快照、文本和容器向量等堆分配。

------------------------------------------------------------------------

## 8. 编辑行为规则基线

### 8.1 Enter

  ID    守卫                       动作
  ----- -------------------------- ------------------------------------
  E01   Raw                        在光标处插入字面换行
  E02   TableCell                  插入 HardBreak Atom
  E03   ListItem 唯一空子块        OutdentItem
  E04   Quote 内空段且为最后子块   ExitQuote
  E05   空 Heading                 转 Paragraph
  E06   容器内空块且不是唯一子块   删除空块，落到相邻块
  E07   Inline 非空且位于 P0       前插空 Paragraph，光标留原块 P0
  E08   Heading 位于 Pn            同层后插 Paragraph
  E09   Heading 位于 Pm            分裂，尾块为 Paragraph
  E10   ListItem 首子段位于 Pn     插入同级新 ListItem
  E11   ListItem 首子段位于 Pm     分裂尾段并移入新 ListItem
  E12   ListItem 非首子段          按项内分裂规则处理
  E13   Quote 内段落位于 Pn        在同一 Quote 内插 Paragraph
  E14   Root 空 Paragraph          前插空 Paragraph，光标留原块
  E15   Paragraph 位于 Pn          同层后插 Paragraph
  E16   Paragraph 位于 Pm          分裂 Paragraph
  E17   Atomic Node 被选中         在节点之后插 Paragraph，保留原节点

修饰键：

-   Shift+Enter：Inline / Cell 插入 SoftBreak；Raw 插入字面换行。
-   Mod+Enter：Cell 在下方插入新行并定位首格；其他情况在当前顶层块之后插
    Paragraph，退出全部容器。

### 8.2 Backspace

  ID    守卫                           行为
  ----- ------------------------------ --------------------------------------
  B01   非折叠选区                     DeleteRange 后结束
  B02   Node 选中                      删除节点，定位前叶末尾或后叶开头
  B03   Pm/Pn 且前一单位为 Atom        删除整个 Atom
  B04   Pm/Pn 其他情况                 删除前一个 grapheme cluster
  B05   P0 且当前为 Raw                移到前叶末尾；禁止降级/合并
  B06   P0 且当前为 Cell               移到前一格末尾；首格 no-op
  B07   P0 且 Inline 非 Paragraph      转为 Paragraph，内容不变
  B08   P0 且位于 ListItem             OutdentItem
  B09   P0 且 Quote 首子块             将当前块移出 Quote 并置于 Quote 之前
  B10   P0 且前后 Inline 且同 parent   Join，保留前块类型
  B11   P0 且前块 Raw                  仅移动光标
  B12   P0 且前块 Atomic               选中前一 Node，不删除
  B13   P0 且前块存在但 parent 不同    仅移动光标到前叶末尾
  B14   P0 且无前块                    no-op

语义注记：

-   标题在列表项内连续退格的顺序为降级、减少列表缩进、同容器合并。
-   原子块两阶段删除：先选中，再删除。

### 8.3 DeleteForward

  ID    守卫                                     行为
  ----- ---------------------------------------- -----------------------------
  F01   非折叠选区                               DeleteRange 后结束
  F02   Node 选中                                同 B02
  F03   P0/Pm 且后一单位为 Atom                  删除整个 Atom
  F04   P0/Pm                                    删除后一个 grapheme cluster
  F05   Pn 且 next/cur 同 parent 且双方 Inline   Join，保留当前块类型
  F06   Pn 其他情况                              no-op；禁止降级与跨容器合并

### 8.4 Tab / Shift+Tab

  -----------------------------------------------------------------------------------------------
  ID                场景               Tab                              Shift+Tab
  ----------------- ------------------ -------------------------------- -------------------------
  T01               选区跨两个或以上   所有顶层选中项缩进               所有顶层选中项反缩进
                    ListItem                                            

  T02               ListItem 内        缩进；上限为前一 sibling 深度 +  反缩进；深度 0 则退出列表
                                       1；无前 sibling no-op            

  T03               TableCell          下一格并全选；末格追加新行首格   上一格并全选；首格 no-op

  T04               CodeBlock          插入 tab 或 tab_size             移除当前/覆盖行行首最多
                                       空格；多行选区按行缩进           tab_size 空格

  T05               Quote 首子段       嵌套一层 Quote                   解开一层 Quote

  T06               普通段落/标题      no-op                            no-op
  -----------------------------------------------------------------------------------------------

普通段落 Tab 必须 no-op，避免行首缩进被 Markdown 重新解析成代码块。

### 8.5 DeleteRange

1.  将 start..end 映射到叶子序列。
2.  表格特判：范围完全落在若干单元格时只清空内容；完整覆盖行则删行；整表覆盖则删表并以空
    Paragraph 替换。
3.  首尾叶子部分覆盖，保留范围外内容。
4.  中间完整覆盖叶子，删除节点。
5.  首尾同 parent 且均 Inline 时 Join；Raw
    或跨容器不兼容时尾叶保留为空块。
6.  Cleanup，光标定位到映射后的 range.start，重算 pending_marks。
7.  DeleteRange 必须是一条 History 记录，并保证 root 至少一个块。

### 8.6 Cleanup 与 Pending Marks

每次 Intent 后执行自底向上 Cleanup：

-   空 ListItem 删除；空 List 删除；空 Quote 删除。
-   无行 Table 替换为空 Paragraph。
-   Root 全空时保留一个空 Paragraph，光标 P0。

结构操作必须显式重定位选区，禁止只依赖偏移映射。

`pending_marks`：

-   折叠选区 ToggleMark 不修改文档，只设置 pending_marks。
-   默认继承左邻 Run；块首取块默认；紧随 Atom 取空。
-   光标移动、选区变化、结构操作或块切换清除。
-   下一次 InsertText 使用后清空。

------------------------------------------------------------------------

## 9. IME、剪贴板与 InputRules

### 9.1 IME

-   composition 状态下不分派普通键盘 Intent，按键交给系统 IME。
-   预编辑文本进入临时文档状态，正常参与测量、布局和光标跟随，但不进入普通
    History。
-   `ImeCancel` 删除整段 marked range。
-   `ImeCommit` 将 composition 合并为一条 History。
-   Editor 暴露 `marked_range / text_for_range / selected_range`。
-   Render 通过 Layout 获取 `bounds_for_range`。
-   IME 相关 UTF-16 与 UTF-8 坐标转换集中在 md-core。
-   验证拼音、五笔、手写、候选翻页、Esc 取消、跨块预编辑。

### 9.2 剪贴板

Copy 输出三种 flavor：

-   `text/plain`：去除 Markdown 标记的纯文本。
-   `text/markdown`：源码。
-   `text/html`：富文本。

Paste 优先级：Markdown → Plain（按空行拆段）→ HTML（后续扩展）。不得将 HTML
粘贴支持误列为 v1 已完成能力。

### 9.3 InputRules

至少支持 `#`、`-`、`、`\> \`、围栏代码块等快捷输入。InputRules
必须转换为 Intent / Transaction，不得另开修改文档的旁路。

------------------------------------------------------------------------

## 10. Layout 引擎

### 10.1 纯几何与测量端口

``` rust
pub struct LayoutEnv {
    viewport_width: f32,
    insets: EdgeInsets,
    scale_factor: f32,
    tab_size: u8,
    wrap: bool,
    layout_epoch: u64,
    font_epoch: u64,
}

pub trait TextMeasurer {
    fn line_metrics(&mut self, style: &ResolvedStyle) -> LineMetrics;
    fn cluster_advances(&mut self, text: &str, style: &ResolvedStyle) -> Vec<f32>;
    fn measure(&mut self, text: &str, style: &ResolvedStyle) -> f32;
}
```

`FakeMeasurer` 必须确定性：ASCII 8px、CJK 16px 定宽。生产环境由
md-render 提供真实测量实现。

### 10.2 缓存与失效

-   cluster advances 缓存依赖 `(text, measure_style)`，不得包含 viewport
    width。
-   尺寸缓存依赖
    `(content_version, measure_style_id, font_epoch, layout_epoch)`。
-   纯颜色、透明度、阴影等绘制字段变化只增加 `paint_epoch`，不得触发
    layout。
-   字体、字号、字重、字距、行高、文本 padding 等测量字段变化增加
    `font_epoch`，触发重测 / 重排。
-   版心宽度、gutter、块间距等几何环境变化增加 `layout_epoch`。
-   内容 /
    属性变化：重测受影响块；结构变化：重建叶子顺序并自底向上重测受影响容器。
-   viewport resize：复用 advances，只重新换行与计算尺寸。
-   滚动：只改变可见集，不使布局缓存失效。

### 10.3 换行规则

-   最小单位为 grapheme cluster。
-   实现 UAX#14 子集及 CJK 禁则。
-   行尾空白悬挂，不计入换行宽度。
-   HardBreak 强制断行。
-   SoftBreak 两侧均为 CJK 时禁止额外插空格。
-   代码块禁止软换行，支持横向滚动与 tab stop。
-   `position_at_point(caret_rect(p, aff)) == (p, aff)`
    对所有合法边界成立。

### 10.4 输出与 API

``` rust
pub struct LayoutTree {
    order: Vec<EnterOrLeafOrExit>,
    blocks: Slab<BlockLayout>,
    total_height: f32,
}

pub struct BlockLayout {
    outer: Rect,
    content: Rect,
    decor: Rect,
    body: BodyLayout,
    estimated: bool,
    version: u64,
}
```

BodyLayout：Lines / Container / Raw / Table / Atomic。

必须提供：

`sync / position_at_point / caret_rect / rects_for_range / selection_rects / block_outer / closest_x / visible / total_height / layout_all`。

Render 禁止自行推算 caret、选区和 hit-test 几何。

### 10.5 增量、虚拟化与特殊块

-   增量 Layout 任意编辑序列后必须与全量 Layout 逐字段相等。
-   视口 ±1 屏实测，其余块使用估算高度并标记 estimated。
-   估算高度修正时保持视口锚定块屏幕位置不变。
-   `layout_all()` 供搜索、打印、全选等显式全量需求。
-   代码块：不换行、横向滚动、行号 gutter、语言标签、复制按钮 hover
    显示。
-   表格：列宽 `max(min_content, 权重分配)`；超宽整表横向滚动。
-   图片：尺寸未知时先占位（默认 180px 高或骨架），加载后
    invalidate；失败显示占位和 alt。
-   数学、HTML、frontmatter：以等宽 Raw 方式展示，可折叠。
-   标题折叠其后直到同级标题前的内容；引用 / 列表折叠自身子树。

------------------------------------------------------------------------

## 11. Render、GPUI 与资源

### 11.1 单一自定义 Element

整篇文档由一个自定义 Element 承载，GPUI flex 仅负责应用外壳。

  -----------------------------------------------------------------------
  阶段                                职责
  ----------------------------------- -----------------------------------
  request_layout                      读取可用宽度，更新 LayoutEnv，调用
                                      layout.sync，返回总尺寸

  prepaint                            计算可见集、caret / selection
                                      rect、hitbox、滚动边界

  paint                               视口裁剪与分层绘制
  -----------------------------------------------------------------------

`paint` 必须是 `(LayoutTree, ViewState, SelectionSnapshot)` 的纯函数。

### 11.2 绘制层序

块背景 / 边框 → 装饰底（引用竖线、marker、行号、表格网格）→ 选区 → glyph
→ 装饰顶（下划线、删除线、链接、IME、搜索）→ 图片与原子 → 光标 → 浮层。

块级标记绘制在 gutter；短行内标记使用不参与测量的
overlay；长结构通过浮条呈现。

### 11.3 Shaping 与滚动

-   shaping
    缓存键：`(BlockId, line_idx, content_version, measure_style_id)`。
-   滚动自实现；块高不等，不适用 uniform_list。
-   支持平滑滚动、scroll-into-view、重排锚定。
-   纯滚动帧禁止触发 request_layout。
-   一帧多个 Intent 合并为一次 layout.sync。
-   光标闪烁只重绘光标区域。

### 11.4 图片与资源

ResourceProvider
异步加载与解码图片；尺寸未知时先占位。加载完成回写资源尺寸并 invalidate
对应布局。失败显示占位和 alt。拖放图片 / 文件时统一转成受控 Intent /
资源导入流程。

------------------------------------------------------------------------

## 12. 界面布局与交互

### 12.1 设计前提

1.  纸面优先：默认屏幕只让正文与一条脊线成为视觉中心。
2.  彩色限量：饱和色仅用于光标、选区、链接、结构提示和状态反馈；正文保持中性色。
3.  几何稳定：主题切换、标记显露和交互状态不得引起非预期 reflow。

### 12.2 窗口结构与默认态

``` text
┌────────────────────────────────────────────────────────────┐
│ TitleBar 44px：文档名 · 保存态 · 撤销/重做 · 命令面板 · 导出 │
├────────────┬──────────────────────────────┬────────────────┤
│ Rail       │ Stage                        │ Inspector      │
│ 260px      │ 版心居中，默认 680px          │ 320px          │
│ 文档/大纲  │ 纸面                         │ 块属性/主题     │
├────────────┴──────────────────────────────┴────────────────┤
│ StatusBar 26px：字数 · DocPos · 块类型 · 保存态             │
└────────────────────────────────────────────────────────────┘
```

默认 Rail、Inspector 关闭，只显示 TitleBar + Stage + StatusBar。

  区域            尺寸 默认状态   快捷键
  ----------- -------- ---------- ------------------
  TitleBar        44px 常显       ---
  Rail           260px 关闭       `⌘\\`
  Stage         自适应 常显       ---
  Inspector      320px 关闭       `⌘⌥I`
  StatusBar       26px 常显       `⌘⌥D` 展开调试段

Rail 含"文档"和"大纲"两个 Tab。Inspector 上半为块属性，下半为主题。

### 12.3 版心与中文排版

-   正文建议 16--17px；中文行高 1.75，纯西文约 1.6。
-   默认版心 680px，对应 16--17px 下约 40 个中文字符 / 行。
-   提供"每行字数"控制，宽度由当前字体、字号和目标字数推导；版心仍可提供高级手动覆盖。
-   中文行宽建议 32--40 字；低于 28 字断句频繁，高于 42 字回扫困难。
-   混排西文字号可取中文字号的 0.96--0.98，或使用中文字体自带西文字形。
-   不以 `ch` 定义版心，避免字体切换导致宽度漂移。

### 12.4 垂直节奏

正文行高基线默认 28px（16px × 1.75）。块间距是主题常量；空 Paragraph
是文档节点，二者不可混为一谈。

  块                                              前间距             后间距
  ----------------------------- ------------------------ ------------------
  Paragraph                                            0   0.5 单位（14px）
  H1                                            1.5 单位           0.5 单位
  H2                                           1.25 单位           0.5 单位
  H3/H4                                           1 单位          0.25 单位
  H5/H6                                        0.75 单位          0.25 单位
  List / Quote / Code / Table                   0.5 单位           0.5 单位
  ThematicBreak                                   1 单位             1 单位
  空 Paragraph 节点               高度 1 单位，无 margin                ---

相邻块 margin 必须取 `max(before, after)`，禁止相加。

### 12.5 Gutter 与结构脊线

正文左侧预留 32px gutter：

-   折叠箭头：hover 显示。
-   块类型徽标（H1、¶、列表、引用、代码等）：hover 或 active 显示。
-   拖拽把手：hover 显示。
-   脊线节点圆点：常显但极淡。
-   批量选择框：批量模式显示。

窗口宽度 \<720px 时隐藏 gutter 和脊线。

### 12.6 零抖动标记显露

采用 C+D 组合：

1.  块级标记进 gutter：`#` 不进入正文流；gutter 用 H1
    等徽标表达标题；引用用竖线表达。正文永不显示块级标记符。
2.  短行内标记 overlay：`**`、`*`、`~~`、`==`、反引号等在 run
    边界外侧叠绘，半透明 marker 色，不参与测量。
3.  长结构浮条：光标进入链接时，在块下方浮出 URL 与编辑 /
    打开操作；图片路径、脚注等同理。
4.  边界处 overlay 可翻转到 run 内侧，不能溢出 gutter 或版心。
5.  提供"源码标记常显"逃生开关，但明确提示该模式可能引起布局变化；默认关闭。

硬约束：`layout(markers_shown) == layout(markers_hidden)`，逐字段相等。

### 12.7 响应式断点

  窗口宽度       布局
  -------------- ---------------------------
  ≥1400px        三栏可常驻
  1080--1400px   Inspector 浮层，Rail 抽屉
  720--1080px    单栏 + 双侧抽屉
  \<720px        隐藏 gutter 与脊线

建议最小窗口 720×520；若平台不允许硬限制，则以单栏降级保证可操作性。

### 12.8 TitleBar、StatusBar、命令面板

TitleBar：文档名（原地编辑）、保存态、撤销 /
重做、命令面板、导出菜单、面板开关。

StatusBar
左侧：字数、块数、阅读时长、DocPos、当前块类型。右侧常显保存态；调试展开段显示最近
RuleId、sync 耗时、脏块数和 INV 状态。

`⌘K` 命令面板分组：操作、视图、主题、保存的视图、文档、跳转到标题。

快捷键：

  操作                        快捷键
  --------------------------- -------------------------------
  命令面板                    `⌘K`
  Rail / Inspector / 调试段   `⌘\\` / `⌘⌥I` / `⌘⌥D`
  加粗 / 斜体 / 下划线        `⌘B` / `⌘I` / `⌘U`
  行内代码 / 链接             `⌘E` / `⌘⇧K`
  分块 / 软换行 / 跳出容器    `Enter` / `⇧Enter` / `⌘Enter`
  列表缩进 / 反缩进           `Tab` / `⇧Tab`
  专注模式 / 打字机滚动       `⌘⇧F` / `⌘⌥T`
  退出编辑 / 浮层             `Esc`

专注模式下非激活块降至 30%
不透明度并去饱和，不做模糊。打字机滚动将激活块保持在视口垂直中线。

------------------------------------------------------------------------

## 13. 主题与样式系统

### 13.1 三层 Token

``` text
Palette（原始色阶）
    ↓
Semantic（bg / surface / paper / text / muted / accent / marker …）
    ↓
StyleTarget（Paragraph / Heading(2) / Code / QuoteBar / Caret …）
    ↓
ResolvedStyle（测量字段 + 绘制字段）
```

Document 只保存语义 Marks；Layout 只消费 ResolvedStyle；Render 提供
StyleResolver。主题文件不得把具体字体字号直接绑定到文档节点，必须通过
StyleTarget。

StyleTarget
覆盖：Paragraph、Heading(1..6)、Strong、Emphasis、Strike、Underline、Highlight、InlineCode、Quote、QuoteBar、ListMarker、TaskCheckbox、TableHeader、TableGrid、ThematicBreak、CodeBlock、CodeGutter、Caret、Selection、ImeUnderline、SearchHit、ActiveBlock、LinkHover、Paper、Gutter、Spine、Scrollbar。

### 13.2 样式字段分层

  ------------------------------------------------------------------------------------------------------------------------------
  类型                    字段示例                                                                    Epoch / 后果
  ----------------------- --------------------------------------------------------------------------- --------------------------
  测量字段                font_family、font_size、weight、letter_spacing、line_height、文本           `font_epoch`，重测 / 重排
                          padding、tab_size                                                           

  绘制字段                color、background、border_color、underline_color、opacity、radius、shadow   `paint_epoch`，仅重绘

  布局环境字段            版心宽度、gutter 宽度、块间距、viewport insets                              `layout_epoch`，几何重算
  ------------------------------------------------------------------------------------------------------------------------------

`ResolvedStyle` 类型层面应区分 MeasureStyle 与
PaintStyle，防止误将颜色变化传入 Layout。颜色滑块不得触发
`request_layout`。

### 13.3 默认主题"墨"

  角色      色值
  --------- -----------
  bg        `#0A0E15`
  surface   `#0F141F`
  paper     `#0D121C`
  overlay   `#161C2A`
  line      `#1E2637`
  text      `#E4E9F2`
  muted     `#97A3B8`
  faint     `#5D6A80`
  accent    `#47B8E0`
  marker    `#E0A458`
  success   `#3DDC97`
  warning   `#F0B429`
  danger    `#F2555A`
  info      `#4DA3FF`

不使用纯黑底、纯白字。深色主题正文使用 Regular，标题
Semibold；强调色降低饱和度，避免刺眼。

### 13.4 次选主题"纸"

  角色      色值
  --------- -----------
  bg        `#F2EEE6`
  surface   `#F7F4EE`
  paper     `#FCFAF5`
  line      `#E2DCD0`
  text      `#1F2328`
  muted     `#5C6470`
  faint     `#8A929E`
  accent    `#1F6FEB`
  marker    `#B26A00`
  success   `#1A7F5A`
  warning   `#B26A00`
  danger    `#C0392B`
  info      `#1F6FEB`

浅色采用暖白，不使用刺眼冷白。可提供系统跟随选项。

### 13.5 辅助主题

-   高对比：正文对比度 ≥12:1，禁用半透明装饰。
-   印刷预览：纯白底、纯黑字，无交互色、无环境光效；导出使用独立印刷主题。

### 13.6 中文字体与代码字体

  -----------------------------------------------------------------------
  用途                                字体栈
  ----------------------------------- -----------------------------------
  中文正文                            PingFang SC → Source Han Sans SC →
                                      Noto Sans CJK SC → Microsoft YaHei

  中文衬线                            Source Han Serif SC → Songti SC

  西文正文                            优先中文字体自带西文；独立指定
                                      Inter / IBM Plex Sans 时字号 ×0.97

  等宽                                Sarasa Mono SC 优先
  -----------------------------------------------------------------------

Sarasa Mono SC 用于确保中西字符 2:1
宽度对齐，避免代码块中文注释逐行错位。若不随应用提供字体，需对 CJK
段做宽度补偿。字体许可、安装包体积和子集化在实施阶段评估。

### 13.7 交互色规范

  元素         规范
  ------------ -------------------------------------------------------------
  光标         2px，accent；闪烁周期 530ms；移动后暂停闪烁 300ms
  选区         accent 18--26% alpha；只到字符边界 +2px；行尾悬挂空白不选中
  IME 预编辑   1px accent 实线下划线；当前候选段 2px
  搜索命中     warning 20% 底；当前命中 40% + 1px 边框
  激活块       左侧 2px accent 竖条，不给整块加背景
  链接         accent + 1px 下划线（45% alpha）；hover 底色 accent 12%
  任务勾选     success 填充；完成文本 faint + 删除线

### 13.8 代码高亮

token 类别固定映射：keyword 品红、string 绿、number 橙、comment
灰蓝、function 蓝、type 青、constant 琥珀、punctuation
中性。代码高亮不得复用 accent，避免光标与关键字同色。

### 13.9 对比度与校正

  配对                      门槛
  --------------------- --------
  正文 / 纸面               ≥7:1
  次级文字 / 面板         ≥4.5:1
  弱提示 / 面板             ≥3:1
  光标 / 纸面               ≥3:1
  选区文字 / 选区底色     ≥4.5:1

主题加载时自动校验；不达标沿亮度轴自动微调并标注"已校正"，不得改变色相。校正后再次运行对比度测试。

### 13.10 主题文件与热重载

主题使用 TOML，建议结构：

``` toml
[meta]
name = "墨"
variant = "dark"

[semantic]
bg = "#0A0E15"
paper = "#0D121C"
text = "#E4E9F2"
accent = "#47B8E0"

[target.Paragraph]
font = "body"
size = 16
line_height = 1.75
margin_after = 14

[target.Caret]
color = "accent"
width = 2
```

-   semantic 或 target 绘制字段变化：`paint_epoch`。
-   target 测量字段变化：`font_epoch`。
-   版心、gutter、块间距变化：`layout_epoch`。
-   热重载失败保留旧主题并提示；禁止半加载状态。
-   主题解析、校验、自动校正完成后再原子替换。

------------------------------------------------------------------------

## 14. 可观测性、测试与验收

### 14.1 可观测性

-   每个 Intent 记录 RuleId、Ctx 摘要和耗时（trace）。
-   每帧记录 layout.sync 耗时、重测块数、绘制行数。
-   Debug build 每个 Intent 后执行 `check_invariants()`。
-   Debug overlay 显示最近 RuleId、sync 耗时、脏块数、INV 徽标。
-   Bug 报告附文档序列化快照、选区、最近 20 条 RuleId。

### 14.2 规则夹具

每个行为用例声明
`name / rule / before / intent / after`。简单场景用带光标
Markdown；涉及空块和结构必须用树 DSL，避免空行歧义。

记号：光标 `|`；选区 `[|…|]`。树
DSL：`P{}`、`H1{}`、`Q{}`、`UL{}`、`OL{}`、`LI{}`、`LI[x]{}`、`CB{}`、`TBL{}`、`TC{}`、`HR{}`。

每条 RuleId 至少 3 条用例。覆盖矩阵至少包括：

-   Enter：块类别 × P0/Pm/Pn/Empty × 父容器 × 折叠 / 选区。
-   Backspace：同上，并增加前一块 Inline / Raw / Atomic / None。
-   Tab：规则 × Tab / Shift+Tab × 折叠 / 单选区 / 跨项选区。
-   IME：不同输入法、候选切换、取消、跨块预编辑。
-   UI：窄窗口断点、gutter 显隐、标记 overlay 边界、主题热重载。

### 14.3 验收基线

  ----------------------------------------------------------------------------------------
  ID                      指标                    门槛
  ----------------------- ----------------------- ----------------------------------------
  AC-01                   未编辑保真              ≥200 篇真实 Markdown 逐字节相等，100%

  AC-02                   模型闭合                `parse(serialize(doc)) ≅ doc`，随机 10k
                                                  次通过

  AC-03                   撤销完备                随机 Intent 全部 Undo
                                                  后文档与选区恢复，10k 次通过

  AC-04                   不变式                  随机序列后 INV 全成立，10k 次通过

  AC-05                   增量等价                增量 Layout 与全量 Layout
                                                  逐字段相等，随机编辑 5k 次

  AC-06                   规则覆盖                每条 RuleId ≥3 条测试用例

  AC-07                   输入延迟                10 万字文档，Intent→layout.sync P95
                                                  \<1ms

  AC-08                   滚动                    10 万行虚拟化滚动 ≥60fps

  AC-09                   分层                    非 UI crate 依赖树无 GPUI

  AC-10                   IME                     中文拼音 /
                                                  五笔、候选切换、取消、跨块预编辑无异常

  AC-11                   对比度                  全部主题达到对比度门槛；校正后复检通过

  AC-12                   零抖动                  随机光标移动 5k 次，标记显露前后
                                                  LayoutTree 哈希不变

  AC-13                   热重载                  仅改绘制字段时全量重排次数为 0
  ----------------------------------------------------------------------------------------

------------------------------------------------------------------------

## 15. 已知限制与风险边界

  ----------------------------------------------------------------------------------
  限制                       影响                         处理
  -------------------------- ---------------------------- --------------------------
  容器内空段落不保证保真     引用 /                       作为后续扩展处理
                             列表内空行重解析可能丢失     

  跨容器不合并               引用后段落需多次退格         保持确定性

  TableCell 仅行内内容       单元格不能嵌套列表 / 代码块  作为后续扩展评估

  普通段落 Tab no-op         不支持段落首行缩进           由样式层解决

  多选区结构操作串行         极端场景依赖倒序顺序         固定 DocPos 倒序并覆盖测试

  列表序号重算影响 pristine  插项后 marker 变化           SER 规则明确例外

  bidi 简化                  混排精确定位不准             后续扩展

  Sarasa Mono 体积           安装包体积增加               字体子集化或 CJK 宽度补偿

  overlay 边界溢出           行首 / 行尾标记可能越界      边界翻转绘制并纳入测试

  用户不习惯正文无块级标记   与传统源码习惯不同           提供源码标记常显逃生开关

  Arena 回收与事务耦合       错误回收会破坏 Undo          先实现快照 /
                             或产生陈旧 ID                暂存正确性，再优化回收

  DocPos 混合单位            平台坐标转换与边界语义复杂   统一集中在 md-core /
                                                          DocumentIndex 并做属性测试

  SourceHint 局部失效        错误 pristine                采用保守 dirty 策略并以
                             判断会破坏源码保真           round-trip 测试验证
  ----------------------------------------------------------------------------------

------------------------------------------------------------------------

## 16. 设计治理与模块实现顺序

### 16.1 变更治理

-   Rule ID、验收 ID 一经发布不得复用或改义；废弃时标记 deprecated。
-   新编辑行为必须先更新本文规则，再实现代码与测试。
-   架构决策变更必须记录决策理由、影响 crate、缓存失效范围和迁移计划。
-   UI 数值可通过主题 / 偏好调整，但不得破坏 Layout / Render / Style
    不变量。
-   工程状态、工期、风险状态和交付物由独立 IMPLEMENTATION 文档维护。
-   本文定义最终设计规范，不作为已实现状态的证明。

### 16.2 模块实现顺序

模块按以下依赖顺序进行详细设计与实现：

1.  NodeArena：Slot、Generation、FreeList、分配、暂存、删除、恢复和回收。
2.  BlockNode / BlockKind：每种节点的字段、容器约束和访问接口。
3.  InlineContent / Run / Marks：长度缓存、Atom 规则、Marks 组合、Run
    拆分与合并。
4.  Document：root、受控读写接口、结构操作与错误模型。
5.  DocumentIndex：BlockLocation、leaf_order、parent index、Fenwick Tree
    和增量策略。
6.  DocPos / Position：Separator
    端点、Affinity、合法边界、转换算法与属性测试。
7.  Selection：Range、Node、TableCells、多选区和 goal_x。
8.  Operation / PositionMapping /
    Transaction：原子执行、inverse、rollback、normalization mapping。
9.  Markdown SourceHint：源范围、dirty 状态、split / join / move
    继承规则。
10. Editor Rule Table / History / IME。
11. Layout / Render / App。

------------------------------------------------------------------------

---

**文档结束**

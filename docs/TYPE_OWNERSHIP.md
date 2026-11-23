# 公共基础类型归属

本文件是类型归属的唯一清单，用于落实 `IMPLEMENTATION.md` 步骤 0.3。设计语义仍以 `DESIGN.md` 为准；当实施文档与设计文档冲突时，以设计文档为准，并在 `ARCHITECTURE_DECISIONS.md` 记录理由。

## 归属原则

1. 每个公共类型只能有一个定义 crate；其他 crate 通过该 crate 的公开路径引用，不得复制定义。
2. 类型放在“最底层且语义完整”的 crate：不为了少写一次 `use` 把文档概念下沉到 `md-core`。
3. `md-core` 不知道 Block、Markdown、编辑规则、布局或 UI。
4. `md-document` 不知道 Markdown 语法、像素样式、GPUI。
5. `md-layout` 不知道 GPUI、字体资源、颜色常量；视觉输入通过 `ResolvedStyle` 注入。
6. GPUI 类型只允许出现在 `md-render`，并由 `md-app` 装配。
7. 跨 crate 只通过公开 API 交互；禁止把内部 Arena 槽位、`&mut` 节点存储或 GPUI 句柄泄漏到上层。

## 类型清单

| 类型 | 归属 crate | 说明 |
|---|---|---|
| `TextOffset`、`TextRange` | `md-core` | UTF-8 字节偏移与半开区间；不包含 Block 语义 |
| `TextBuf`、`TextError` | `md-core` | 受控 UTF-8 文本缓冲与文本错误 |
| `Point`、`Size`、`Rect`、`EdgeInsets` | `md-core` | `f32` 基础几何原语 |
| `BlockId`、`NodeSlot`、`NodeArena`、`BlockNode` | `md-document` | Arena 与稳定节点身份 |
| `BlockKind`、`InlineContent`、`Run`、`Marks`、`MarkFlags`、`LinkRef`、`AtomKind` | `md-document` | 文档结构与行内内容 |
| `Document`、`DocumentIndex`、`BlockLocation` | `md-document` | 文档根、派生索引 |
| `DocPos`、`Affinity`、`Position`、`BlockGapPosition` | `md-document` | 全文坐标与结构位置（见 ADR-0002） |
| `Anchor`、`Selection`、`SelectionSet`、`SelectionKind`、`CellPos` | `md-document` | 选区模型 |
| `DocumentError`、`PositionError`、`DocumentValidationError` | `md-document` | 文档与坐标错误 |
| `SourceHint`、`StyleHints` | `md-markdown` | 源码保真元数据 |
| `Parser`、`Builder`、`Serializer`、`MarkdownError` | `md-markdown` | Markdown 转换 |
| `Editor`、`EditorConfig`、`EditorError` | `md-editor` | 编辑内核状态与错误 |
| `Intent`、`RuleId`、`Rule`、`Ctx` | `md-editor` | 意图与有序规则表 |
| `Operation`、`Transaction`、`ChangeSet` | `md-editor` | 可逆原语与原子事务 |
| `PositionMapping`、`MappedPosition` | `md-editor` | 编辑后位置映射 |
| `History`、`HistoryEntry`、`Composition` | `md-editor` | 撤销栈与 IME 状态 |
| `LayoutEnv`、`ResolvedStyle`、`MeasureStyle`、`PaintStyle` | `md-layout` | 布局环境与样式分层 |
| `TextMeasurer`、`FakeMeasurer`、`LineMetrics` | `md-layout` | 测量端口与确定性假测量器 |
| `LayoutTree`、`BlockLayout`、`BodyLayout`、`LayoutError` | `md-layout` | 几何结果与布局错误 |
| `ViewState`、`StyleResolver`、`SelectionSnapshot`、`ResourceProvider` | `md-render` | GPUI 边界状态与资源适配 |
| `App`、`AppConfig`、`FileIo`、`ThemeManager`、`Command` | `md-app` | 应用装配与本地能力 |

## 禁止事项

- 不在 `md-core` 定义 `DocPos`、`BlockId`、`Selection` 等文档语义类型。
- 不在 `md-document` 定义 Markdown `SourceHint`、像素样式或 GPUI 类型。
- 不在 `md-layout` 定义字体句柄、颜色常量或 GPUI 几何类型。
- 不在 `md-render` 重新定义 `DocPos`、`Rect`、`Selection` 等已有类型；只做必要的 GPUI 类型转换。
- 不为“方便”复制结构体；需要跨 crate 共享时，提升到正确的所有者并公开访问接口。

## 实施检查

- 阶段 1 只实现 `md-core` 的文本与几何类型。
- `DocPos`、`Affinity`、`Position` 在阶段 2～3 于 `md-document` 实现。
- 每个新类型进入代码前，先确认本表已有归属；若本表未覆盖，先补充本表并说明理由。

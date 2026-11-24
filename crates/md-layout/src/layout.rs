//! Block layout, hit-testing, selection geometry, and incremental caching.

use std::collections::HashMap;

use md_core::{Point, Rect, Size};
use md_document::{
    Affinity, BlockId, BlockKind, DocPos, Document, DocumentIndex, SelectionKind, SelectionSet,
};

use crate::inline::{InlineLayout, Line, layout_inline};
use crate::measure::{FakeMeasurer, TextMeasurer};
use crate::style::{LayoutEnv, MeasureStyle, ResolvedStyle};

/// Geometry category of a laid-out block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayoutKind {
    /// Paragraph.
    Paragraph,
    /// Heading with level 1..=6.
    Heading(u8),
    /// Fenced code block.
    CodeBlock,
    /// Raw HTML.
    Html,
    /// Math block.
    Math,
    /// YAML frontmatter.
    FrontMatter,
    /// Blockquote.
    Quote,
    /// List container.
    List {
        /// Whether the list is ordered.
        ordered: bool,
    },
    /// List item.
    ListItem,
    /// Footnote definition.
    Footnote,
    /// Table container.
    Table,
    /// Table row.
    TableRow,
    /// Table cell.
    TableCell,
    /// Horizontal rule.
    ThematicBreak,
    /// Image block.
    Image,
}

/// One laid-out block.
#[derive(Clone, Debug, PartialEq)]
pub struct BlockLayout {
    /// Source block.
    pub id: BlockId,
    /// Geometry category.
    pub kind: LayoutKind,
    /// Outer rectangle including padding.
    pub rect: Rect,
    /// Inner content rectangle.
    pub content_rect: Rect,
    /// Inline lines for leaf blocks.
    pub lines: Vec<Line>,
    /// Child layouts.
    pub children: Vec<BlockLayout>,
}

impl BlockLayout {
    /// Returns whether this layout is a leaf.
    #[must_use]
    pub fn is_leaf(&self) -> bool {
        self.children.is_empty()
    }
}

/// Complete layout result.
#[derive(Clone, Debug, PartialEq)]
pub struct LayoutTree {
    /// Root block layouts.
    pub blocks: Vec<BlockLayout>,
    /// Snapshot of the document index used for mapping.
    pub index: DocumentIndex,
    /// Layout environment.
    pub env: LayoutEnv,
    /// Total content height.
    pub height: f32,
}

impl LayoutTree {
    /// Creates an empty layout tree for the given environment.
    #[must_use]
    pub fn empty(env: LayoutEnv) -> Self {
        Self {
            blocks: Vec::new(),
            index: DocumentIndex::default(),
            env,
            height: 0.0,
        }
    }

    /// Lays out a document from scratch.
    #[must_use]
    pub fn full(document: &Document, env: LayoutEnv) -> Self {
        LayoutEngine::new().sync(document, env)
    }

    /// Returns the total content height.
    #[must_use]
    pub fn total_height(&self) -> f32 {
        self.height
    }

    /// Finds a block layout recursively.
    #[must_use]
    pub fn block_outer(&self, id: BlockId) -> Option<&BlockLayout> {
        fn find(blocks: &[BlockLayout], id: BlockId) -> Option<&BlockLayout> {
            for block in blocks {
                if block.id == id {
                    return Some(block);
                }
                if let Some(found) = find(&block.children, id) {
                    return Some(found);
                }
            }
            None
        }
        find(&self.blocks, id)
    }

    /// Hit-tests a point and returns the position plus block-boundary affinity.
    ///
    /// Clicking at the end of a block yields `Upstream`, so Backspace deletes the
    /// last character instead of joining the next block.
    #[must_use]
    pub fn position_with_affinity(&self, point: Point) -> Option<(DocPos, Affinity)> {
        let leaf = self
            .leaf_at_point(point)
            .or_else(|| self.nearest_leaf(point.y))?;
        let line = leaf
            .lines
            .iter()
            .find(|line| point.y >= line.rect.top() && point.y < line.rect.bottom())
            .or_else(|| {
                leaf.lines.iter().min_by(|left, right| {
                    let left_distance = if point.y < left.rect.top() {
                        left.rect.top() - point.y
                    } else {
                        (point.y - left.rect.bottom()).max(0.0)
                    };
                    let right_distance = if point.y < right.rect.top() {
                        right.rect.top() - point.y
                    } else {
                        (point.y - right.rect.bottom()).max(0.0)
                    };
                    left_distance.total_cmp(&right_distance)
                })
            })?;
        let local = line.offset_at_x(point.x);
        let leaf_index = self.index.leaf_index(leaf.id)?;
        let start = self.index.leaf_start(leaf_index)?;
        let length = self.index.leaf_len(leaf_index)?;
        let affinity = if point.x >= line.rect.right()
            || (local == length && leaf_index + 1 < self.index.leaf_order().len())
        {
            Affinity::Upstream
        } else {
            Affinity::Downstream
        };
        Some((DocPos::new(start + local), affinity))
    }

    /// Returns the caret rectangle for a position resolved with an affinity.
    #[must_use]
    pub fn caret_rect_with_affinity(
        &self,
        document: &Document,
        pos: DocPos,
        affinity: Affinity,
    ) -> Option<Rect> {
        let resolved = self.index.resolve_position(document, pos, affinity).ok()?;
        let layout = self.block_outer(resolved.block_id)?;
        let line = layout
            .lines
            .iter()
            .find(|line| resolved.offset >= line.start && resolved.offset <= line.end)?;
        Some(Rect::new(
            line.x_for_offset(resolved.offset),
            line.rect.top(),
            1.0,
            line.rect.height(),
        ))
    }

    /// Returns the caret rectangle for a document position.
    #[must_use]
    pub fn caret_rect(&self, pos: DocPos) -> Option<Rect> {
        let hit = self.index.leaf_at(pos.get())?;
        let layout = self.block_outer(hit.block_id)?;
        let line = layout
            .lines
            .iter()
            .find(|line| hit.offset >= line.start && hit.offset <= line.end)?;
        Some(Rect::new(
            line.x_for_offset(hit.offset),
            line.rect.top(),
            1.0,
            line.rect.height(),
        ))
    }

    /// Returns the closest caret x for a document position.
    #[must_use]
    pub fn closest_x(&self, pos: DocPos) -> Option<f32> {
        self.caret_rect(pos).map(|rect| rect.left())
    }

    /// Returns the document position nearest to a point.
    #[must_use]
    pub fn position_at_point(&self, point: Point) -> Option<DocPos> {
        let leaf = self
            .leaf_at_point(point)
            .or_else(|| self.nearest_leaf(point.y))?;
        let line = leaf
            .lines
            .iter()
            .find(|line| point.y >= line.rect.top() && point.y < line.rect.bottom())
            .or_else(|| {
                leaf.lines.iter().min_by(|left, right| {
                    let left_distance = if point.y < left.rect.top() {
                        left.rect.top() - point.y
                    } else {
                        (point.y - left.rect.bottom()).max(0.0)
                    };
                    let right_distance = if point.y < right.rect.top() {
                        right.rect.top() - point.y
                    } else {
                        (point.y - right.rect.bottom()).max(0.0)
                    };
                    left_distance.total_cmp(&right_distance)
                })
            })?;
        let local = line.offset_at_x(point.x);
        let leaf_index = self.index.leaf_index(leaf.id)?;
        let start = self.index.leaf_start(leaf_index)?;
        Some(DocPos::new(start + local))
    }

    /// Returns rectangles covering a document range.
    #[must_use]
    pub fn rects_for_range(&self, start: DocPos, end: DocPos) -> Vec<Rect> {
        let (Some(from), Some(to)) = (
            self.index.leaf_at(start.get()),
            self.index.leaf_at(end.get()),
        ) else {
            return Vec::new();
        };
        let mut rects = Vec::new();
        for leaf_index in from.leaf_index..=to.leaf_index {
            let Some(leaf_id) = self.index.leaf_order().get(leaf_index).copied() else {
                break;
            };
            let Some(layout) = self.block_outer(leaf_id) else {
                continue;
            };
            let local_start = if leaf_index == from.leaf_index {
                from.offset
            } else {
                0
            };
            let local_end = if leaf_index == to.leaf_index {
                to.offset
            } else {
                self.index.leaf_len(leaf_index).unwrap_or(0)
            };
            for line in &layout.lines {
                if let Some(rect) = line.rect_for_range(local_start, local_end) {
                    rects.push(rect);
                }
            }
        }
        rects
    }

    /// Returns rectangles for a selection set.
    #[must_use]
    pub fn selection_rects(&self, selection: &SelectionSet) -> Vec<Rect> {
        let Some(primary) = selection.primary_selection() else {
            return Vec::new();
        };
        if !matches!(primary.kind, SelectionKind::Range) || primary.is_collapsed() {
            return Vec::new();
        }
        self.rects_for_range(primary.start(), primary.end())
    }

    /// Returns leaf blocks intersecting the viewport, expanded by one screen.
    #[must_use]
    pub fn visible(&self, viewport: Rect) -> Vec<BlockId> {
        let margin = viewport.height();
        let expanded = Rect::new(
            viewport.left(),
            viewport.top() - margin,
            viewport.width(),
            viewport.height() + margin * 2.0,
        );
        let mut visible = Vec::new();
        fn visit(blocks: &[BlockLayout], viewport: Rect, visible: &mut Vec<BlockId>) {
            for block in blocks {
                if block.rect.intersect(viewport).is_none() {
                    continue;
                }
                if block.is_leaf() {
                    visible.push(block.id);
                } else {
                    visit(&block.children, viewport, visible);
                }
            }
        }
        visit(&self.blocks, expanded, &mut visible);
        visible
    }

    fn nearest_leaf(&self, y: f32) -> Option<&BlockLayout> {
        fn visit(blocks: &[BlockLayout], y: f32) -> Option<(&BlockLayout, f32)> {
            let mut best = None;
            for block in blocks {
                if block.is_leaf() {
                    let distance = if y < block.rect.top() {
                        block.rect.top() - y
                    } else if y > block.rect.bottom() {
                        y - block.rect.bottom()
                    } else {
                        0.0
                    };
                    if best.is_none_or(|(_, current): (&BlockLayout, f32)| distance < current) {
                        best = Some((block, distance));
                    }
                } else if let Some(found) = visit(&block.children, y)
                    && best.is_none_or(|(_, current)| found.1 < current)
                {
                    best = Some(found);
                }
            }
            best
        }
        visit(&self.blocks, y).map(|(block, _)| block)
    }

    fn leaf_at_point(&self, point: Point) -> Option<&BlockLayout> {
        fn visit(blocks: &[BlockLayout], point: Point) -> Option<&BlockLayout> {
            let mut best: Option<&BlockLayout> = None;
            for block in blocks {
                if block.is_leaf() {
                    if block.rect.contains(point)
                        && best.is_none_or(|current: &BlockLayout| {
                            block.rect.width() * block.rect.height()
                                < current.rect.width() * current.rect.height()
                        })
                    {
                        best = Some(block);
                    }
                } else if let Some(found) = visit(&block.children, point) {
                    best = Some(found);
                }
            }
            best
        }
        visit(&self.blocks, point)
    }
}

#[derive(Clone, Debug)]
struct CachedInline {
    kind: BlockKind,
    width: f32,
    style: MeasureStyle,
    lines: Vec<Line>,
    height: f32,
}

/// Incremental layout engine with a leaf-line cache.
pub struct LayoutEngine<'a> {
    cache: HashMap<BlockId, CachedInline>,
    measurer: Box<dyn TextMeasurer + 'a>,
}

impl Default for LayoutEngine<'static> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> LayoutEngine<'a> {
    /// Creates an engine with an empty cache.
    #[must_use]
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
            measurer: Box::new(FakeMeasurer::new()),
        }
    }

    /// Creates an engine with a custom measurer.
    #[must_use]
    pub fn with_measurer(measurer: Box<dyn TextMeasurer + 'a>) -> Self {
        Self {
            cache: HashMap::new(),
            measurer,
        }
    }

    /// Invalidates one block.
    pub fn invalidate(&mut self, id: BlockId) {
        self.cache.remove(&id);
    }

    /// Invalidates all cached blocks.
    pub fn invalidate_all(&mut self) {
        self.cache.clear();
    }

    /// Lays out a document, reusing unchanged leaf line layouts.
    #[must_use]
    pub fn sync(&mut self, document: &Document, env: LayoutEnv) -> LayoutTree {
        self.cache.retain(|id, _| document.node(*id).is_some());
        let viewport_width = env.width();
        let padding = env.style.content_padding.max(0.0);
        let width = (viewport_width - padding * 2.0)
            .max(1.0)
            .min(env.style.max_content_width.max(1.0));
        let x = ((viewport_width - width) * 0.5).max(padding.min(viewport_width * 0.5));
        let mut blocks = Vec::new();
        let mut y = 0.0;
        for id in document.root_children() {
            let block = self.layout_block(document, *id, Point::new(x, y), width, env);
            y = block.rect.bottom() + env.style.block_gap + blank_line_gap(document, *id, env);
            blocks.push(block);
        }
        let height = blocks.last().map_or(0.0, |block| block.rect.bottom());
        LayoutTree {
            blocks,
            index: document.index().clone(),
            env,
            height,
        }
    }

    fn layout_block(
        &mut self,
        document: &Document,
        id: BlockId,
        origin: Point,
        width: f32,
        env: LayoutEnv,
    ) -> BlockLayout {
        let Some(kind) = document.kind(id).cloned() else {
            return empty_layout(id, origin, width, env.style);
        };
        match &kind {
            BlockKind::Paragraph(content) => self.layout_inline_leaf(
                id,
                kind.clone(),
                content,
                LayoutKind::Paragraph,
                origin,
                width,
                MeasureStyle::new(env.style.font_size, env.style.line_height),
            ),
            BlockKind::Heading { level, content } => {
                let scale = env
                    .style
                    .heading_scale
                    .get(usize::from(level.saturating_sub(1)))
                    .copied()
                    .unwrap_or(1.1);
                let font = env.style.font_size * scale;
                let mut measure = MeasureStyle::new(font, font * 1.3);
                measure.weight = if *level <= 2 { 600.0 } else { 500.0 };
                self.layout_inline_leaf(
                    id,
                    kind.clone(),
                    content,
                    LayoutKind::Heading(*level),
                    origin,
                    width,
                    measure,
                )
            }
            BlockKind::TableCell { content } => self.layout_inline_leaf(
                id,
                kind.clone(),
                content,
                LayoutKind::TableCell,
                origin,
                width,
                MeasureStyle::new(env.style.font_size, env.style.line_height),
            ),
            BlockKind::CodeBlock { text, .. } => {
                self.layout_raw_leaf(id, LayoutKind::CodeBlock, text.as_str(), origin, width, env)
            }
            BlockKind::HtmlBlock(text) => {
                self.layout_raw_leaf(id, LayoutKind::Html, text.as_str(), origin, width, env)
            }
            BlockKind::MathBlock(text) => {
                self.layout_raw_leaf(id, LayoutKind::Math, text.as_str(), origin, width, env)
            }
            BlockKind::FrontMatter(text) => self.layout_raw_leaf(
                id,
                LayoutKind::FrontMatter,
                text.as_str(),
                origin,
                width,
                env,
            ),
            BlockKind::ThematicBreak => {
                let width = width.max(0.0);
                let height = env.style.rule_thickness.max(1.0);
                BlockLayout {
                    id,
                    kind: LayoutKind::ThematicBreak,
                    rect: Rect::new(origin.x, origin.y, width, height),
                    content_rect: Rect::new(origin.x, origin.y, width, height),
                    lines: Vec::new(),
                    children: Vec::new(),
                }
            }
            BlockKind::ImageBlock { .. } => {
                let height = env.style.image_height;
                BlockLayout {
                    id,
                    kind: LayoutKind::Image,
                    rect: Rect::new(origin.x, origin.y, width.max(0.0), height),
                    content_rect: Rect::new(origin.x, origin.y, width.max(0.0), height),
                    lines: Vec::new(),
                    children: Vec::new(),
                }
            }
            BlockKind::Quote(_) => self.layout_container(
                document,
                id,
                LayoutKind::Quote,
                origin,
                width,
                env,
                env.style.quote_indent,
                env.style.quote_padding,
            ),
            BlockKind::FootnoteDefinition { .. } => self.layout_container(
                document,
                id,
                LayoutKind::Footnote,
                origin,
                width,
                env,
                env.style.quote_indent,
                env.style.quote_padding,
            ),
            BlockKind::List { ordered, .. } => {
                self.layout_list(document, id, *ordered, origin, width, env)
            }
            BlockKind::ListItem { .. } => self.layout_container(
                document,
                id,
                LayoutKind::ListItem,
                origin,
                width,
                env,
                env.style.list_indent,
                0.0,
            ),
            BlockKind::Table { aligns, .. } => {
                self.layout_table(document, id, aligns.len(), origin, width, env)
            }
            BlockKind::TableRow { .. } => {
                self.layout_table_row(document, id, origin, width, env, None)
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn layout_inline_leaf(
        &mut self,
        id: BlockId,
        kind: BlockKind,
        content: &md_document::InlineContent,
        layout_kind: LayoutKind,
        origin: Point,
        width: f32,
        style: MeasureStyle,
    ) -> BlockLayout {
        let layout = if let Some(cached) = self.cache.get(&id)
            && cached.kind == kind
            && cached.width == width
            && cached.style == style
        {
            let mut lines = cached.lines.clone();
            for line in &mut lines {
                line.rect.origin.x += origin.x;
                line.rect.origin.y += origin.y;
                line.baseline += origin.y;
            }
            InlineLayout {
                lines,
                size: Size::new(width, cached.height),
            }
        } else {
            let layout = layout_inline(content, &style, self.measurer.as_ref(), width, Point::ZERO);
            self.cache.insert(
                id,
                CachedInline {
                    kind,
                    width,
                    style,
                    lines: layout.lines.clone(),
                    height: layout.size.height,
                },
            );
            let mut lines = layout.lines;
            for line in &mut lines {
                line.rect.origin.x += origin.x;
                line.rect.origin.y += origin.y;
                line.baseline += origin.y;
            }
            InlineLayout {
                lines,
                size: layout.size,
            }
        };
        BlockLayout {
            id,
            kind: layout_kind,
            rect: Rect::new(origin.x, origin.y, width.max(0.0), layout.size.height),
            content_rect: Rect::new(origin.x, origin.y, width.max(0.0), layout.size.height),
            lines: layout.lines,
            children: Vec::new(),
        }
    }

    fn layout_raw_leaf(
        &mut self,
        id: BlockId,
        layout_kind: LayoutKind,
        text: &str,
        origin: Point,
        width: f32,
        env: LayoutEnv,
    ) -> BlockLayout {
        let mut style = MeasureStyle::new(env.style.code_font_size, env.style.code_font_size * 1.5);
        style.mono = true;
        let padding = env.style.code_padding;
        let mut content = md_document::InlineContent::new();
        for (index, line) in text.split('\n').enumerate() {
            if index > 0 {
                let _ = content.insert_atom_at(
                    content.len(),
                    md_document::AtomKind::HardBreak,
                    md_document::Marks::plain(),
                );
            }
            content.append_content(&md_document::InlineContent::from_text(line));
        }
        let layout = layout_inline(
            &content,
            &style,
            self.measurer.as_ref(),
            (width - padding * 2.0).max(1.0),
            Point::ZERO,
        );
        let mut lines = layout.lines;
        for line in &mut lines {
            line.rect.origin.x += origin.x + padding;
            line.rect.origin.y += origin.y + padding;
            line.baseline += origin.y + padding;
        }
        let height = layout.size.height + padding * 2.0;
        BlockLayout {
            id,
            kind: layout_kind,
            rect: Rect::new(origin.x, origin.y, width.max(0.0), height),
            content_rect: Rect::new(
                origin.x + padding,
                origin.y + padding,
                (width - padding * 2.0).max(0.0),
                layout.size.height,
            ),
            lines,
            children: Vec::new(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn layout_container(
        &mut self,
        document: &Document,
        id: BlockId,
        kind: LayoutKind,
        origin: Point,
        width: f32,
        env: LayoutEnv,
        indent: f32,
        vertical_padding: f32,
    ) -> BlockLayout {
        let children = document.children(id).unwrap_or(&[]);
        let mut layouts = Vec::new();
        let mut y = origin.y + vertical_padding;
        let inner_width = (width - indent).max(1.0);
        for child in children {
            let layout = self.layout_block(
                document,
                *child,
                Point::new(origin.x + indent, y),
                inner_width,
                env,
            );
            y = layout.rect.bottom() + env.style.block_gap + blank_line_gap(document, *child, env);
            layouts.push(layout);
        }
        let height = layouts.last().map_or(0.0, |layout| {
            layout.rect.bottom() - origin.y + vertical_padding
        });
        BlockLayout {
            id,
            kind,
            rect: Rect::new(origin.x, origin.y, width.max(0.0), height),
            content_rect: Rect::new(
                origin.x + indent,
                origin.y + vertical_padding,
                inner_width,
                height,
            ),
            lines: Vec::new(),
            children: layouts,
        }
    }

    fn layout_list(
        &mut self,
        document: &Document,
        id: BlockId,
        ordered: bool,
        origin: Point,
        width: f32,
        env: LayoutEnv,
    ) -> BlockLayout {
        let items = document.children(id).unwrap_or(&[]);
        let mut children = Vec::new();
        let mut y = origin.y;
        let inner_width = (width - env.style.list_indent).max(1.0);
        for item in items {
            let layout = self.layout_block(
                document,
                *item,
                Point::new(origin.x + env.style.list_indent, y),
                inner_width,
                env,
            );
            y = layout.rect.bottom() + env.style.block_gap + blank_line_gap(document, *item, env);
            children.push(layout);
        }
        let height = children
            .last()
            .map_or(0.0, |layout| layout.rect.bottom() - origin.y);
        BlockLayout {
            id,
            kind: LayoutKind::List { ordered },
            rect: Rect::new(origin.x, origin.y, width.max(0.0), height),
            content_rect: Rect::new(
                origin.x + env.style.list_indent,
                origin.y,
                inner_width,
                height,
            ),
            lines: Vec::new(),
            children,
        }
    }

    fn layout_table(
        &mut self,
        document: &Document,
        id: BlockId,
        columns: usize,
        origin: Point,
        width: f32,
        env: LayoutEnv,
    ) -> BlockLayout {
        let rows = document.children(id).unwrap_or(&[]);
        let mut children = Vec::new();
        let mut y = origin.y;
        for row in rows {
            let layout = self.layout_table_row(
                document,
                *row,
                Point::new(origin.x, y),
                width,
                env,
                Some(columns.max(1)),
            );
            y = layout.rect.bottom();
            children.push(layout);
        }
        let height = children
            .last()
            .map_or(0.0, |layout| layout.rect.bottom() - origin.y);
        BlockLayout {
            id,
            kind: LayoutKind::Table,
            rect: Rect::new(origin.x, origin.y, width.max(0.0), height),
            content_rect: Rect::new(origin.x, origin.y, width.max(0.0), height),
            lines: Vec::new(),
            children,
        }
    }

    fn layout_table_row(
        &mut self,
        document: &Document,
        id: BlockId,
        origin: Point,
        width: f32,
        env: LayoutEnv,
        columns: Option<usize>,
    ) -> BlockLayout {
        let cells = document.children(id).unwrap_or(&[]);
        let count = columns.unwrap_or(cells.len()).max(1);
        let cell_width = width / count as f32;
        let padding = env.style.cell_padding;
        let mut children = Vec::new();
        let mut height: f32 = 0.0;
        for (index, cell) in cells.iter().enumerate() {
            let layout = self.layout_block(
                document,
                *cell,
                Point::new(
                    origin.x + index as f32 * cell_width + padding,
                    origin.y + padding,
                ),
                (cell_width - padding * 2.0).max(1.0),
                env,
            );
            height = height.max(layout.rect.height() + padding * 2.0);
            children.push(layout);
        }
        let height = height.max(env.style.line_height);
        BlockLayout {
            id,
            kind: LayoutKind::TableRow,
            rect: Rect::new(origin.x, origin.y, width.max(0.0), height),
            content_rect: Rect::new(origin.x, origin.y, width.max(0.0), height),
            lines: Vec::new(),
            children,
        }
    }
}

fn empty_layout(id: BlockId, origin: Point, width: f32, style: ResolvedStyle) -> BlockLayout {
    let height = style.line_height;
    BlockLayout {
        id,
        kind: LayoutKind::Paragraph,
        rect: Rect::new(origin.x, origin.y, width.max(0.0), height),
        content_rect: Rect::new(origin.x, origin.y, width.max(0.0), height),
        lines: Vec::new(),
        children: Vec::new(),
    }
}

fn blank_line_gap(document: &Document, id: BlockId, env: LayoutEnv) -> f32 {
    document
        .node(id)
        .map_or(0, |node| node.source().style().blank_lines_after.min(8)) as f32
        * env.style.line_height
}

/// Returns the default measurement style for body text.
#[must_use]
pub fn body_measure_style(style: &ResolvedStyle) -> MeasureStyle {
    MeasureStyle::new(style.font_size, style.line_height)
}

/// Lays out a document with a caller-supplied measurer.
#[must_use]
pub fn layout_with_measurer(
    document: &Document,
    env: LayoutEnv,
    measurer: &mut dyn TextMeasurer,
) -> LayoutTree {
    struct Borrowed<'a>(&'a mut dyn TextMeasurer);
    impl TextMeasurer for Borrowed<'_> {
        fn advance(&self, text: &str, style: &MeasureStyle) -> f32 {
            self.0.advance(text, style)
        }

        fn atom_advance(&self, kind: &md_document::AtomKind, style: &MeasureStyle) -> f32 {
            self.0.atom_advance(kind, style)
        }

        fn line_metrics(&self, style: &MeasureStyle) -> crate::measure::LineMetrics {
            self.0.line_metrics(style)
        }
    }

    let mut engine = LayoutEngine::with_measurer(Box::new(Borrowed(measurer)));
    engine.sync(document, env)
}

#[cfg(test)]
mod tests {
    use super::{LayoutEngine, LayoutTree};
    use crate::style::{LayoutEnv, ResolvedStyle};
    use md_core::{Point, Rect, Size};
    use md_document::{
        Affinity, Align, Anchor, BlockKind, DocPos, Document, InlineContent, Selection,
        SelectionSet,
    };

    fn env(width: f32) -> LayoutEnv {
        LayoutEnv::new(Size::new(width, 400.0), ResolvedStyle::default())
    }

    fn paragraph(text: &str) -> BlockKind {
        BlockKind::Paragraph(InlineContent::from_text(text))
    }

    #[test]
    fn affinity_caret_rects_differ_at_block_boundary() {
        let mut document = Document::new();
        document
            .set_inline_content(document.root_children()[0], InlineContent::from_text("abc"))
            .unwrap();
        document.append_block(None, paragraph("def")).unwrap();
        let tree = LayoutTree::full(&document, env(400.0));
        let up = tree
            .caret_rect_with_affinity(&document, DocPos::new(3), Affinity::Upstream)
            .unwrap();
        let down = tree
            .caret_rect_with_affinity(&document, DocPos::new(3), Affinity::Downstream)
            .unwrap();
        assert!(up.top() < down.top());
        assert!(up.left() > down.left());
    }

    #[test]
    fn click_beyond_line_end_still_hits_nearest_line_end() {
        let mut document = Document::new();
        document
            .set_inline_content(document.root_children()[0], InlineContent::from_text("abc"))
            .unwrap();
        document.append_block(None, paragraph("def")).unwrap();
        let tree = LayoutTree::full(&document, env(400.0));
        let line = tree.blocks[0].lines.last().unwrap();
        let point = Point::new(line.rect.right() + 20.0, line.rect.top() + 1.0);
        let (position, affinity) = tree.position_with_affinity(point).unwrap();
        assert_eq!(position, DocPos::new(3));
        assert_eq!(affinity, Affinity::Upstream);
    }

    #[test]
    fn click_at_block_end_reports_upstream_affinity() {
        let mut document = Document::new();
        document
            .set_inline_content(document.root_children()[0], InlineContent::from_text("abc"))
            .unwrap();
        document.append_block(None, paragraph("def")).unwrap();
        let tree = LayoutTree::full(&document, env(400.0));
        let line = tree.blocks[0].lines.last().unwrap();
        let point = Point::new(line.rect.right() - 1.0, line.rect.top() + 1.0);
        let (position, affinity) = tree.position_with_affinity(point).unwrap();
        assert_eq!(position, DocPos::new(3));
        assert_eq!(affinity, Affinity::Upstream);
    }

    #[test]
    fn paragraph_wraps_and_reports_height() {
        let mut document = Document::new();
        document
            .set_inline_content(
                document.root_children()[0],
                InlineContent::from_text("abcdefghijklmnopqrstuvwxyz"),
            )
            .unwrap();
        let tree = LayoutTree::full(&document, env(40.0));
        let block = &tree.blocks[0];
        assert!(block.lines.len() > 1);
        assert!(block.rect.height() > 20.0);
    }

    #[test]
    fn heading_uses_scaled_line_height() {
        let mut document = Document::new();
        let root = document.root_children()[0];
        document
            .set_block_kind(
                root,
                BlockKind::Heading {
                    level: 1,
                    content: InlineContent::from_text("Title"),
                },
            )
            .unwrap();
        let tree = LayoutTree::full(&document, env(300.0));
        let heading = &tree.blocks[0];
        let body = LayoutTree::full(
            &{
                let mut body_doc = Document::new();
                body_doc
                    .set_inline_content(
                        body_doc.root_children()[0],
                        InlineContent::from_text("Title"),
                    )
                    .unwrap();
                body_doc
            },
            env(300.0),
        );
        assert!(heading.lines[0].rect.height() > body.blocks[0].lines[0].rect.height());
    }

    #[test]
    fn caret_rect_and_hit_test_round_trip() {
        let mut document = Document::new();
        document
            .set_inline_content(document.root_children()[0], InlineContent::from_text("abc"))
            .unwrap();
        let tree = LayoutTree::full(&document, env(300.0));
        let caret = tree.caret_rect(DocPos::new(1)).unwrap();
        let point = Point::new(caret.left() + 0.1, caret.top() + 1.0);
        assert_eq!(tree.position_at_point(point), Some(DocPos::new(1)));
        assert!(tree.closest_x(DocPos::new(2)).is_some());
    }

    #[test]
    fn selection_rects_cover_a_range() {
        let mut document = Document::new();
        document
            .set_inline_content(
                document.root_children()[0],
                InlineContent::from_text("abcdef"),
            )
            .unwrap();
        let tree = LayoutTree::full(&document, env(300.0));
        let selection = SelectionSet::new(Selection::range(
            Anchor::new(DocPos::new(1), Affinity::Upstream),
            Anchor::new(DocPos::new(4), Affinity::Upstream),
        ));
        let rects = tree.selection_rects(&selection);
        assert!(!rects.is_empty());
        assert!(rects.iter().all(|rect| rect.width() > 0.0));
    }

    #[test]
    fn extra_source_blank_lines_add_vertical_gap() {
        let mut document = Document::new();
        let first = document.root_children()[0];
        document
            .set_inline_content(first, InlineContent::from_text("a"))
            .unwrap();
        let second = document.append_block(None, paragraph("b")).unwrap();
        let mut hint = document.node(first).unwrap().source().clone();
        hint.style_mut().blank_lines_after = 1;
        document.set_source(first, hint).unwrap();
        let tree = LayoutTree::full(&document, env(300.0));
        let first_layout = tree.block_outer(first).unwrap();
        let second_layout = tree.block_outer(second).unwrap();
        let gap = second_layout.rect.top() - first_layout.rect.bottom();
        assert!(gap > env(300.0).style.block_gap + env(300.0).style.line_height * 0.5);
    }

    #[test]
    fn quote_and_list_layout_children() {
        let mut document = Document::new();
        let quote = document
            .insert_block(None, 0, BlockKind::Quote(vec![]))
            .unwrap();
        document
            .append_block(Some(quote), paragraph("quoted"))
            .unwrap();
        let list = document
            .insert_block(
                Some(quote),
                1,
                BlockKind::List {
                    ordered: false,
                    start: 1,
                    tight: true,
                    items: vec![],
                },
            )
            .unwrap();
        let item = document
            .append_block(
                Some(list),
                BlockKind::ListItem {
                    task: None,
                    children: vec![],
                },
            )
            .unwrap();
        document
            .append_block(Some(item), paragraph("item"))
            .unwrap();
        let tree = LayoutTree::full(&document, env(300.0));
        let quote_layout = tree.block_outer(quote).unwrap();
        assert_eq!(quote_layout.children.len(), 2);
        assert_eq!(tree.block_outer(list).unwrap().children.len(), 1);
    }

    #[test]
    fn table_layout_contains_rows_and_cells() {
        let mut document = Document::new();
        let table = document
            .insert_block(
                None,
                0,
                BlockKind::Table {
                    aligns: vec![Align::Left; 2],
                    rows: vec![],
                },
            )
            .unwrap();
        let row = document
            .append_block(Some(table), BlockKind::TableRow { cells: vec![] })
            .unwrap();
        for text in ["a", "b"] {
            let cell = document
                .append_block(
                    Some(row),
                    BlockKind::TableCell {
                        content: InlineContent::from_text(text),
                    },
                )
                .unwrap();
            assert!(document.node(cell).is_some());
        }
        let tree = LayoutTree::full(&document, env(300.0));
        let table_layout = tree.block_outer(table).unwrap();
        assert_eq!(table_layout.children.len(), 1);
        assert_eq!(table_layout.children[0].children.len(), 2);
    }

    #[test]
    fn viewport_visible_excludes_far_blocks() {
        let mut document = Document::new();
        for index in 0..20 {
            if index == 0 {
                document
                    .set_inline_content(
                        document.root_children()[0],
                        InlineContent::from_text("first"),
                    )
                    .unwrap();
            } else {
                document.append_block(None, paragraph("line")).unwrap();
            }
        }
        let tree = LayoutTree::full(&document, env(300.0));
        let visible = tree.visible(Rect::new(0.0, 0.0, 300.0, 40.0));
        assert!(!visible.is_empty());
        let last = *document.root_children().last().unwrap();
        assert!(!visible.contains(&last));
    }

    #[test]
    fn incremental_layout_equals_full_after_edits() {
        let mut document = Document::new();
        document
            .set_inline_content(
                document.root_children()[0],
                InlineContent::from_text("alpha"),
            )
            .unwrap();
        document.append_block(None, paragraph("beta")).unwrap();
        let mut engine = LayoutEngine::new();
        let first = engine.sync(&document, env(120.0));
        let root = document.root_children()[0];
        document
            .set_inline_content(root, InlineContent::from_text("alpha beta gamma"))
            .unwrap();
        let incremental = engine.sync(&document, env(120.0));
        let full = LayoutTree::full(&document, env(120.0));
        assert_eq!(incremental, full);
        assert_ne!(first, incremental);
    }

    #[test]
    fn incremental_matches_full_for_5000_edits() {
        let mut document = Document::new();
        let root = document.root_children()[0];
        let mut engine = LayoutEngine::new();
        for index in 0..5000 {
            let text = match index % 4 {
                0 => "a",
                1 => "ab",
                2 => "abc",
                _ => "abcd",
            };
            document
                .set_inline_content(root, InlineContent::from_text(text))
                .unwrap();
            let incremental = engine.sync(&document, env(80.0));
            let full = LayoutTree::full(&document, env(80.0));
            assert_eq!(incremental, full);
        }
    }
}

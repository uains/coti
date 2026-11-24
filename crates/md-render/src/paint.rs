//! Pure paint planning from layout geometry.

use md_core::Rect;
use md_document::{AtomKind, BlockKind, DocPos, Document, SelectionSet};
use md_layout::{BlockLayout, LayoutKind, LayoutTree};

use crate::theme::{Color, PaintCommand, PaintLayer, PaintPlan, Theme};

/// Builds a deterministic paint plan without touching the document.
#[must_use]
pub fn build_paint_plan(
    layout: &LayoutTree,
    document: &Document,
    selection: &SelectionSet,
    caret: Option<DocPos>,
    theme: &Theme,
) -> PaintPlan {
    let mut plan = PaintPlan::default();

    for block in &layout.blocks {
        paint_block_backgrounds(block, &mut plan, theme);
    }

    for block in &layout.blocks {
        paint_block_decorations(block, &mut plan, theme);
    }

    for rect in layout.selection_rects(selection) {
        plan.push(
            PaintLayer::Selection,
            PaintCommand::FillRect {
                rect,
                color: theme.selection,
            },
        );
    }

    for block in &layout.blocks {
        paint_text(block, &mut plan, theme, None);
    }

    for block in &layout.blocks {
        paint_atoms(block, document, &mut plan);
    }

    if let Some(pos) = caret
        && let Some(rect) = layout.caret_rect(pos)
    {
        plan.push(
            PaintLayer::Caret,
            PaintCommand::Caret {
                rect: Rect::new(rect.left(), rect.top(), theme.caret_width, rect.height()),
                color: theme.caret,
            },
        );
    }

    plan
}

fn paint_block_backgrounds(block: &BlockLayout, plan: &mut PaintPlan, theme: &Theme) {
    let color = match block.kind {
        LayoutKind::CodeBlock | LayoutKind::Html | LayoutKind::Math | LayoutKind::FrontMatter => {
            Some(theme.code_background)
        }
        LayoutKind::Quote | LayoutKind::Footnote => Some(theme.quote_background),
        LayoutKind::Table | LayoutKind::TableRow => Some(theme.background),
        _ => None,
    };
    if let Some(color) = color {
        plan.push(
            PaintLayer::BlockBackground,
            PaintCommand::FillRect {
                rect: block.rect,
                color,
            },
        );
    }
    for child in &block.children {
        paint_block_backgrounds(child, plan, theme);
    }
}

fn paint_text(block: &BlockLayout, plan: &mut PaintPlan, theme: &Theme, inherited: Option<Color>) {
    let color = inherited.unwrap_or(match block.kind {
        LayoutKind::Heading(_) => theme.heading,
        LayoutKind::CodeBlock | LayoutKind::Html | LayoutKind::Math | LayoutKind::FrontMatter => {
            theme.code_text
        }
        LayoutKind::Quote | LayoutKind::Footnote => theme.quote_text,
        _ => theme.text,
    });
    for line in &block.lines {
        let mut text = String::new();
        let mut start_x = line.rect.left();
        let mut cursor_x = line.rect.left();
        for cluster in &line.clusters {
            if cluster.hard_break {
                continue;
            }
            if let Some(AtomKind::Image(image)) = &cluster.atom {
                if !text.is_empty() {
                    plan.push(
                        PaintLayer::Glyphs,
                        PaintCommand::Text {
                            rect: Rect::new(
                                start_x,
                                line.rect.top(),
                                (cursor_x - start_x).max(1.0),
                                line.rect.height(),
                            ),
                            text: std::mem::take(&mut text),
                            color,
                            font_size: line.font_size,
                            font_weight: line.font_weight,
                            mono: line.mono,
                        },
                    );
                }
                let width = cluster.advance.max(64.0);
                plan.push(
                    PaintLayer::Atoms,
                    PaintCommand::Image {
                        rect: Rect::new(cursor_x, line.rect.top(), width, line.rect.height()),
                        source: image.destination().to_owned(),
                    },
                );
                cursor_x += width;
                start_x = cursor_x;
            } else {
                text.push_str(&cluster.text);
                cursor_x += cluster.advance;
            }
        }
        if !text.is_empty() {
            plan.push(
                PaintLayer::Glyphs,
                PaintCommand::Text {
                    rect: Rect::new(
                        start_x,
                        line.rect.top(),
                        (cursor_x - start_x).max(1.0),
                        line.rect.height(),
                    ),
                    text,
                    color,
                    font_size: line.font_size,
                    font_weight: line.font_weight,
                    mono: line.mono,
                },
            );
        }
    }
    let child_inherited = match block.kind {
        LayoutKind::Quote | LayoutKind::Footnote => Some(theme.quote_text),
        _ => inherited,
    };
    for child in &block.children {
        paint_text(child, plan, theme, child_inherited);
    }
}

fn paint_block_decorations(block: &BlockLayout, plan: &mut PaintPlan, theme: &Theme) {
    match block.kind {
        LayoutKind::Quote | LayoutKind::Footnote => {
            plan.push(
                PaintLayer::Decorations,
                PaintCommand::FillRect {
                    rect: Rect::new(
                        block.rect.left() + 2.0,
                        block.rect.top() + 4.0,
                        3.0,
                        (block.rect.height() - 8.0).max(1.0),
                    ),
                    color: theme.quote_bar,
                },
            );
        }
        LayoutKind::CodeBlock | LayoutKind::Html | LayoutKind::Math | LayoutKind::FrontMatter => {
            let thickness = 1.0;
            for rect in [
                Rect::new(
                    block.rect.left(),
                    block.rect.top(),
                    block.rect.width(),
                    thickness,
                ),
                Rect::new(
                    block.rect.left(),
                    block.rect.bottom() - thickness,
                    block.rect.width(),
                    thickness,
                ),
                Rect::new(
                    block.rect.left(),
                    block.rect.top(),
                    thickness,
                    block.rect.height(),
                ),
                Rect::new(
                    block.rect.right() - thickness,
                    block.rect.top(),
                    thickness,
                    block.rect.height(),
                ),
            ] {
                plan.push(
                    PaintLayer::Decorations,
                    PaintCommand::FillRect {
                        rect,
                        color: theme.code_border,
                    },
                );
            }
        }
        LayoutKind::Table => {
            if let Some(header) = block.children.first() {
                plan.push(
                    PaintLayer::Decorations,
                    PaintCommand::FillRect {
                        rect: header.rect,
                        color: theme.table_header,
                    },
                );
            }
            for rect in [
                Rect::new(block.rect.left(), block.rect.top(), block.rect.width(), 1.0),
                Rect::new(
                    block.rect.left(),
                    block.rect.bottom() - 1.0,
                    block.rect.width(),
                    1.0,
                ),
                Rect::new(
                    block.rect.left(),
                    block.rect.top(),
                    1.0,
                    block.rect.height(),
                ),
                Rect::new(
                    block.rect.right() - 1.0,
                    block.rect.top(),
                    1.0,
                    block.rect.height(),
                ),
            ] {
                plan.push(
                    PaintLayer::Decorations,
                    PaintCommand::FillRect {
                        rect,
                        color: theme.border,
                    },
                );
            }
            for (row_index, row) in block.children.iter().enumerate() {
                if row_index + 1 < block.children.len() {
                    plan.push(
                        PaintLayer::Decorations,
                        PaintCommand::FillRect {
                            rect: Rect::new(
                                block.rect.left(),
                                row.rect.bottom() - 1.0,
                                block.rect.width(),
                                1.0,
                            ),
                            color: theme.border,
                        },
                    );
                }
                for (cell_index, _) in row.children.iter().enumerate() {
                    if cell_index + 1 == row.children.len() {
                        continue;
                    }
                    let cell_width = row.rect.width() / row.children.len().max(1) as f32;
                    let x = row.rect.left() + (cell_index + 1) as f32 * cell_width;
                    plan.push(
                        PaintLayer::Decorations,
                        PaintCommand::FillRect {
                            rect: Rect::new(x - 0.5, row.rect.top(), 1.0, row.rect.height()),
                            color: theme.border,
                        },
                    );
                }
            }
        }
        LayoutKind::List { ordered } => {
            for (index, item) in block.children.iter().enumerate() {
                if ordered {
                    plan.push(
                        PaintLayer::Decorations,
                        PaintCommand::Text {
                            rect: Rect::new(
                                item.rect.left() - 18.0,
                                item.rect.top(),
                                18.0,
                                theme.line_height,
                            ),
                            text: format!("{}.", index + 1),
                            color: theme.list_marker,
                            font_size: theme.font_size,
                            font_weight: 400.0,
                            mono: false,
                        },
                    );
                } else {
                    plan.push(
                        PaintLayer::Decorations,
                        PaintCommand::Text {
                            rect: Rect::new(
                                item.rect.left() - 10.0,
                                item.rect.top(),
                                10.0,
                                theme.line_height,
                            ),
                            text: "•".to_owned(),
                            color: theme.list_marker,
                            font_size: theme.font_size,
                            font_weight: 400.0,
                            mono: false,
                        },
                    );
                }
            }
        }
        _ => {}
    }
    for child in &block.children {
        paint_block_decorations(child, plan, theme);
    }
}

fn paint_atoms(block: &BlockLayout, document: &Document, plan: &mut PaintPlan) {
    if block.kind == LayoutKind::Image
        && let Some(BlockKind::ImageBlock { image, .. }) = document.kind(block.id)
    {
        plan.push(
            PaintLayer::Atoms,
            PaintCommand::Image {
                rect: block.rect,
                source: image.destination().to_owned(),
            },
        );
    }
    for child in &block.children {
        paint_atoms(child, document, plan);
    }
}

/// Returns whether a plan contains a command at a layer.
#[must_use]
pub fn has_layer(plan: &PaintPlan, layer: PaintLayer) -> bool {
    plan.items.iter().any(|item| item.layer == layer)
}

/// Returns a transparent color for tests and overlays.
#[must_use]
pub const fn transparent() -> Color {
    Color::TRANSPARENT
}

#[cfg(test)]
mod tests {
    use super::build_paint_plan;
    use crate::theme::{PaintCommand, PaintLayer, Theme};
    use md_core::Size;
    use md_document::{Affinity, Anchor, DocPos, Document, InlineContent, Selection, SelectionSet};
    use md_layout::{LayoutEnv, LayoutTree, ResolvedStyle};

    #[test]
    fn table_grid_uses_column_boundaries_and_skips_last_cell() {
        let mut document = Document::new();
        let table = document
            .insert_block(
                None,
                0,
                md_document::BlockKind::Table {
                    aligns: vec![md_document::Align::Left; 2],
                    rows: vec![],
                },
            )
            .unwrap();
        let row = document
            .append_block(
                Some(table),
                md_document::BlockKind::TableRow { cells: vec![] },
            )
            .unwrap();
        for text in ["a", "b"] {
            document
                .append_block(
                    Some(row),
                    md_document::BlockKind::TableCell {
                        content: InlineContent::from_text(text),
                    },
                )
                .unwrap();
        }
        let env = LayoutEnv::new(Size::new(300.0, 200.0), ResolvedStyle::default());
        let layout = LayoutTree::full(&document, env);
        let table_layout = &layout.blocks[0];
        let middle = table_layout.rect.left() + table_layout.rect.width() / 2.0;
        let cell = &table_layout.children[0].children[0];
        let plan = build_paint_plan(
            &layout,
            &document,
            &SelectionSet::new(Selection::collapsed(DocPos::ZERO, Affinity::Downstream)),
            None,
            &Theme::default(),
        );
        let verticals: Vec<_> = plan
            .items
            .iter()
            .filter_map(|item| match &item.command {
                PaintCommand::FillRect { rect, color }
                    if *color == Theme::default().border
                        && rect.width() <= 1.0
                        && rect.height() > 20.0 =>
                {
                    Some(*rect)
                }
                _ => None,
            })
            .collect();
        assert!(
            verticals
                .iter()
                .any(|rect| (rect.left() - middle).abs() < 1.5),
            "missing column divider: {verticals:?}"
        );
        assert!(
            !verticals
                .iter()
                .any(|rect| (rect.left() - (cell.rect.right() - 1.0)).abs() < 0.75),
            "last cell still has inset divider: {verticals:?}"
        );
    }

    #[test]
    fn paint_plan_orders_layers_and_does_not_mutate_document() {
        let mut document = Document::new();
        document
            .set_inline_content(
                document.root_children()[0],
                InlineContent::from_text("hello"),
            )
            .unwrap();
        let before = document.clone();
        let env = LayoutEnv::new(Size::new(300.0, 200.0), ResolvedStyle::default());
        let layout = LayoutTree::full(&document, env);
        let selection = SelectionSet::new(Selection::range(
            Anchor::new(DocPos::new(1), Affinity::Upstream),
            Anchor::new(DocPos::new(3), Affinity::Upstream),
        ));
        let plan = build_paint_plan(
            &layout,
            &document,
            &selection,
            Some(DocPos::new(2)),
            &Theme::default(),
        );
        assert_eq!(document, before);
        let selection_index = plan
            .items
            .iter()
            .position(|item| item.layer == PaintLayer::Selection)
            .unwrap();
        let glyph_index = plan
            .items
            .iter()
            .position(|item| item.layer == PaintLayer::Glyphs)
            .unwrap();
        let caret_index = plan
            .items
            .iter()
            .position(|item| item.layer == PaintLayer::Caret)
            .unwrap();
        assert!(selection_index < glyph_index);
        assert!(glyph_index < caret_index);
        assert!(matches!(
            plan.items[caret_index].command,
            PaintCommand::Caret { .. }
        ));
    }
}

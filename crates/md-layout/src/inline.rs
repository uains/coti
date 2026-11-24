//! Inline cluster layout, line breaking, and caret geometry.

use md_core::{Point, Rect, Size};
use md_document::{AtomKind, InlineContent, Run};
use unicode_segmentation::UnicodeSegmentation;

use crate::measure::TextMeasurer;
use crate::style::MeasureStyle;

/// One measured grapheme or atom.
#[derive(Clone, Debug, PartialEq)]
pub struct Cluster {
    /// Display text, or an atom placeholder.
    pub text: String,
    /// Inclusive local start offset.
    pub start: usize,
    /// Exclusive local end offset.
    pub end: usize,
    /// Measured advance.
    pub advance: f32,
    /// Atom payload when this cluster is atomic.
    pub atom: Option<AtomKind>,
    /// Whether this cluster forces a line break.
    pub hard_break: bool,
}

/// One laid-out line.
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    /// Clusters on this line.
    pub clusters: Vec<Cluster>,
    /// Absolute line rectangle.
    pub rect: Rect,
    /// Inclusive local start.
    pub start: usize,
    /// Exclusive local end.
    pub end: usize,
    /// Baseline y coordinate.
    pub baseline: f32,
    /// Font size used for this line.
    pub font_size: f32,
    /// Font weight used for this line.
    pub font_weight: f32,
}

impl Line {
    /// Returns the caret x coordinate at a local offset.
    #[must_use]
    pub fn x_for_offset(&self, offset: usize) -> f32 {
        if self.clusters.is_empty() {
            return self.rect.left();
        }
        let mut x = self.rect.left();
        for cluster in &self.clusters {
            if offset <= cluster.start {
                return x;
            }
            if offset <= cluster.end {
                let span = cluster.end.saturating_sub(cluster.start).max(1) as f32;
                let within = offset.saturating_sub(cluster.start) as f32 / span;
                return x + cluster.advance * within;
            }
            x += cluster.advance;
        }
        self.rect.right()
    }

    /// Returns the nearest local offset for an x coordinate.
    #[must_use]
    pub fn offset_at_x(&self, x: f32) -> usize {
        if self.clusters.is_empty() {
            return self.start;
        }
        let mut current = self.rect.left();
        for cluster in &self.clusters {
            let next = current + cluster.advance;
            if x < (current + next) * 0.5 {
                return cluster.start;
            }
            current = next;
        }
        self.end
    }

    /// Returns the rectangle covering a local range on this line.
    #[must_use]
    pub fn rect_for_range(&self, start: usize, end: usize) -> Option<Rect> {
        if end < self.start || start > self.end {
            return None;
        }
        let from = self.x_for_offset(start.max(self.start));
        let to = self.x_for_offset(end.min(self.end));
        Some(Rect::new(
            from.min(to),
            self.rect.top(),
            (to - from).abs().max(1.0),
            self.rect.height(),
        ))
    }
}

/// Laid-out inline content.
#[derive(Clone, Debug, PartialEq)]
pub struct InlineLayout {
    /// Lines in visual order.
    pub lines: Vec<Line>,
    /// Bounding size.
    pub size: Size,
}

impl InlineLayout {
    /// Returns the line containing a local offset.
    #[must_use]
    pub fn line_for_offset(&self, offset: usize) -> Option<&Line> {
        self.lines
            .iter()
            .find(|line| offset >= line.start && offset <= line.end)
            .or_else(|| self.lines.last())
    }
}

/// Lays out inline content into wrapped lines.
#[must_use]
pub fn layout_inline(
    content: &InlineContent,
    style: &MeasureStyle,
    measurer: &dyn TextMeasurer,
    width: f32,
    origin: Point,
) -> InlineLayout {
    let max_width = width.max(1.0);
    let metrics = measurer.line_metrics(style);
    let mut clusters = Vec::new();
    let mut base = 0;
    for run in content.runs() {
        match run {
            Run::Text { text, .. } => {
                for (byte, grapheme) in text.as_str().grapheme_indices(true) {
                    let start = base + byte;
                    clusters.push(Cluster {
                        text: grapheme.to_owned(),
                        start,
                        end: start + grapheme.len(),
                        advance: measurer.advance(grapheme, style),
                        atom: None,
                        hard_break: false,
                    });
                }
                base = clusters.last().map_or(base, |cluster| cluster.end);
            }
            Run::Atom { kind, .. } => {
                let start = base;
                let hard_break = matches!(kind, AtomKind::HardBreak);
                clusters.push(Cluster {
                    text: match kind {
                        AtomKind::HardBreak => "\n".to_owned(),
                        AtomKind::SoftBreak => " ".to_owned(),
                        AtomKind::Image(_) => "[image]".to_owned(),
                        AtomKind::InlineMath(source) => source.as_str().to_owned(),
                        AtomKind::FootnoteRef(label) => label.clone(),
                    },
                    start,
                    end: start + 1,
                    advance: measurer.atom_advance(kind, style),
                    atom: Some(kind.clone()),
                    hard_break,
                });
                base = start + 1;
            }
        }
    }

    let mut lines = Vec::new();
    let mut current = Vec::new();
    let mut current_width = 0.0;
    for cluster in clusters {
        if cluster.hard_break {
            lines.push(make_line(
                std::mem::take(&mut current),
                origin,
                lines.len(),
                &metrics,
                style.font_size,
                style.weight,
            ));
            current_width = 0.0;
            continue;
        }
        if current_width + cluster.advance > max_width && !current.is_empty() {
            let split = current
                .iter()
                .rposition(|item: &Cluster| item.text == " " && item.start > current[0].start);
            if let Some(split) = split.filter(|split| *split > 0) {
                let remainder: Vec<Cluster> = current.split_off(split + 1);
                current_width = remainder.iter().map(|item| item.advance).sum();
                lines.push(make_line(
                    std::mem::take(&mut current),
                    origin,
                    lines.len(),
                    &metrics,
                    style.font_size,
                    style.weight,
                ));
                current = remainder;
            } else {
                lines.push(make_line(
                    std::mem::take(&mut current),
                    origin,
                    lines.len(),
                    &metrics,
                    style.font_size,
                    style.weight,
                ));
                current_width = 0.0;
            }
        }
        current_width += cluster.advance;
        current.push(cluster);
    }
    if !current.is_empty() || lines.is_empty() {
        lines.push(make_line(
            std::mem::take(&mut current),
            origin,
            lines.len(),
            &metrics,
            style.font_size,
            style.weight,
        ));
    }

    let size = Size::new(width.max(0.0), metrics.line_height * lines.len() as f32);
    InlineLayout { lines, size }
}

fn make_line(
    clusters: Vec<Cluster>,
    origin: Point,
    index: usize,
    metrics: &crate::measure::LineMetrics,
    font_size: f32,
    font_weight: f32,
) -> Line {
    let start = clusters.first().map_or(0, |cluster| cluster.start);
    let end = clusters.last().map_or(start, |cluster| cluster.end);
    let width = clusters.iter().map(|cluster| cluster.advance).sum::<f32>();
    let top = origin.y + index as f32 * metrics.line_height;
    Line {
        clusters,
        rect: Rect::new(origin.x, top, width, metrics.line_height),
        start,
        end,
        baseline: top + metrics.ascent,
        font_size,
        font_weight,
    }
}

#[cfg(test)]
mod tests {
    use super::layout_inline;
    use crate::measure::FakeMeasurer;
    use crate::style::MeasureStyle;
    use md_core::Point;
    use md_document::{AtomKind, InlineContent, Marks};

    #[test]
    fn ascii_text_wraps_at_width() {
        let measurer = FakeMeasurer::new();
        let layout = layout_inline(
            &InlineContent::from_text("abcd"),
            &MeasureStyle::default(),
            &measurer,
            20.0,
            Point::ZERO,
        );
        assert!(layout.lines.len() >= 2);
    }

    #[test]
    fn cjk_text_wraps_between_characters() {
        let layout = layout_inline(
            &InlineContent::from_text("中文"),
            &MeasureStyle::new(14.0, 20.0),
            &FakeMeasurer::new(),
            20.0,
            Point::ZERO,
        );
        assert_eq!(layout.lines.len(), 2);
    }

    #[test]
    fn caret_x_and_offset_round_trip() {
        let layout = layout_inline(
            &InlineContent::from_text("abc"),
            &MeasureStyle::default(),
            &FakeMeasurer::new(),
            200.0,
            Point::ZERO,
        );
        let line = &layout.lines[0];
        let x = line.x_for_offset(1);
        assert_eq!(line.offset_at_x(x), 1);
    }

    #[test]
    fn hard_break_forces_new_line() {
        let mut content = InlineContent::from_text("a");
        content
            .insert_atom_at(1, AtomKind::HardBreak, Marks::plain())
            .unwrap();
        content.append_content(&InlineContent::from_text("b"));
        let layout = layout_inline(
            &content,
            &MeasureStyle::default(),
            &FakeMeasurer::new(),
            200.0,
            Point::ZERO,
        );
        assert_eq!(layout.lines.len(), 2);
    }
}

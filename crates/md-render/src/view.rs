//! Render view state assembled from layout, theme, and input adapters.

use md_core::{Rect, Size};
use md_document::{DocPos, Document, SelectionSet};
use md_layout::{LayoutEnv, LayoutTree, ResolvedStyle};

use crate::input::{CaretBlink, InputAdapter, ScrollState};
use crate::paint::{build_paint_plan, build_paint_plan_visible};
use crate::theme::{PaintPlan, Theme};

/// GPUI-facing view state. It owns no document and never mutates it.
#[derive(Debug)]
pub struct RenderView {
    /// Current layout tree.
    pub layout: LayoutTree,
    /// Visual theme.
    pub theme: Theme,
    /// Scroll state.
    pub scroll: ScrollState,
    /// Caret blink state.
    pub blink: CaretBlink,
    /// Input adapter.
    pub input: InputAdapter,
}

impl RenderView {
    /// Creates a view from an existing layout.
    #[must_use]
    pub fn new(layout: LayoutTree, theme: Theme) -> Self {
        let viewport = layout.env.viewport;
        Self {
            layout,
            theme,
            scroll: ScrollState {
                offset_y: 0.0,
                viewport_height: viewport.height,
            },
            blink: CaretBlink::default(),
            input: InputAdapter::default(),
        }
    }

    /// Creates a view with an empty layout, to be filled by a real measurer.
    #[must_use]
    pub fn empty(viewport: Size, style: ResolvedStyle, theme: Theme) -> Self {
        let layout = LayoutTree::empty(LayoutEnv::new(viewport, style));
        Self::new(layout, theme)
    }

    /// Builds the paint plan for the current selection and caret.
    #[must_use]
    pub fn paint(
        &self,
        document: &Document,
        selection: &SelectionSet,
        caret: Option<DocPos>,
    ) -> PaintPlan {
        build_paint_plan(&self.layout, document, selection, caret, &self.theme)
    }

    /// Builds a paint plan restricted to a layout-space viewport.
    #[must_use]
    pub fn paint_visible(
        &self,
        document: &Document,
        selection: &SelectionSet,
        caret: Option<DocPos>,
        viewport: Rect,
    ) -> PaintPlan {
        build_paint_plan_visible(
            &self.layout,
            document,
            selection,
            caret,
            &self.theme,
            viewport,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::RenderView;
    use crate::theme::Theme;
    use md_core::Size;
    use md_document::Affinity;
    use md_document::{DocPos, Document, Selection, SelectionSet};
    use md_layout::{LayoutEnv, LayoutTree, ResolvedStyle};

    #[test]
    fn view_builds_paint_plan_without_document_mutation() {
        let document = Document::new();
        let before = document.clone();
        let layout = LayoutTree::full(
            &document,
            LayoutEnv::new(Size::new(300.0, 200.0), ResolvedStyle::default()),
        );
        let view = RenderView::new(layout, Theme::default());
        let selection = SelectionSet::new(Selection::collapsed(DocPos::ZERO, Affinity::Upstream));
        let plan = view.paint(&document, &selection, Some(DocPos::ZERO));
        assert_eq!(document, before);
        assert!(!plan.items.is_empty());
    }
}

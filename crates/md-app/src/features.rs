//! Headless product features: outline, search, folding, status, export, and
//! command palette data models.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use md_document::{BlockId, BlockKind, DocPos, Document, InlineContent};

use crate::commands::{self, Command};

/// Heading outline entry.
#[derive(Clone, Debug, PartialEq)]
pub struct OutlineItem {
    /// Heading block.
    pub block_id: BlockId,
    /// Heading level 1..=6.
    pub level: u8,
    /// Plain heading text.
    pub title: String,
    /// Document position of the heading.
    pub pos: DocPos,
}

/// Builds a flat, document-ordered heading outline.
#[must_use]
pub fn build_outline(document: &Document) -> Vec<OutlineItem> {
    let mut items = Vec::new();
    collect_outline(document, document.root_children(), &mut items);
    items
}

fn collect_outline(document: &Document, blocks: &[BlockId], items: &mut Vec<OutlineItem>) {
    for block in blocks {
        if let Some(BlockKind::Heading { level, content }) = document.kind(*block) {
            let pos = document
                .index()
                .block_location(*block)
                .map_or(DocPos::ZERO, |location| DocPos::new(location.doc_start));
            items.push(OutlineItem {
                block_id: *block,
                level: *level,
                title: content.plain_text(),
                pos,
            });
        }
        if let Some(children) = document.children(*block) {
            collect_outline(document, children, items);
        }
    }
}

/// One search hit over the serialized Markdown source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SearchMatch {
    /// Inclusive byte start.
    pub start: usize,
    /// Exclusive byte end.
    pub end: usize,
}

/// Incremental search state.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SearchState {
    /// Query text.
    pub query: String,
    /// All hits.
    pub matches: Vec<SearchMatch>,
    /// Current hit index.
    pub current: Option<usize>,
}

impl SearchState {
    /// Recomputes hits for a source.
    pub fn search(&mut self, source: &str, query: &str, case_sensitive: bool) {
        self.query = query.to_owned();
        self.matches.clear();
        self.current = None;
        if query.is_empty() {
            return;
        }
        if case_sensitive {
            self.collect(source, query);
        } else {
            self.collect(&source.to_lowercase(), &query.to_lowercase());
        }
        if !self.matches.is_empty() {
            self.current = Some(0);
        }
    }

    fn collect(&mut self, haystack: &str, needle: &str) {
        let mut start = 0;
        while let Some(found) = haystack[start..].find(needle) {
            let offset = start + found;
            self.matches.push(SearchMatch {
                start: offset,
                end: offset + needle.len(),
            });
            start = offset + needle.len().max(1);
        }
    }

    /// Moves to the next hit, wrapping.
    pub fn next_match(&mut self) -> Option<SearchMatch> {
        if self.matches.is_empty() {
            return None;
        }
        let next = self
            .current
            .map_or(0, |index| (index + 1) % self.matches.len());
        self.current = Some(next);
        self.matches.get(next).copied()
    }

    /// Moves to the previous hit, wrapping.
    pub fn previous_match(&mut self) -> Option<SearchMatch> {
        if self.matches.is_empty() {
            return None;
        }
        let previous = self.current.map_or(0, |index| {
            if index == 0 {
                self.matches.len() - 1
            } else {
                index - 1
            }
        });
        self.current = Some(previous);
        self.matches.get(previous).copied()
    }
}

/// Fold state for heading/quote/list subtrees.
#[derive(Clone, Debug, Default)]
pub struct FoldState {
    folded: HashSet<BlockId>,
}

impl FoldState {
    /// Toggles folding for a block and returns the new state.
    pub fn toggle(&mut self, block: BlockId) -> bool {
        if self.folded.remove(&block) {
            false
        } else {
            self.folded.insert(block);
            true
        }
    }

    /// Returns whether a block is folded.
    #[must_use]
    pub fn is_folded(&self, block: BlockId) -> bool {
        self.folded.contains(&block)
    }

    /// Clears all folds.
    pub fn clear(&mut self) {
        self.folded.clear();
    }

    /// Returns the folded count.
    #[must_use]
    pub fn len(&self) -> usize {
        self.folded.len()
    }

    /// Returns whether no block is folded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.folded.is_empty()
    }
}

/// Returns blocks that can be folded.
#[must_use]
pub fn foldable_blocks(document: &Document) -> Vec<BlockId> {
    let mut blocks = Vec::new();
    fn visit(document: &Document, roots: &[BlockId], blocks: &mut Vec<BlockId>) {
        for id in roots {
            if matches!(
                document.kind(*id),
                Some(BlockKind::Heading { .. } | BlockKind::Quote(_) | BlockKind::List { .. })
            ) {
                blocks.push(*id);
            }
            if let Some(children) = document.children(*id) {
                visit(document, children, blocks);
            }
        }
    }
    visit(document, document.root_children(), &mut blocks);
    blocks
}

/// Status-bar data.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StatusBar {
    /// Character count.
    pub characters: usize,
    /// Word count.
    pub words: usize,
    /// Block count.
    pub blocks: usize,
    /// Estimated reading minutes.
    pub reading_minutes: usize,
    /// Caret position.
    pub position: DocPos,
}

/// Computes status-bar data.
#[must_use]
pub fn status_bar(document: &Document, position: DocPos) -> StatusBar {
    let text = document_plain_text(document);
    let characters = text.chars().count();
    let words = text.split_whitespace().count();
    StatusBar {
        characters,
        words,
        blocks: document.block_count(),
        reading_minutes: words.div_ceil(300).max(1),
        position,
    }
}

/// Exports a plain-text projection.
#[must_use]
pub fn export_plain_text(document: &Document) -> String {
    let mut output = String::new();
    fn visit(document: &Document, blocks: &[BlockId], output: &mut String) {
        for id in blocks {
            let Some(kind) = document.kind(*id) else {
                continue;
            };
            match kind {
                BlockKind::Paragraph(content)
                | BlockKind::Heading { content, .. }
                | BlockKind::TableCell { content } => {
                    output.push_str(&content.plain_text());
                    output.push('\n');
                }
                BlockKind::CodeBlock { text, .. }
                | BlockKind::HtmlBlock(text)
                | BlockKind::MathBlock(text)
                | BlockKind::FrontMatter(text) => {
                    output.push_str(text.as_str());
                    output.push('\n');
                }
                BlockKind::ThematicBreak => output.push_str("---\n"),
                BlockKind::ImageBlock { alt, .. } => {
                    output.push_str(alt);
                    output.push('\n');
                }
                _ => {}
            }
            if let Some(children) = document.children(*id) {
                visit(document, children, output);
            }
        }
    }
    visit(document, document.root_children(), &mut output);
    output.trim_end().to_owned()
}

fn document_plain_text(document: &Document) -> String {
    let mut output = String::new();
    fn visit(document: &Document, blocks: &[BlockId], output: &mut String) {
        for id in blocks {
            match document.kind(*id) {
                Some(BlockKind::Paragraph(content))
                | Some(BlockKind::Heading { content, .. })
                | Some(BlockKind::TableCell { content }) => {
                    output.push_str(&content.plain_text());
                    output.push(' ');
                }
                Some(BlockKind::CodeBlock { text, .. }) => {
                    output.push_str(text.as_str());
                    output.push(' ');
                }
                _ => {}
            }
            if let Some(children) = document.children(*id) {
                visit(document, children, output);
            }
        }
    }
    visit(document, document.root_children(), &mut output);
    output
}

/// Filters commands by labels, ids, and shortcuts.
#[must_use]
pub fn filter_commands(query: &str) -> Vec<Command> {
    let query = query.trim().to_lowercase();
    let mut commands = Vec::new();
    for menu in commands::menus() {
        for item in menu.items {
            if (query.is_empty()
                || item.label.to_lowercase().contains(&query)
                || item.command.id().contains(&query)
                || item.command.shortcut().to_lowercase().contains(&query))
                && !commands.contains(&item.command)
            {
                commands.push(item.command);
            }
        }
    }
    commands
}

/// Resolves an image destination relative to a document path.
#[must_use]
pub fn resolve_image_path(document_path: Option<&Path>, destination: &str) -> PathBuf {
    let path = PathBuf::from(destination);
    if path.is_absolute() {
        return path;
    }
    document_path
        .and_then(Path::parent)
        .map_or(path.clone(), |parent| parent.join(path))
}

/// Returns the inline content text for tests and exporters.
#[must_use]
pub fn inline_text(content: &InlineContent) -> String {
    content.plain_text()
}

#[cfg(test)]
mod tests {
    use super::{
        SearchState, build_outline, export_plain_text, filter_commands, foldable_blocks,
        resolve_image_path, status_bar,
    };
    use md_document::{BlockKind, DocPos, Document, InlineContent};
    use std::path::Path;

    fn heading_doc() -> Document {
        let mut document = Document::new();
        document
            .set_block_kind(
                document.root_children()[0],
                BlockKind::Heading {
                    level: 1,
                    content: InlineContent::from_text("Title"),
                },
            )
            .unwrap();
        document
            .append_block(
                None,
                BlockKind::Paragraph(InlineContent::from_text("body text")),
            )
            .unwrap();
        document
    }

    #[test]
    fn outline_contains_headings() {
        let items = build_outline(&heading_doc());
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "Title");
        assert_eq!(items[0].level, 1);
        assert_eq!(items[0].pos, DocPos::ZERO);
    }

    #[test]
    fn search_next_and_previous_wrap() {
        let mut search = SearchState::default();
        search.search("one two one", "one", true);
        assert_eq!(search.matches.len(), 2);
        assert_eq!(search.next_match().unwrap().start, 8);
        assert_eq!(search.next_match().unwrap().start, 0);
        assert_eq!(search.next_match().unwrap().start, 8);
        assert_eq!(search.previous_match().unwrap().start, 0);
    }

    #[test]
    fn fold_and_status_and_export_work() {
        let document = heading_doc();
        let foldable = foldable_blocks(&document);
        assert!(!foldable.is_empty());
        let status = status_bar(&document, DocPos::ZERO);
        assert!(status.characters > 0);
        assert_eq!(status.blocks, document.block_count());
        let text = export_plain_text(&document);
        assert!(text.contains("Title"));
        assert!(text.contains("body text"));
    }

    #[test]
    fn command_palette_and_image_resolution() {
        assert!(!filter_commands("save").is_empty());
        assert_eq!(
            resolve_image_path(Some(Path::new("book/note.md")), "img/a.png"),
            Path::new("book/img/a.png")
        );
    }
}

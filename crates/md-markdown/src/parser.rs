//! pulldown-cmark event stream to `md-document` builder.

use std::ops::Range;
use std::sync::Arc;

use md_core::TextBuf;
use md_document::{
    Align, BlockId, BlockKind, Document, FenceStyle, ImageRef, InlineContent, LinkRef, MarkFlags,
    Marks, SourceHint,
};
use pulldown_cmark::{Alignment, CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

use crate::{MarkdownDocument, MarkdownError, MarkdownResult};

/// Parses CommonMark + GFM into a [`MarkdownDocument`].
pub(crate) fn parse(source: &str) -> MarkdownResult<MarkdownDocument> {
    let source: Arc<str> = Arc::from(source);
    let mut builder = Builder::new(Arc::clone(&source));

    for (event, range) in Parser::new_ext(&source, options()).into_offset_iter() {
        builder.handle(event, range)?;
    }
    builder.finish()?;

    Ok(MarkdownDocument {
        document: builder.document,
        source,
        dirty: false,
    })
}

fn options() -> Options {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_MATH);
    options.insert(Options::ENABLE_YAML_STYLE_METADATA_BLOCKS);
    options.insert(Options::ENABLE_GFM);
    options
}

struct OpenBlock {
    id: BlockId,
    start: usize,
}

struct Builder {
    document: Document,
    source: Arc<str>,
    containers: Vec<BlockId>,
    open: Vec<OpenBlock>,
    inline: Option<InlineBuilder>,
}

impl Builder {
    fn new(source: Arc<str>) -> Self {
        Self {
            document: Document::new_empty(),
            source,
            containers: Vec::new(),
            open: Vec::new(),
            inline: None,
        }
    }

    fn handle(&mut self, event: Event<'_>, range: Range<usize>) -> MarkdownResult<()> {
        match event {
            Event::Start(tag) => {
                self.start(tag, range.start)?;
            }
            Event::End(tag) => {
                self.end(tag, range.end)?;
            }
            Event::Text(text) => self.text(&text),
            Event::Code(code) => {
                if let Some(inline) = self.inline.as_mut() {
                    inline.code(&code);
                }
            }
            Event::InlineMath(math) => {
                if let Some(inline) = self.inline.as_mut() {
                    inline.atom(md_document::AtomKind::InlineMath(TextBuf::from(
                        math.as_ref(),
                    )));
                }
            }
            Event::DisplayMath(math) => {
                let id = self.append_leaf(
                    BlockKind::MathBlock(TextBuf::from(math.as_ref())),
                    range.start,
                )?;
                self.set_source(id, range)?;
            }
            Event::Html(html) | Event::InlineHtml(html) => {
                if let Some(inline) = self.inline.as_mut() {
                    inline.text(&html);
                } else if let Some(id) = self.open.last().map(|open| open.id)
                    && let Some(text) = self.document.raw_text_mut(id)
                {
                    text.append(&html);
                }
            }
            Event::FootnoteReference(label) => {
                if let Some(inline) = self.inline.as_mut() {
                    inline.atom(md_document::AtomKind::FootnoteRef(label.to_string()));
                }
            }
            Event::SoftBreak => {
                if let Some(inline) = self.inline.as_mut() {
                    inline.atom(md_document::AtomKind::SoftBreak);
                }
            }
            Event::HardBreak => {
                if let Some(inline) = self.inline.as_mut() {
                    inline.atom(md_document::AtomKind::HardBreak);
                }
            }
            Event::Rule => {
                let id = self.append_leaf(BlockKind::ThematicBreak, range.start)?;
                self.set_source(id, range)?;
            }
            Event::TaskListMarker(checked) => {
                if let Some(id) = self.open.last().map(|open| open.id) {
                    self.document.set_list_item_task(id, Some(checked))?;
                }
            }
        }
        Ok(())
    }

    fn start(&mut self, tag: Tag<'_>, start: usize) -> MarkdownResult<()> {
        match tag {
            Tag::Paragraph => self.start_inline(BlockKind::Paragraph(InlineContent::new()), start),
            Tag::Heading { level, .. } => self.start_inline(
                BlockKind::Heading {
                    level: heading_level(level),
                    content: InlineContent::new(),
                },
                start,
            ),
            Tag::BlockQuote(_) => self.start_container(BlockKind::Quote(vec![]), start),
            Tag::CodeBlock(kind) => {
                let (lang, fence) = code_block_info(kind);
                self.start_leaf(
                    BlockKind::CodeBlock {
                        lang,
                        text: TextBuf::new(),
                        fence,
                    },
                    start,
                )
            }
            Tag::HtmlBlock => self.start_leaf(BlockKind::HtmlBlock(TextBuf::new()), start),
            Tag::List(start_number) => {
                let ordered = start_number.is_some();
                let start_number = start_number.unwrap_or(1).min(u32::MAX as u64) as u32;
                self.start_container(
                    BlockKind::List {
                        ordered,
                        start: start_number,
                        tight: true,
                        items: vec![],
                    },
                    start,
                )
            }
            Tag::Item => self.start_container(
                BlockKind::ListItem {
                    task: None,
                    children: vec![],
                },
                start,
            ),
            Tag::FootnoteDefinition(_) => self.start_container(BlockKind::Quote(vec![]), start),
            Tag::DefinitionList => self.start_container(BlockKind::Quote(vec![]), start),
            Tag::DefinitionListTitle => {
                self.start_inline(BlockKind::Paragraph(InlineContent::new()), start)
            }
            Tag::DefinitionListDefinition => self.start_container(BlockKind::Quote(vec![]), start),
            Tag::Table(aligns) => {
                let aligns = aligns.into_iter().map(map_align).collect();
                self.start_container(
                    BlockKind::Table {
                        aligns,
                        rows: vec![],
                    },
                    start,
                )
            }
            Tag::TableHead | Tag::TableRow => {
                self.start_container(BlockKind::TableRow { cells: vec![] }, start)
            }
            Tag::TableCell => self.start_inline(
                BlockKind::TableCell {
                    content: InlineContent::new(),
                },
                start,
            ),
            Tag::Emphasis => {
                if let Some(inline) = self.inline.as_mut() {
                    inline.start_flag(MarkFlags::EMPHASIS);
                }
                Ok(())
            }
            Tag::Strong => {
                if let Some(inline) = self.inline.as_mut() {
                    inline.start_flag(MarkFlags::STRONG);
                }
                Ok(())
            }
            Tag::Strikethrough => {
                if let Some(inline) = self.inline.as_mut() {
                    inline.start_flag(MarkFlags::STRIKE);
                }
                Ok(())
            }
            Tag::Superscript | Tag::Subscript => Ok(()),
            Tag::Link {
                dest_url, title, ..
            } => {
                if let Some(inline) = self.inline.as_mut() {
                    inline.start_link(dest_url.to_string(), title.to_string());
                }
                Ok(())
            }
            Tag::Image {
                dest_url, title, ..
            } => {
                if let Some(inline) = self.inline.as_mut() {
                    inline.start_image(dest_url.to_string(), title.to_string());
                }
                Ok(())
            }
            Tag::MetadataBlock(_) => self.start_leaf(BlockKind::FrontMatter(TextBuf::new()), start),
        }
    }

    fn end(&mut self, tag: TagEnd, end: usize) -> MarkdownResult<()> {
        match tag {
            TagEnd::Paragraph | TagEnd::Heading(_) | TagEnd::TableCell => self.finish_inline(end),
            TagEnd::BlockQuote(_)
            | TagEnd::List(_)
            | TagEnd::Item
            | TagEnd::FootnoteDefinition
            | TagEnd::DefinitionList
            | TagEnd::DefinitionListDefinition
            | TagEnd::Table
            | TagEnd::TableHead
            | TagEnd::TableRow => self.finish_container(end),
            TagEnd::CodeBlock | TagEnd::HtmlBlock | TagEnd::MetadataBlock(_) => {
                self.finish_leaf(end)
            }
            TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough | TagEnd::Link => {
                if let Some(inline) = self.inline.as_mut() {
                    inline.end_marks();
                }
                Ok(())
            }
            TagEnd::Superscript | TagEnd::Subscript => Ok(()),
            TagEnd::Image => {
                if let Some(inline) = self.inline.as_mut() {
                    inline.end_image();
                }
                Ok(())
            }
            TagEnd::DefinitionListTitle => self.finish_inline(end),
        }
    }

    fn text(&mut self, text: &str) {
        if let Some(inline) = self.inline.as_mut() {
            inline.text(text);
        } else if let Some(id) = self.open.last().map(|open| open.id)
            && let Some(raw) = self.document.raw_text_mut(id)
        {
            raw.append(text);
        }
    }

    fn start_inline(&mut self, kind: BlockKind, start: usize) -> MarkdownResult<()> {
        let id = self.append_leaf(kind, start)?;
        self.inline = Some(InlineBuilder::new());
        self.open.push(OpenBlock { id, start });
        Ok(())
    }

    fn finish_inline(&mut self, end: usize) -> MarkdownResult<()> {
        let Some(open) = self.open.pop() else {
            return Err(MarkdownError::Parse {
                message: "unexpected inline end event".to_owned(),
            });
        };
        let Some(inline) = self.inline.take() else {
            return Err(MarkdownError::Parse {
                message: "inline end without an active builder".to_owned(),
            });
        };
        self.document.set_inline_content(open.id, inline.finish())?;
        self.set_source(open.id, open.start..end)?;
        Ok(())
    }

    fn start_container(&mut self, kind: BlockKind, start: usize) -> MarkdownResult<()> {
        let parent = self.containers.last().copied();
        let id = self.document.append_block(parent, kind)?;
        self.containers.push(id);
        self.open.push(OpenBlock { id, start });
        Ok(())
    }

    fn finish_container(&mut self, end: usize) -> MarkdownResult<()> {
        let Some(open) = self.open.pop() else {
            return Err(MarkdownError::Parse {
                message: "unexpected container end event".to_owned(),
            });
        };
        if self.containers.last() == Some(&open.id) {
            self.containers.pop();
        }
        self.set_source(open.id, open.start..end)?;
        self.ensure_container_child(open.id)?;
        Ok(())
    }

    fn start_leaf(&mut self, kind: BlockKind, start: usize) -> MarkdownResult<()> {
        let id = self.append_leaf(kind, start)?;
        self.open.push(OpenBlock { id, start });
        Ok(())
    }

    fn finish_leaf(&mut self, end: usize) -> MarkdownResult<()> {
        let Some(open) = self.open.pop() else {
            return Err(MarkdownError::Parse {
                message: "unexpected leaf end event".to_owned(),
            });
        };
        self.set_source(open.id, open.start..end)?;
        Ok(())
    }

    fn append_leaf(&mut self, kind: BlockKind, _start: usize) -> MarkdownResult<BlockId> {
        let parent = self.containers.last().copied();
        Ok(self.document.append_block(parent, kind)?)
    }

    fn set_source(&mut self, id: BlockId, range: Range<usize>) -> MarkdownResult<()> {
        self.document
            .set_source(id, SourceHint::pristine(Arc::clone(&self.source), range))?;
        Ok(())
    }

    fn ensure_container_child(&mut self, id: BlockId) -> MarkdownResult<()> {
        let Some(kind) = self.document.kind(id) else {
            return Ok(());
        };
        let needs_child = match kind {
            BlockKind::Quote(children) | BlockKind::ListItem { children, .. } => {
                children.is_empty()
            }
            _ => false,
        };
        if needs_child {
            self.document
                .append_block(Some(id), BlockKind::Paragraph(InlineContent::new()))?;
        }
        Ok(())
    }

    fn finish(&mut self) -> MarkdownResult<()> {
        if self.document.root_children().is_empty() {
            self.document
                .append_block(None, BlockKind::Paragraph(InlineContent::new()))?;
        }
        Ok(())
    }
}

struct InlineBuilder {
    content: InlineContent,
    marks: Marks,
    mark_stack: Vec<Marks>,
    images: Vec<ImageState>,
}

struct ImageState {
    destination: String,
    title: String,
    alt: String,
}

impl InlineBuilder {
    fn new() -> Self {
        Self {
            content: InlineContent::new(),
            marks: Marks::plain(),
            mark_stack: Vec::new(),
            images: Vec::new(),
        }
    }

    fn start_flag(&mut self, flag: MarkFlags) {
        self.mark_stack.push(self.marks.clone());
        self.marks.flags.insert(flag);
    }

    fn start_link(&mut self, destination: String, title: String) {
        self.mark_stack.push(self.marks.clone());
        self.marks.link = Some(link_ref(destination, title));
    }

    fn end_marks(&mut self) {
        if let Some(marks) = self.mark_stack.pop() {
            self.marks = marks;
        }
    }

    fn start_image(&mut self, destination: String, title: String) {
        self.images.push(ImageState {
            destination,
            title,
            alt: String::new(),
        });
    }

    fn end_image(&mut self) {
        let Some(image) = self.images.pop() else {
            return;
        };
        let image = image_ref(image.destination, image.title);
        self.content
            .push_atom(md_document::AtomKind::Image(image), self.marks.clone());
    }

    fn text(&mut self, text: &str) {
        if let Some(image) = self.images.last_mut() {
            image.alt.push_str(text);
        } else {
            self.content.push_text(text, self.marks.clone());
        }
    }

    fn code(&mut self, code: &str) {
        let mut marks = self.marks.clone();
        marks.flags.insert(MarkFlags::CODE);
        self.content.push_text(code, marks);
    }

    fn atom(&mut self, kind: md_document::AtomKind) {
        self.content.push_atom(kind, self.marks.clone());
    }

    fn finish(mut self) -> InlineContent {
        self.content.normalize();
        self.content
    }
}

fn link_ref(destination: String, title: String) -> LinkRef {
    let link = LinkRef::new(destination);
    if title.is_empty() {
        link
    } else {
        link.with_title(title)
    }
}

fn image_ref(destination: String, title: String) -> ImageRef {
    let image = ImageRef::new(destination);
    if title.is_empty() {
        image
    } else {
        image.with_title(title)
    }
}

fn heading_level(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

fn map_align(align: Alignment) -> Align {
    match align {
        Alignment::None => Align::None,
        Alignment::Left => Align::Left,
        Alignment::Center => Align::Center,
        Alignment::Right => Align::Right,
    }
}

fn code_block_info(kind: CodeBlockKind<'_>) -> (Option<String>, FenceStyle) {
    match kind {
        CodeBlockKind::Fenced(info) => {
            let language = info
                .split_whitespace()
                .next()
                .filter(|language| !language.is_empty())
                .map(str::to_owned);
            (language, FenceStyle::BACKTICK)
        }
        CodeBlockKind::Indented => (None, FenceStyle::BACKTICK),
    }
}

#[cfg(test)]
mod tests {
    use super::parse;

    fn assert_roundtrip(source: &str) {
        let document = parse(source).unwrap();
        assert!(
            document.validate().is_ok(),
            "document did not validate: {source:?}"
        );
        assert_eq!(document.serialize(), source);
    }

    #[test]
    fn parses_headings_paragraphs_and_quotes() {
        assert_roundtrip("# Title\n\nHello **world**.\n\n> quote\n");
    }

    #[test]
    fn parses_lists_and_task_items() {
        assert_roundtrip("- one\n- two\n\n1. first\n2. second\n\n- [x] done\n");
    }

    #[test]
    fn parses_code_tables_and_rules() {
        assert_roundtrip("```rust\nfn main() {}\n```\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n---\n");
    }

    #[test]
    fn parses_links_images_and_inline_atoms() {
        assert_roundtrip(
            "A [link](https://example.com) and ![alt](cover.png).\n\nline  \nbreak and $x+1$.\n",
        );
    }

    #[test]
    fn parses_frontmatter_and_html_blocks() {
        assert_roundtrip("---\ntitle: Test\n---\n\n<div>\nraw\n</div>\n\nBody.\n");
    }
}

//! Canonical Markdown serializer for dirty documents.

use md_document::{Align, AtomKind, BlockId, BlockKind, Document, MarkFlags, Run};

/// Serializes a dirty document to canonical Markdown.
pub(crate) fn serialize(document: &Document, _source: &str) -> String {
    let mut output = String::new();
    for id in document.root_children() {
        write_block(document, *id, &mut output);
    }
    output
}

fn write_block(document: &Document, id: BlockId, output: &mut String) {
    let Some(node) = document.node(id) else {
        return;
    };

    match node.kind() {
        BlockKind::Paragraph(content) => {
            write_inline(content, output);
            output.push_str("\n\n");
        }
        BlockKind::Heading { level, content } => {
            output.push_str(&"#".repeat(usize::from(*level)));
            output.push(' ');
            write_inline(content, output);
            output.push_str("\n\n");
        }
        BlockKind::TableCell { content } => {
            write_inline(content, output);
        }
        BlockKind::CodeBlock { lang, text, fence } => {
            let marker = if fence.marker() == '~' { '~' } else { '`' };
            let fence_len = fence.length().max(3);
            let fence_text = marker.to_string().repeat(usize::from(fence_len));
            output.push_str(&fence_text);
            if let Some(lang) = lang {
                output.push_str(lang);
            }
            output.push('\n');
            output.push_str(text.as_str());
            if !text.as_str().ends_with('\n') {
                output.push('\n');
            }
            output.push_str(&fence_text);
            output.push_str("\n\n");
        }
        BlockKind::HtmlBlock(text) => {
            output.push_str(text.as_str());
            if !text.as_str().ends_with('\n') {
                output.push('\n');
            }
            output.push('\n');
        }
        BlockKind::MathBlock(text) => {
            let trimmed = text.as_str().trim_matches('\n');
            output.push_str("$$\n");
            output.push_str(trimmed);
            if !trimmed.is_empty() {
                output.push('\n');
            }
            output.push_str("$$\n\n");
        }
        BlockKind::FrontMatter(text) => {
            output.push_str("---\n");
            output.push_str(text.as_str());
            if !text.as_str().ends_with('\n') {
                output.push('\n');
            }
            output.push_str("---\n\n");
        }
        BlockKind::FootnoteDefinition { label, children } => {
            let mut inner = String::new();
            for child in children {
                write_block(document, *child, &mut inner);
            }
            let trimmed = inner.trim_end_matches('\n');
            output.push_str("[^");
            output.push_str(label);
            output.push_str("]: ");
            for (index, line) in trimmed.lines().enumerate() {
                if index > 0 {
                    output.push_str("    ");
                }
                output.push_str(line);
                output.push('\n');
            }
            if trimmed.is_empty() {
                output.push('\n');
            }
            output.push('\n');
        }
        BlockKind::Quote(children) => {
            let mut inner = String::new();
            for child in children {
                write_block(document, *child, &mut inner);
            }
            for line in inner.lines() {
                output.push_str("> ");
                output.push_str(line);
                output.push('\n');
            }
            output.push('\n');
        }
        BlockKind::List {
            ordered,
            start,
            items,
            ..
        } => {
            for (index, item) in items.iter().enumerate() {
                let marker = if *ordered {
                    format!("{}. ", start + index as u32)
                } else {
                    "- ".to_owned()
                };
                let mut inner = String::new();
                if let Some(item_node) = document.node(*item)
                    && let BlockKind::ListItem { task, children } = item_node.kind()
                {
                    let mut first = true;
                    for child in children {
                        if first {
                            if let Some(checked) = task {
                                inner.push_str(if *checked { "[x] " } else { "[ ] " });
                            }
                            first = false;
                        }
                        write_block(document, *child, &mut inner);
                    }
                }
                write_indented_item(&marker, &inner, output);
            }
            output.push('\n');
        }
        BlockKind::ListItem { task, children } => {
            if let Some(checked) = task {
                output.push_str(if *checked { "[x] " } else { "[ ] " });
            }
            for child in children {
                write_block(document, *child, output);
            }
        }
        BlockKind::Table { aligns, rows } => {
            write_table(document, aligns, rows, output);
        }
        BlockKind::TableRow { cells } => {
            output.push('|');
            for cell in cells {
                output.push(' ');
                write_block(document, *cell, output);
                output.push_str(" |");
            }
            output.push('\n');
        }
        BlockKind::ThematicBreak => output.push_str("---\n\n"),
        BlockKind::ImageBlock { image, alt } => {
            output.push_str("![");
            output.push_str(alt);
            output.push_str("](");
            output.push_str(image.destination());
            if let Some(title) = image.title() {
                output.push_str(" \"");
                output.push_str(title);
                output.push('"');
            }
            output.push_str(")\n\n");
        }
    }
}

fn write_indented_item(marker: &str, content: &str, output: &mut String) {
    let trimmed = content.trim_end_matches('\n');
    let padding = " ".repeat(marker.len());
    for (index, line) in trimmed.lines().enumerate() {
        if index == 0 {
            output.push_str(marker);
            output.push_str(line);
        } else {
            output.push_str(&padding);
            output.push_str(line);
        }
        output.push('\n');
    }
    if trimmed.is_empty() {
        output.push_str(marker);
        output.push('\n');
    }
}

fn write_table(document: &Document, aligns: &[Align], rows: &[BlockId], output: &mut String) {
    let Some(first_row) = rows.first() else {
        output.push('\n');
        return;
    };

    write_row(document, *first_row, output);
    output.push('|');
    for align in aligns {
        output.push_str(match align {
            Align::None => " --- |",
            Align::Left => " :--- |",
            Align::Center => " :---: |",
            Align::Right => " ---: |",
        });
    }
    output.push('\n');

    for row in rows.iter().skip(1) {
        write_row(document, *row, output);
    }
    output.push('\n');
}

fn write_row(document: &Document, row: BlockId, output: &mut String) {
    let Some(node) = document.node(row) else {
        return;
    };
    let BlockKind::TableRow { cells } = node.kind() else {
        return;
    };
    output.push('|');
    for cell in cells {
        output.push(' ');
        write_block(document, *cell, output);
        output.push_str(" |");
    }
    output.push('\n');
}

fn write_inline(content: &md_document::InlineContent, output: &mut String) {
    for run in content.runs() {
        match run {
            Run::Text { text, marks } => {
                write_text_run(text.as_str(), marks, output);
            }
            Run::Atom { kind, marks } => {
                write_atom(kind, marks, output);
            }
        }
    }
}

fn write_text_run(text: &str, marks: &md_document::Marks, output: &mut String) {
    if marks.flags.contains(MarkFlags::CODE) {
        let fence = "`".repeat(longest_backtick_run(text) + 1);
        output.push_str(&fence);
        output.push_str(text);
        output.push_str(&fence);
        return;
    }

    let link_open = marks.link.is_some();
    if link_open {
        output.push('[');
    }

    if marks.flags.contains(MarkFlags::STRONG) {
        output.push_str("**");
    }
    if marks.flags.contains(MarkFlags::EMPHASIS) {
        output.push('*');
    }
    if marks.flags.contains(MarkFlags::STRIKE) {
        output.push_str("~~");
    }
    if marks.flags.contains(MarkFlags::HIGHLIGHT) {
        output.push_str("==");
    }
    if marks.flags.contains(MarkFlags::UNDERLINE) {
        output.push_str("<u>");
    }

    output.push_str(&escape_text(text));

    if marks.flags.contains(MarkFlags::UNDERLINE) {
        output.push_str("</u>");
    }
    if marks.flags.contains(MarkFlags::HIGHLIGHT) {
        output.push_str("==");
    }
    if marks.flags.contains(MarkFlags::STRIKE) {
        output.push_str("~~");
    }
    if marks.flags.contains(MarkFlags::EMPHASIS) {
        output.push('*');
    }
    if marks.flags.contains(MarkFlags::STRONG) {
        output.push_str("**");
    }

    if let Some(link) = &marks.link {
        output.push_str("](");
        output.push_str(link.destination());
        if let Some(title) = link.title() {
            output.push_str(" \"");
            output.push_str(title);
            output.push('"');
        }
        output.push(')');
    }
}

fn write_atom(kind: &AtomKind, marks: &md_document::Marks, output: &mut String) {
    match kind {
        AtomKind::HardBreak => output.push_str("  \n"),
        AtomKind::SoftBreak => output.push('\n'),
        AtomKind::Image(image) => {
            output.push_str("![");
            output.push_str("](");
            output.push_str(image.destination());
            if let Some(title) = image.title() {
                output.push_str(" \"");
                output.push_str(title);
                output.push('"');
            }
            output.push(')');
        }
        AtomKind::InlineMath(text) => {
            output.push('$');
            output.push_str(text.as_str());
            output.push('$');
        }
        AtomKind::FootnoteRef(label) => {
            output.push_str("[^");
            output.push_str(label);
            output.push(']');
        }
    }
    let _ = marks;
}

fn escape_text(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        if matches!(
            character,
            '\\' | '*' | '_' | '[' | ']' | '<' | '>' | '`' | '#' | '|'
        ) {
            escaped.push('\\');
        }
        escaped.push(character);
    }
    escaped
}

fn longest_backtick_run(text: &str) -> usize {
    let mut longest = 0;
    let mut current = 0;
    for character in text.chars() {
        if character == '`' {
            current += 1;
            longest = longest.max(current);
        } else {
            current = 0;
        }
    }
    longest
}

#[cfg(test)]
mod tests {
    use crate::parse;

    #[test]
    fn dirty_serialization_writes_marks_and_links() {
        let mut document =
            parse("**bold** and *italic* and [link](https://example.com)\n").unwrap();
        document.document_mut();
        let output = document.serialize();
        assert!(output.contains("**bold**"));
        assert!(output.contains("*italic*"));
        assert!(output.contains("[link](https://example.com)"));
    }

    #[test]
    fn dirty_serialization_writes_lists_and_tables() {
        let mut document = parse("- one\n- two\n\n| a | b |\n|---|---|\n| 1 | 2 |\n").unwrap();
        document.document_mut();
        let output = document.serialize();
        assert!(output.contains("- one"));
        assert!(output.contains("| a | b |"));
        assert!(output.contains("| --- | --- |"));
    }

    #[test]
    fn dirty_serialization_writes_code_and_quotes() {
        let mut document = parse("```rust\nfn main() {}\n```\n\n> quoted\n").unwrap();
        document.document_mut();
        let output = document.serialize();
        assert!(output.contains("```rust"));
        assert!(output.contains("> quoted"));
    }
}

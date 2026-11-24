//! Scale regression gates for the headless layout engine.

use std::time::{Duration, Instant};

use md_core::Size;
use md_document::{BlockKind, Document, InlineContent};
use md_layout::{LayoutEnv, LayoutTree, ResolvedStyle};

fn env(width: f32, height: f32) -> LayoutEnv {
    LayoutEnv::new(Size::new(width, height), ResolvedStyle::default())
}

#[test]
#[ignore = "scale gate: run with cargo test --release -- --ignored"]
fn hundred_thousand_character_paragraph_lays_out() {
    let text = "日常书写与札记。abc ".repeat(5_000);
    let mut document = Document::new();
    document
        .set_inline_content(document.root_children()[0], InlineContent::from_text(text))
        .unwrap();
    let started = Instant::now();
    let tree = LayoutTree::full(&document, env(800.0, 600.0));
    let elapsed = started.elapsed();
    assert!(tree.total_height() > 0.0);
    assert!(
        elapsed < Duration::from_secs(5),
        "100k-char layout took {elapsed:?}"
    );
}

#[test]
#[ignore = "scale gate: run with cargo test --release -- --ignored"]
fn ten_thousand_paragraphs_lay_out() {
    let mut document = Document::new();
    document
        .set_inline_content(
            document.root_children()[0],
            InlineContent::from_text("line 0"),
        )
        .unwrap();
    for index in 1..10_000 {
        document
            .append_block(
                None,
                BlockKind::Paragraph(InlineContent::from_text(format!("line {index}"))),
            )
            .unwrap();
    }
    let started = Instant::now();
    let tree = LayoutTree::full(&document, env(800.0, 600.0));
    let elapsed = started.elapsed();
    assert_eq!(tree.blocks.len(), 10_000);
    assert!(
        elapsed < Duration::from_secs(10),
        "10k-block layout took {elapsed:?}"
    );
}

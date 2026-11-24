//! Random Document operation sequences keep every invariant (AC-04).

use md_document::{BlockKind, Document, InlineContent};

struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0
    }

    fn pick(&mut self, max: usize) -> usize {
        if max == 0 {
            0
        } else {
            (self.next() as usize) % max
        }
    }
}

fn paragraph(text: &str) -> BlockKind {
    BlockKind::Paragraph(InlineContent::from_text(text))
}

#[test]
fn random_document_operations_keep_invariants() {
    let mut rng = Lcg(0x0D0C_2026);
    for _ in 0..1000 {
        let mut document = Document::new();
        for _ in 0..32 {
            match rng.next() % 6 {
                0 => {
                    let index = rng.pick(document.root_children().len() + 1);
                    let _ = document.insert_block(None, index, paragraph("p"));
                }
                1 => {
                    let roots = document.root_children().to_vec();
                    if let Some(id) = roots.get(rng.pick(roots.len())).copied() {
                        let _ = document.remove_block(id);
                    }
                }
                2 => {
                    let roots = document.root_children().to_vec();
                    if roots.len() > 1 {
                        let id = roots[rng.pick(roots.len())];
                        let target = rng.pick(roots.len() + 1);
                        let _ = document.move_block(id, None, target);
                    }
                }
                3 => {
                    let roots = document.root_children().to_vec();
                    if let Some(id) = roots.get(rng.pick(roots.len())).copied() {
                        let _ =
                            document.set_inline_content(id, InlineContent::from_text("changed"));
                    }
                }
                4 => {
                    let roots = document.root_children().to_vec();
                    if let Some(id) = roots.get(rng.pick(roots.len())).copied() {
                        let len = document.kind(id).and_then(BlockKind::leaf_len).unwrap_or(0);
                        let _ = document.split_block(id, rng.pick(len + 1));
                    }
                }
                _ => {
                    let roots = document.root_children().to_vec();
                    if roots.len() > 1 {
                        let index = rng.pick(roots.len() - 1);
                        let _ = document.join_blocks(roots[index], roots[index + 1]);
                    }
                }
            }
            assert!(
                document.validate().is_ok(),
                "invariant failed after operation: {:?}",
                document.validate()
            );
        }
    }
}

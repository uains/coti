//! Markdown serializer placeholder; full dirty serialization is added next.

use md_document::Document;

pub(crate) fn serialize(_document: &Document, source: &str) -> String {
    source.to_owned()
}

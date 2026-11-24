//! AC-01 fixture corpus: pristine byte equality and canonical reparse.

use std::fs;
use std::path::{Path, PathBuf};

use md_markdown::parse;

fn fixture_paths() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/markdown");
    let mut paths: Vec<_> = fs::read_dir(&root)
        .expect("fixture directory should exist")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "md"))
        .collect();
    paths.sort();
    paths
}

#[test]
fn fixture_corpus_has_at_least_200_files() {
    let paths = fixture_paths();
    assert!(
        paths.len() >= 200,
        "expected at least 200 Markdown fixtures, found {}",
        paths.len()
    );
}

#[test]
fn every_fixture_roundtrips_byte_for_byte() {
    for path in fixture_paths() {
        let source = fs::read_to_string(&path).expect("fixture should be valid UTF-8");
        let document = parse(&source)
            .unwrap_or_else(|error| panic!("{}: parse failed: {error}", path.display()));

        document
            .validate()
            .unwrap_or_else(|error| panic!("{}: validate failed: {error}", path.display()));
        assert_eq!(
            document.serialize(),
            source,
            "{}: pristine serialization changed bytes",
            path.display()
        );
    }
}

#[test]
fn every_fixture_canonical_output_reparses() {
    for path in fixture_paths() {
        let source = fs::read_to_string(&path).expect("fixture should be valid UTF-8");
        let mut document = parse(&source)
            .unwrap_or_else(|error| panic!("{}: parse failed: {error}", path.display()));
        document.document_mut();
        let canonical = document.serialize();

        let reparsed = parse(&canonical).unwrap_or_else(|error| {
            panic!(
                "{}: canonical output failed to parse: {error}\n---\n{canonical}",
                path.display()
            )
        });
        reparsed.validate().unwrap_or_else(|error| {
            panic!(
                "{}: canonical output failed validation: {error}\n---\n{canonical}",
                path.display()
            )
        });
    }
}

#[test]
fn every_fixture_canonical_output_is_stable() {
    for path in fixture_paths() {
        let source = fs::read_to_string(&path).expect("fixture should be valid UTF-8");
        let mut document = parse(&source)
            .unwrap_or_else(|error| panic!("{}: parse failed: {error}", path.display()));
        document.document_mut();
        let first = document.serialize();

        let mut reparsed = parse(&first)
            .unwrap_or_else(|error| panic!("{}: reparse failed: {error}", path.display()));
        reparsed.document_mut();
        let second = reparsed.serialize();

        assert_eq!(
            second,
            first,
            "{}: canonical serialization is not stable",
            path.display()
        );
    }
}

#[test]
fn canonical_document_is_isomorphic_after_reparse() {
    for path in fixture_paths() {
        let source = fs::read_to_string(&path).expect("fixture should be valid UTF-8");
        let mut document = parse(&source)
            .unwrap_or_else(|error| panic!("{}: parse failed: {error}", path.display()));
        document.document_mut();
        let canonical = document.serialize();
        let reparsed = parse(&canonical)
            .unwrap_or_else(|error| panic!("{}: reparse failed: {error}", path.display()));
        assert!(
            document.document().content_eq(reparsed.document()),
            "{}: canonical model differs after reparse",
            path.display()
        );
    }
}

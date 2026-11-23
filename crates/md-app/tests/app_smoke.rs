//! Cross-crate smoke tests for the assembled application.

use std::error::Error;

#[test]
fn error_source_chain_survives_layer_boundaries() {
    let error = md_render::RenderError::from(md_layout::LayoutError::from(
        md_document::DocumentError::from(md_core::TextError::NotCharBoundary { offset: 7 }),
    ));

    assert!(error.to_string().contains("not a UTF-8 character boundary"));

    let mut depth = 0;
    let mut source = error.source();
    while let Some(cause) = source {
        depth += 1;
        source = cause.source();
    }
    assert!(depth >= 3, "expected a layered source chain, got {depth}");
}

#[test]
fn application_binary_starts_and_prints_banner() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_coti"))
        .output()
        .expect("coti binary should start");

    assert!(
        output.status.success(),
        "coti exited with {}",
        output.status
    );

    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(stdout.contains("Coti"), "unexpected stdout: {stdout}");
}

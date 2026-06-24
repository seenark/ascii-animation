use std::fs;

fn release_workflow() -> String {
    fs::read_to_string(".github/workflows/release.yml").expect("release workflow should exist")
}

#[test]
fn release_workflow_targets_only_supported_platforms() {
    let workflow = release_workflow();

    assert!(workflow.contains("tags:\n      - \"v*.*.*\""));
    assert!(workflow.contains("target: x86_64-unknown-linux-gnu"));
    assert!(workflow.contains("target: aarch64-unknown-linux-gnu"));
    assert!(workflow.contains("target: aarch64-apple-darwin"));
    assert!(!workflow.contains("target: x86_64-apple-darwin"));
    assert!(!workflow.contains("target: x86_64-pc-windows-msvc"));
}

#[test]
fn release_workflow_omits_npm_and_uses_github_release_assets() {
    let workflow = release_workflow();

    assert!(workflow.contains("name: Verify tag matches crate version"));
    assert!(workflow.contains("cargo metadata --no-deps --format-version 1"));
    assert!(workflow.contains("cargo build --locked --release --target"));
    assert!(workflow.contains("softprops/action-gh-release@v2"));
    assert!(workflow.contains("SHA256SUMS"));
    assert!(workflow.contains("release-assets/ascii-animation"));
    assert!(!workflow.contains("npm publish"));
    assert!(!workflow.contains("NPM_TOKEN"));
}

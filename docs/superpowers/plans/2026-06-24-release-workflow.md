# Release Workflow Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a tag-triggered GitHub Actions workflow that verifies the crate version, builds release artifacts for Linux x86_64, Linux aarch64, and macOS aarch64, and publishes them to GitHub Releases with checksums.

**Architecture:** Keep release automation in one workflow at `.github/workflows/release.yml` with three jobs: `verify-version`, `build`, and `release`. Add a Rust integration test that asserts the workflow file contains the approved trigger, target matrix, packaging contract, and exclusions so the release configuration stays under test.

**Tech Stack:** GitHub Actions YAML, Rust integration tests, Cargo test

## Global Constraints

- Trigger only on pushed tags matching `v*.*.*`.
- Verify the tag version matches `Cargo.toml` package version.
- Build exactly `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`, and `aarch64-apple-darwin`.
- Do not build Windows or Intel macOS artifacts.
- Package Unix binaries as `.tar.gz` archives.
- Publish both raw binaries and archives as GitHub Release assets.
- Generate and publish `SHA256SUMS`.
- Do not publish to npm.
- The binary name is `ascii-animation`.
- Use `cargo build --locked --release --target <target>`.
- Use stable Rust with `rustup toolchain install stable --profile minimal --target <target>`.

---

### Task 1: Add release workflow regression tests

**Files:**
- Create: `tests/release_workflow.rs`
- Test: `tests/release_workflow.rs`

**Interfaces:**
- Consumes: `.github/workflows/release.yml` as plain text loaded by `std::fs::read_to_string`
- Produces: integration tests `release_workflow_targets_only_supported_platforms()` and `release_workflow_omits_npm_and_uses_github_release_assets()`

- [ ] **Step 1: Write the failing test**

```rust
use std::fs;

fn release_workflow() -> String {
    fs::read_to_string(".github/workflows/release.yml")
        .expect("release workflow should exist")
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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test release_workflow`
Expected: FAIL with `.github/workflows/release.yml` missing

- [ ] **Step 3: Commit**

```bash
git add tests/release_workflow.rs
git commit -m "test: add release workflow coverage"
```

### Task 2: Implement the GitHub release workflow

**Files:**
- Create: `.github/workflows/release.yml`
- Modify: `tests/release_workflow.rs`
- Test: `tests/release_workflow.rs`

**Interfaces:**
- Consumes: crate version from `Cargo.toml`, binary path `target/<target>/release/ascii-animation`, release asset contract from the approved spec
- Produces: workflow jobs `verify-version`, `build`, and `release` in `.github/workflows/release.yml`

- [ ] **Step 1: Write the minimal workflow implementation**

```yaml
name: Release

on:
  push:
    tags:
      - "v*.*.*"

permissions:
  contents: write

jobs:
  verify-version:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Verify tag matches crate version
        shell: bash
        run: |
          set -euo pipefail
          VERSION="${GITHUB_REF_NAME#v}"
          CARGO_VERSION="$(cargo metadata --no-deps --format-version 1 | python3 -c 'import json, sys; print(json.load(sys.stdin)["packages"][0]["version"])')"
          if [ "$CARGO_VERSION" != "$VERSION" ]; then
            echo "::error::Cargo package version ${CARGO_VERSION} does not match release tag ${VERSION}"
            exit 1
          fi

  build:
    needs: verify-version
    runs-on: ${{ matrix.os }}
    strategy:
      fail-fast: false
      matrix:
        include:
          - os: ubuntu-latest
            target: x86_64-unknown-linux-gnu
            raw_name: ascii-animation-${{ github.ref_name }}-x86_64-unknown-linux-gnu
            archive_name: ascii-animation-${{ github.ref_name }}-x86_64-unknown-linux-gnu.tar.gz
          - os: ubuntu-24.04-arm
            target: aarch64-unknown-linux-gnu
            raw_name: ascii-animation-${{ github.ref_name }}-aarch64-unknown-linux-gnu
            archive_name: ascii-animation-${{ github.ref_name }}-aarch64-unknown-linux-gnu.tar.gz
          - os: macos-14
            target: aarch64-apple-darwin
            raw_name: ascii-animation-${{ github.ref_name }}-aarch64-apple-darwin
            archive_name: ascii-animation-${{ github.ref_name }}-aarch64-apple-darwin.tar.gz
    steps:
      - uses: actions/checkout@v4
      - name: Install Rust stable target
        shell: bash
        run: |
          rustup toolchain install stable --profile minimal --target "${{ matrix.target }}"
          rustup default stable
      - name: Build release binary
        shell: bash
        run: cargo build --locked --release --target "${{ matrix.target }}"
      - name: Prepare unix assets
        shell: bash
        run: |
          set -euo pipefail
          mkdir -p release-assets
          cp "target/${{ matrix.target }}/release/ascii-animation" "release-assets/${{ matrix.raw_name }}"
          chmod 755 "release-assets/${{ matrix.raw_name }}"
          cp "release-assets/${{ matrix.raw_name }}" "release-assets/ascii-animation"
          tar -czf "release-assets/${{ matrix.archive_name }}" -C release-assets ascii-animation
          rm "release-assets/ascii-animation"
      - uses: actions/upload-artifact@v4
        with:
          name: ${{ matrix.target }}
          path: |
            release-assets/${{ matrix.raw_name }}
            release-assets/${{ matrix.archive_name }}

  release:
    needs: build
    runs-on: ubuntu-latest
    steps:
      - uses: actions/download-artifact@v4
        with:
          path: dist
      - name: Flatten release assets
        shell: bash
        run: |
          set -euo pipefail
          mkdir -p dist/release-assets
          find dist -mindepth 2 -type f -exec cp {} dist/release-assets/ \;
      - name: Generate SHA256SUMS
        shell: bash
        run: |
          set -euo pipefail
          cd dist/release-assets
          find . -maxdepth 1 -type f ! -name SHA256SUMS -print0 | sort -z | xargs -0 shasum -a 256 | sed 's#  \./#  #' > SHA256SUMS
      - uses: softprops/action-gh-release@v2
        with:
          files: dist/release-assets/*
```

- [ ] **Step 2: Run the focused test to verify it passes**

Run: `cargo test --test release_workflow`
Expected: PASS

- [ ] **Step 3: Run the adjacent CLI/config regression tests**

Run: `cargo test --test cli --test scene_config`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add .github/workflows/release.yml tests/release_workflow.rs
git commit -m "ci: add release workflow"
```

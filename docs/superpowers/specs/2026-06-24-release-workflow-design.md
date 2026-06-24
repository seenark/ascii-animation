# ASCII Animation Release Workflow Design

Date: 2026-06-24

## Goal

Add a GitHub Actions release workflow that publishes GitHub Release artifacts for the Rust `ascii-animation` binary when a semantic version tag is pushed.

The workflow must support Linux x86_64, Linux aarch64, and macOS aarch64. It must not build Windows or Intel macOS artifacts. Distribution is GitHub Releases only; npm publishing is explicitly out of scope because install flow is via mise rather than npm.

## Scope

This design covers a single release automation workflow under `.github/workflows/`.

It does not change runtime code, crate packaging metadata beyond version verification, or any install script. It only defines how release assets are verified, built, packaged, and attached to GitHub Releases.

## Requirements

- Trigger on pushed tags matching `v*.*.*`.
- Verify that the tag version matches `Cargo.toml` package version.
- Build exactly these Rust targets:
  - `x86_64-unknown-linux-gnu`
  - `aarch64-unknown-linux-gnu`
  - `aarch64-apple-darwin`
- Do not build:
  - `x86_64-apple-darwin`
  - any Windows target
- Package Unix binaries as `.tar.gz` archives.
- Publish both raw binaries and archives as GitHub Release assets.
- Generate and publish a `SHA256SUMS` file covering all assets.
- Do not publish to npm.

## Constraints

- The binary name is `ascii-animation`, matching the crate name.
- Use `cargo build --locked --release --target <target>`.
- Use stable Rust with `rustup toolchain install stable --profile minimal --target <target>`.
- Use GitHub-hosted runners only.
- Keep workflow design boring and single-purpose: one workflow with sequential verify, build, and release jobs.
- Artifact names must include the pushed tag so released files are self-describing.

## Recommended workflow shape

### Job 1: `verify-version`

Run on `ubuntu-latest`.

Responsibilities:

- Check out the repository.
- Strip the leading `v` from `github.ref_name`.
- Read the crate version from `Cargo.toml` via `cargo metadata --no-deps --format-version 1`.
- Fail the workflow if the crate version does not equal the tag version.

Rationale:

This prevents publishing mislabeled binaries and keeps release tags aligned with crate metadata.

### Job 2: `build`

Run as a matrix job after `verify-version`.

Matrix entries:

- `ubuntu-latest` / `x86_64-unknown-linux-gnu`
- `ubuntu-24.04-arm` / `aarch64-unknown-linux-gnu`
- `macos-14` / `aarch64-apple-darwin`

Responsibilities per matrix entry:

- Check out the repository.
- Install stable Rust for the matrix target.
- Build the release binary with Cargo.
- Copy the produced binary into a `release-assets/` directory.
- Rename the copied binary to a target-specific release filename.
- Create a second temporary copy named exactly `ascii-animation`.
- Package that temporary copy into a `.tar.gz` file so install tools can extract a stable binary name from every archive.
- Upload both the raw renamed binary and the archive as workflow artifacts.

Rationale:

This keeps release assets target-specific while ensuring archive contents expose the canonical executable name expected by install tools such as mise backends.

## Asset naming contract

For a tag like `v0.1.0`, publish these file patterns:

- `ascii-animation-v0.1.0-x86_64-unknown-linux-gnu`
- `ascii-animation-v0.1.0-x86_64-unknown-linux-gnu.tar.gz`
- `ascii-animation-v0.1.0-aarch64-unknown-linux-gnu`
- `ascii-animation-v0.1.0-aarch64-unknown-linux-gnu.tar.gz`
- `ascii-animation-v0.1.0-aarch64-apple-darwin`
- `ascii-animation-v0.1.0-aarch64-apple-darwin.tar.gz`
- `SHA256SUMS`

This contract is intentionally simple and GitHub-Release-friendly. A mise integration can choose the archive matching the current platform and unpack a binary named `ascii-animation`.

## Job 3: `release`

Run on `ubuntu-latest` after all `build` jobs succeed.

Responsibilities:

- Download all uploaded artifacts.
- Flatten artifacts into one release staging directory.
- Generate `SHA256SUMS` from all staged files except `SHA256SUMS` itself.
- Create or update the GitHub Release for the pushed tag.
- Upload all staged files as release assets.

Rationale:

A dedicated release job isolates the final publish step from the per-platform build jobs and guarantees a single checksum file over the final asset set.

## Permissions

Workflow permissions:

- `contents: write` for creating the GitHub Release and uploading assets.

No npm token, package registry permissions, or OIDC publish steps are needed.

## Out of scope

- npm publish
- crates.io publish
- Windows ZIP packaging
- Intel macOS packaging
- Draft release approval flow
- Signing, notarization, or provenance attestation
- Auto-updating any mise plugin repository or manifest file

## Verification strategy

Implementation verification should cover:

1. YAML parses and sits at `.github/workflows/release.yml`.
2. Target matrix contains only the three approved targets.
3. Version verification reads crate version and rejects tag mismatch.
4. Release packaging uses the `ascii-animation` binary name inside archives.
5. Final release job generates a `SHA256SUMS` file and uploads all assets.

## Decisions

- Use GitHub Releases only as the distribution output.
- Keep release automation in one workflow rather than splitting build and publish across multiple workflows.
- Publish both raw binaries and tarballs.
- Exclude Windows and Intel macOS entirely.
- Omit npm publishing because the install path is mise, not npm.

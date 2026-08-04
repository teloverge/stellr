# Accept README Release Preview Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Promote one explicitly reviewed live preview into immutable versioned README assets, with the README reference changed only after the complete artifact set is verified.

**Architecture:** Add an acceptance module beside preview generation. It loads the exact four-file review directory, verifies a domain-separated SHA-256 digest and the canonical story rendering before any tracked write, publishes three immutable versioned assets through verified sibling temporaries, then atomically replaces only the delimited README showcase section. The command returns explicit unreferenced paths if asset publication completed but README replacement did not.

**Tech Stack:** Rust 2024, `sha2`, `resvg`, `serde_json`, `clap`, native Windows `ReplaceFileW`, Cargo tests.

## Global Constraints

- Native Windows 11 and native `cargo.exe`, `git.exe`, and PowerShell only.
- Accepted paths are `docs/assets/readme-showcase/<version>.svg`, `<version>.png`, and `<version>-story.json`.
- The expected reviewed-preview digest is required and must match before any tracked path is created or changed.
- SVG remains at most 750 KiB, PNG at most 1.5 MiB, and manifest at most 1 MiB.
- Existing versioned artifacts are immutable: exact bytes are idempotent; different bytes fail closed.
- The README uses animated SVG by default, PNG for reduced motion and strict Markdown, concise alt text, and an adjacent issue/resolution summary.
- The README is the last publication point; a failure leaves its prior showcase bytes intact and reports every complete unreferenced asset.

---

### Task 1: Reviewed Preview Identity and Validation

**Files:**
- Create: `crates/showcase/tests/accept_preview.rs`
- Create: `crates/showcase/src/acceptance.rs`
- Modify: `crates/showcase/src/preview_operation.rs`
- Modify: `crates/showcase/src/lib.rs`
- Modify: `crates/showcase/Cargo.toml`

**Interfaces:**
- Produces: `preview_digest(&StaticPreview) -> String` using `sha256:<lowercase hex>`.
- Produces: `accept_release_preview(repository_root: &Path, preview_directory: &Path, expected_digest: &str) -> Result<AcceptanceReceipt, AcceptanceError>`.
- Consumes: the exact `release.svg`, `release.png`, `story.json`, and `review.html` preview set and the canonical `validate_outputs` contract.

- [x] **Step 1: Write the failing digest and invalid-acceptance tests**

  Create real temporary repositories and rendered preview directories. Assert the hand-recorded digest format is stable, a wrong digest leaves `README.md` and `docs/assets` untouched, unexpected files fail closed, and a valid preview reaches the not-yet-implemented publication behavior.

- [x] **Step 2: Run the focused test to verify RED**

  Run `cargo.exe test -p stellr-showcase --test accept_preview` and confirm compilation fails because the acceptance API is absent.

- [x] **Step 3: Implement minimal loading, digest, and canonical validation**

  Hash a domain separator plus each fixed artifact name, byte length, and bytes. Reject malformed digests, reparse points, missing/extra artifacts, a manifest/release-version mismatch, and anything that fails the trusted canonical rerender before creating tracked directories.

- [x] **Step 4: Run the focused test to verify GREEN**

  Run `cargo.exe test -p stellr-showcase --test accept_preview` and require all Task 1 cases to pass.

### Task 2: Immutable Assets and README Publication Point

**Files:**
- Modify: `crates/showcase/tests/accept_preview.rs`
- Modify: `crates/showcase/src/acceptance.rs`
- Modify: `README.md`

**Interfaces:**
- Produces: `AcceptanceReceipt { assets: [PathBuf; 3], readme: PathBuf, digest: String }`.
- Produces: `AcceptanceError::Publication` carrying the exact `unreferenced_assets: Vec<PathBuf>`.
- Consumes: validated preview bytes and release story from Task 1.

- [x] **Step 1: Write failing publication-order and immutability tests**

  Assert three versioned files receive exact preview bytes; the README contains a delimited `<picture>` block with SVG, reduced-motion PNG, ordinary PNG link, and literal visible/resolved counts; a conflicting existing asset is preserved; and an injected README replacement failure preserves the old README while reporting all three complete asset paths.

- [x] **Step 2: Run the focused tests to verify RED**

  Run `cargo.exe test -p stellr-showcase --test accept_preview` and confirm failures name missing assets/README behavior rather than fixture errors.

- [x] **Step 3: Implement verified immutable writes and atomic README replacement**

  Write each missing asset with `create_new`, `sync_all`, same-directory rename, and reread verification. Treat matching existing bytes as idempotent and reject conflicts. Build the README section from the story, write/sync a sibling temporary, replace the existing README with native `ReplaceFileW` and recovery backup on Windows, reread it, and attach already-complete asset paths to any post-publication error.

- [x] **Step 4: Run focused tests to verify GREEN**

  Run `cargo.exe test -p stellr-showcase --test accept_preview` and require all publication, interruption, and idempotence cases to pass.

### Task 3: Native CLI, Contract Tests, and Release Record

**Files:**
- Modify: `crates/showcase/src/main.rs`
- Modify: `crates/showcase/tests/readme_contract.rs`
- Modify: `CHANGELOG.md`
- Create: `docs/validation/release-preview-acceptance.md`

**Interfaces:**
- Produces: `cargo.exe run -p stellr-showcase -- accept --preview <path> --digest sha256:<hex>`.
- Consumes: `accept_release_preview` and `preview_digest`; preview output prints the review digest that acceptance requires.

- [x] **Step 1: Write failing CLI and repository-contract tests**

  Assert `accept` requires both preview and digest, preview output exposes the digest, and the accepted README contract is version-agnostic through its start/end markers and delivery paths.

- [x] **Step 2: Run focused tests to verify RED**

  Run `cargo.exe test -p stellr-showcase --all-targets` and confirm the new CLI/README expectations fail before implementation.

- [x] **Step 3: Implement CLI and documentation**

  Add the `Accept` subcommand, resolve relative preview paths under the native repository root, print receipt paths/digest or exact unreferenced paths on failure, update the append-only Unreleased changelog, and record focused native validation evidence.

- [x] **Step 4: Run final verification and reviews**

  Run `cargo.exe fmt --all -- --check`, `cargo.exe clippy --workspace --all-targets -- -D warnings`, `cargo.exe test --workspace --locked`, `git diff --check`, and two final code reviews. Commit only after all findings are resolved and gates rerun.

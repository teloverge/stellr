# Release preview acceptance validation

**Date:** 2026-08-02

**Issue:** #53 - Accept reviewed previews as versioned README assets

**Environment:** Native Windows 11, `cargo.exe`, native NTFS junctions and file
symlinks, and `ReplaceFileW` for the README publication point

## Command contract

Preview prints the domain-separated SHA-256 identity of the exact reviewed
four-file set:

```text
Preview ready: <repository>\target\readme-showcase\<version>
Review digest: sha256:<64 lowercase hexadecimal digits>
```

The maintainer supplies that exact value when accepting the review:

```powershell
cargo.exe run -p stellr-showcase -- accept `
  --preview target/readme-showcase/<version> `
  --digest sha256:<reviewed digest>
```

Acceptance verifies the digest, exact file set, manifest release identity, full
PNG decoding, SVG safety, and byte identity with a trusted canonical rerender
before creating or changing a tracked path.

## Publication contract

The command publishes and rereads these immutable files in order:

```text
docs/assets/readme-showcase/<version>.svg
docs/assets/readme-showcase/<version>.png
docs/assets/readme-showcase/<version>-story.json
```

Each missing asset is written and synced through a sibling temporary, then
moved without replacing an existing path. Existing exact bytes are idempotent;
different bytes fail closed. Only after all three files are complete does the
command atomically replace the delimited README showcase section. The section
contains an animated SVG, a reduced-motion PNG source, an ordinary Markdown PNG
link, concise alternative text, and an adjacent visible/resolved issue summary.
Windows-safe version names are percent-encoded in README URLs and separately
escaped for HTML and Markdown text contexts.

If README replacement fails, the previous README is restored or retained and
the error lists every complete visual asset not already referenced by that
retained README. The story manifest is treated as transitively published only
when both SVG and PNG delivery paths remain referenced. The recovery backup
remains available through final byte verification;
a replacement error or verification mismatch restores it over a missing or
potentially changed target. Output ancestry is rechecked after directory
creation and immediately before every tracked asset and README publication.

## Focused native evidence

```powershell
cargo.exe test -p stellr-showcase --test accept_preview
cargo.exe test -p stellr-showcase --test live_preview
cargo.exe test -p stellr-showcase --test readme_contract
cargo.exe test -p stellr-showcase --bin stellr-showcase
cargo.exe run -p stellr-showcase -- accept --help
```

The acceptance suite covers the stable digest vector, wrong digest, unexpected
files, semantically wrong artifacts with a matching digest, release-version
identity, byte-identical repetition, immutable conflicts, real read-only
`ReplaceFileW` failure, displaced-backup recovery, input and output junctions,
post-success verification mismatch recovery, complete inline-Markdown and URL
escaping for special-character versions, complete and partial prior-reference
classification, and an individual reviewed-artifact symlink. Failure cases
assert that the prior README remains byte-identical and that no write escapes
through a reparse point.

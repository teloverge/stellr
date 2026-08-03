# Live release preview validation

**Date:** 2026-08-02

**Issue:** #52 - Generate a fail-closed live release preview

**Environment:** Native Windows 11, `cargo.exe`, GitHub CLI authentication,
bundled Playwright with the installed Microsoft Edge channel

## Operation contract

The native `preview` command infers `owner/name` from the GitHub `origin` unless
`--repo owner/name` is supplied. The caller provides exactly one starting
boundary (`--from-release` or `--from-cutoff`) and an explicit UTC ending
cutoff. The milestone is also the release version unless `--version` overrides
it.

The operation completes these stages before publishing a visible directory:

1. acquire the complete live story through `ReleaseHistorySource`;
2. verify repository, release, and milestone identity;
3. render the story twice and compare all output bytes;
4. independently validate budgets, SVG safety, manifest agreement, full PNG
   decoding and dimensions, then compare SVG, PNG, manifest, and review HTML
   byte-for-byte with a trusted canonical rerender;
5. write and reread all four files in a unique sibling staging directory;
6. rename the complete staging directory to
   `target/readme-showcase/<version>`.

If the destination already exists, all four names and bytes must match exactly.
A differing review is preserved and the command fails rather than replacing it.
Release versions are validated as safe single Windows path components. Every
existing output ancestor, destination, staging directory, and artifact is
rejected when Windows marks it as a reparse point, including junctions and
symbolic links.

## Automated failure evidence

```powershell
cargo.exe test -p stellr-showcase --test live_preview
cargo.exe test -p stellr-showcase --all-targets
cargo.exe clippy -p stellr-showcase --all-targets -- -D warnings
```

The focused suite passed eight tests covering complete four-file publication,
byte-identical repetition, partial GitHub pagination, renderer/rasterization
failure, nondeterministic output, unsafe or semantically incorrect SVG, SVG
budget overflow, truncated or valid-but-incorrect PNG, active review HTML,
preservation of a differing existing preview, and native Windows target and
destination junction rejection. Every failure assertion also confirmed that no
destination preview was published or redirected outside the repository.

## Live GitHub preview

The current live milestone title was read from GitHub before running:

```powershell
cargo.exe run -p stellr-showcase -- preview `
  --milestone "M1 — the chart" `
  --version "issue-52-live-smoke" `
  --from-cutoff "2026-07-31T00:00:00Z" `
  --cutoff "2026-08-03T00:44:50Z"
```

The command completed against `teloverge/stellr` and published exactly four
ignored review files. The canonical manifest contains 17 visible issues, 23
directed edges, and eight evidence beats. A second invocation with the identical
window succeeded only after proving the existing artifacts were byte-identical.

| Artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| `release.svg` | 120,029 | `0D0DF36400DCC6980CF250738BF9E6F6CF75EABE5B52E86AF140D4CC242BCE63` |
| `release.png` | 131,145 | `E5163740EC63C1A2C6B8538F28141E5C3100DD21234300787816005AC111D922` |
| `story.json` | 20,645 | `3DE05574B816808BAC5FE3D153975D863796A09AEF9F5902F375D49B97963C00` |
| `review.html` | 154,172 | `B8CC08D651F100207FAD907EB17D601B8C7FA5FEA3418FB44D97A7CEB1A1175D` |

## Visual review

The integrated browser remained unavailable after its native sandbox startup
failure, so the approved fallback used Playwright with installed Edge. Normal
motion captures at five and ten seconds showed real M1 status/focus changes over
fixed geometry. A forced reduced-motion capture showed the complete final state
with 17 solid resolved stars, all 23 dependency arrows, release identity and
summary, bounded labels, and no CURRENT, READY, caption, or particle overlays.

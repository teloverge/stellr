# Markdown Relationship Lines Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restore visible dependency and parent lines when issue relationships are expressed in structured Markdown sections, including from an existing local cache.

**Architecture:** Keep relationship interpretation inside `stellr-github`. Extend the existing fenced-code-aware scanner with exact relationship-section state, then apply one deterministic enrichment function to both fresh provider results and deserialized cache snapshots. Leave the core derivation, workflow topology, layout, and canvas renderer unchanged.

**Tech Stack:** Rust 2024, Serde JSON, Octocrab-compatible GitHub GraphQL mapping, native Windows PowerShell, Cargo, Svelte 5, Vitest, Tauri 2, NSIS.

## Global Constraints

- Use native Windows 11 PowerShell and Windows toolchains only; do not use WSL or Linux tooling.
- Use issue bodies already present in the ordinary snapshot or local cache; add no GitHub request.
- Native GitHub relationships remain authoritative; Markdown is additive for dependencies and fallback-only for a missing parent.
- Recognize only exact case-insensitive ATX headings `Blocked by`, `Blocks`, and `Parent`, with an optional closing heading marker.
- Ignore fenced code, stop a relationship section at the next ATX heading, deduplicate references, and ignore an ambiguous Markdown parent.
- Do not change renderer colors, geometry, animation, layout, focus behavior, or fixed topology.
- Keep pending release notes in `Unreleased`; this focused fix does not create or rewrite a shipped-version section.

---

### Task 1: Parse dependency relationship sections

**Files:**
- Modify: `crates/github/src/textref.rs`
- Test: `crates/github/src/textref.rs`

**Interfaces:**
- Consumes: `scan(body: &str) -> TextRefs`, existing inline relationship parsing, fence handling, and container-prefix handling.
- Produces: section-aware `blocked_by` and `blocks` vectors with the same sorted, deduplicated contract.

- [ ] **Step 1: Write failing dependency-section tests**

Add tests that name the production breaks they catch:

```rust
#[test]
fn scans_dependency_references_beneath_markdown_headings() {
    let refs = scan(
        "## Blocked by\n\n- #17\n- #19\n## Blocks ##\n* #23\n",
    );

    assert_eq!(refs.blocked_by, vec![17, 19]);
    assert_eq!(refs.blocks, vec![23]);
}

#[test]
fn relationship_section_ends_at_the_next_heading() {
    let refs = scan("## Blocked by\n- #17\n## Acceptance criteria\n- #99\n");

    assert_eq!(refs.blocked_by, vec![17]);
}

#[test]
fn section_references_still_ignore_fenced_examples_and_deduplicate_inline_refs() {
    let refs = scan(
        "Blocked by #17\n## Blocked by\n- #17\n```\n- #99\n```\n- #19\n",
    );

    assert_eq!(refs.blocked_by, vec![17, 19]);
}
```

- [ ] **Step 2: Run the focused tests and verify RED**

Run:

```powershell
cargo.exe test -p stellr-github textref::tests --locked
```

Expected: the new tests compile but fail because list items beneath relationship headings are not associated with a bucket.

- [ ] **Step 3: Implement exact heading and section parsing**

In `textref.rs`, introduce a private dependency-section enum and an ATX heading parser:

```rust
#[derive(Clone, Copy)]
enum RelationshipSection {
    BlockedBy,
    Blocks,
}

fn atx_heading(line: &str) -> Option<&str> {
    let line = line.trim_start();
    let level = line.bytes().take_while(|byte| *byte == b'#').count();
    if !(1..=6).contains(&level)
        || !line.as_bytes().get(level).is_some_and(u8::is_ascii_whitespace)
    {
        return None;
    }

    let title = line[level..].trim();
    let without_markers = title.trim_end_matches('#');
    if without_markers.len() != title.len()
        && without_markers
            .chars()
            .last()
            .is_some_and(char::is_whitespace)
    {
        Some(without_markers.trim_end())
    } else {
        Some(title)
    }
}
```

Track `section: Option<RelationshipSection>` beside the existing fence state. A heading always replaces or clears the section. For non-heading, non-fenced lines, preserve the existing inline `Blocked by`/`Blocks` detection; otherwise route the stripped content into the active section. Extract all `#` followed immediately by ASCII digits through one helper, then retain the existing sort/dedup finalization.

- [ ] **Step 4: Run focused tests and verify GREEN**

Run:

```powershell
cargo.exe test -p stellr-github textref::tests --locked
```

Expected: all text-reference tests pass.

- [ ] **Step 5: Commit the dependency-section slice**

```powershell
git add -- crates/github/src/textref.rs
git -c gpg.ssh.program=C:/Windows/System32/OpenSSH/ssh-keygen.exe commit -m "fix(github): parse dependency relationship sections"
```

---

### Task 2: Enrich fresh snapshots with parent and dependency relationships

**Files:**
- Modify: `crates/github/src/textref.rs`
- Modify: `crates/github/src/sync.rs`
- Test: `crates/github/tests/sync_test.rs`

**Interfaces:**
- Consumes: `TextRefs`, `RawIssue`, native `blocked_by`, native `parent_issue`, and bodies already returned by the existing GraphQL issue query.
- Produces: `pub(crate) fn enrich_relationships(issues: &mut [RawIssue])`, which deterministically merges section and inline dependencies, applies `Blocks` inversions, and fills only a missing unambiguous parent.

- [ ] **Step 1: Write a failing provider-boundary test**

Add a `wiremock` test using the real `GithubProvider::fetch` mapping seam and the existing `node` fixture. Supply four issues:

```rust
node(1, "Root", Some(""), "https://example.test/o/r/issues/1", "OPEN", None, &[], None, &[], &[], None),
node(2, "Markdown child", Some("## Parent\n\n#1\n## Blocked by\n\n- #1"), "https://example.test/o/r/issues/2", "OPEN", None, &[], None, &[], &[], None),
node(3, "Native parent wins", Some("## Parent\n\n#1\n## Blocks\n\n- #2"), "https://example.test/o/r/issues/3", "OPEN", None, &[], None, &[], &[], Some(9)),
node(4, "Ambiguous parent", Some("## Parent\n\n- #1\n- #2"), "https://example.test/o/r/issues/4", "OPEN", None, &[], None, &[], &[], None),
```

Assert hand-derived results:

```rust
assert_eq!(issues[1].parent_issue, Some(1));
assert_eq!(issues[1].blocked_by, vec![1, 3]);
assert_eq!(issues[2].parent_issue, Some(9));
assert_eq!(issues[3].parent_issue, None);
```

- [ ] **Step 2: Run the provider test and verify RED**

Run:

```powershell
cargo.exe test -p stellr-github --test sync_test fetch_enriches_markdown_relationship_sections --locked -- --exact
```

Expected: the Markdown parent remains `None` and section relationships are incomplete.

- [ ] **Step 3: Implement deterministic relationship enrichment**

Extend `TextRefs` and the section enum with a parent bucket:

```rust
pub struct TextRefs {
    pub blocked_by: Vec<u64>,
    pub blocks: Vec<u64>,
    pub parents: Vec<u64>,
}
```

Sort and deduplicate `parents` with the other buckets. Add:

```rust
pub(crate) fn enrich_relationships(issues: &mut [RawIssue]) {
    let mut inversions = Vec::new();

    for issue in issues.iter_mut() {
        let refs = scan(&issue.body);
        issue.blocked_by.extend(refs.blocked_by);
        issue.blocked_by.sort_unstable();
        issue.blocked_by.dedup();
        inversions.extend(refs.blocks.into_iter().map(|target| (issue.number, target)));
        if issue.parent_issue.is_none() && refs.parents.len() == 1 {
            issue.parent_issue = Some(refs.parents[0]);
        }
    }

    let positions = issues
        .iter()
        .enumerate()
        .map(|(index, issue)| (issue.number, index))
        .collect::<HashMap<_, _>>();
    for (blocker, target) in inversions {
        if let Some(&index) = positions.get(&target) {
            issues[index].blocked_by.push(blocker);
        }
    }
    for issue in issues {
        issue.blocked_by.sort_unstable();
        issue.blocked_by.dedup();
    }
}
```

Import `HashMap` and `RawIssue` in `textref.rs`. Simplify `map_issues` so it constructs `RawIssue` values with only native relationships, then calls `textref::enrich_relationships(&mut issues)` once before returning. Do not change the GraphQL query or pagination.

- [ ] **Step 4: Run provider and parser suites and verify GREEN**

Run:

```powershell
cargo.exe test -p stellr-github --test sync_test --locked
cargo.exe test -p stellr-github textref::tests --locked
```

Expected: the new provider-boundary test and all existing mapping/parser tests pass.

- [ ] **Step 5: Commit the fresh-snapshot slice**

```powershell
git add -- crates/github/src/textref.rs crates/github/src/sync.rs crates/github/tests/sync_test.rs
git -c gpg.ssh.program=C:/Windows/System32/OpenSSH/ssh-keygen.exe commit -m "fix(github): enrich structured issue relationships"
```

---

### Task 3: Repair relationships from existing cache snapshots

**Files:**
- Modify: `crates/github/src/cache.rs`
- Test: `crates/github/src/cache.rs`

**Interfaces:**
- Consumes: `textref::enrich_relationships(&mut [RawIssue])` from Task 2 and deserialized `Snapshot` data.
- Produces: `Cache::load` returns an enriched in-memory snapshot without rewriting the cache or contacting GitHub.

- [ ] **Step 1: Write a failing cache-boundary test**

Construct and store a snapshot with three raw issues whose native relationship fields are empty:

```rust
let mut source = snapshot("Root", 1_753_000_000);
source.issues.push(RawIssue {
    number: 2,
    parent_issue: None,
    title: "Dependent".into(),
    body: "## Parent\n\n#1\n## Blocked by\n\n- #1".into(),
    state: IssueState::Open,
    assignees: vec![],
    milestone: None,
    labels: vec![],
    blocked_by: vec![],
    url: "u2".into(),
});
source.issues.push(RawIssue {
    number: 3,
    parent_issue: None,
    title: "Blocker by inversion".into(),
    body: "## Blocks\n\n- #2".into(),
    state: IssueState::Open,
    assignees: vec![],
    milestone: None,
    labels: vec![],
    blocked_by: vec![],
    url: "u3".into(),
});
```

After `cache.store` and `cache.load`, assert:

```rust
assert_eq!(loaded.issues[1].parent_issue, Some(1));
assert_eq!(loaded.issues[1].blocked_by, vec![1, 3]);
```

Store the loaded snapshot and load it again; assert the second load equals the first to prove idempotence.

- [ ] **Step 2: Run the cache test and verify RED**

Run:

```powershell
cargo.exe test -p stellr-github cache::tests::load_enriches_relationships_from_cached_bodies --locked -- --exact
```

Expected: the loaded relationship fields remain empty.

- [ ] **Step 3: Enrich only the deserialized in-memory cache value**

Change `Cache::load` to deserialize mutably, enrich, and return:

```rust
pub fn load(&self, repo: &RepoRef) -> Option<Snapshot> {
    let bytes = fs::read(self.path_for(repo)).ok()?;
    let mut snapshot: Snapshot = serde_json::from_slice(&bytes).ok()?;
    crate::textref::enrich_relationships(&mut snapshot.issues);
    Some(snapshot)
}
```

Do not write during load and do not add a provider call.

- [ ] **Step 4: Run the complete GitHub crate tests and verify GREEN**

Run:

```powershell
cargo.exe test -p stellr-github --locked
```

Expected: all unit and integration tests pass.

- [ ] **Step 5: Commit the cache-repair slice**

```powershell
git add -- crates/github/src/cache.rs
git -c gpg.ssh.program=C:/Windows/System32/OpenSSH/ssh-keygen.exe commit -m "fix(github): recover relationships from cached bodies"
```

---

### Task 4: Verify the complete fix and prepare the Windows installer

**Files:**
- Verify: `crates/github/src/textref.rs`
- Verify: `crates/github/src/sync.rs`
- Verify: `crates/github/src/cache.rs`
- Verify: `crates/github/tests/sync_test.rs`
- Verify: `web/src/lib/starmap/edge-visual.test.ts`
- Build output: `artifacts/windows-x64/Stellr_0.1.0_windows-x64_nsis_UNSIGNED-NOT-FOR-RELEASE.exe`

**Interfaces:**
- Consumes: completed parser, fresh-snapshot, and cache enrichment slices.
- Produces: native Windows verification evidence, two-axis code-review findings resolved, and a locally installable unsigned NSIS artifact.

- [ ] **Step 1: Run focused downstream renderer evidence**

```powershell
C:\Users\pfdev\.vite-plus\bin\npm.exe --prefix web test -- src/lib/starmap/edge-visual.test.ts
```

Expected: the real canvas seam continues to paint dependency/workflow strokes.

- [ ] **Step 2: Run complete frontend gates**

```powershell
C:\Users\pfdev\.vite-plus\bin\npm.exe --prefix web run check
C:\Users\pfdev\.vite-plus\bin\npm.exe --prefix web test
C:\Users\pfdev\.vite-plus\bin\npm.exe --prefix web run build
```

Expected: zero Svelte errors/warnings, all Vitest files pass, and the production bundle builds.

- [ ] **Step 3: Run complete native Rust gates**

```powershell
cargo.exe fmt --all -- --check
cargo.exe clippy --workspace --all-targets --locked -- -D warnings
cargo.exe test --workspace --locked -- --test-threads=1
cargo.exe build --workspace --locked
git diff --check
```

Expected: all commands exit zero. Tests requiring live GitHub writes may remain explicitly ignored; no executed test may fail.

- [ ] **Step 4: Run two-axis code review**

Use the `code-review` skill with fixed point `a530da4`, reviewing repository standards and the approved design independently. Resolve required findings with a fresh focused red/green cycle, rerun affected suites, and retain unrelated code unchanged.

- [ ] **Step 5: Build only the native Windows NSIS development installer**

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts\build-windows-nsis.ps1
```

Expected output includes:

```text
WINDOWS_NSIS_SIGNATURE=NotSigned
WINDOWS_NSIS_ARTIFACT=D:\dev\stellr\artifacts\windows-x64\Stellr_0.1.0_windows-x64_nsis_UNSIGNED-NOT-FOR-RELEASE.exe
```

Verify the file exists, its `.sha256` sidecar matches `Get-FileHash -Algorithm SHA256`, and no macOS or Linux packaging script ran.

- [ ] **Step 6: Confirm final branch scope**

```powershell
git status --short --branch
git log --oneline a530da4..HEAD
git diff --stat a530da4..HEAD
git diff --check a530da4..HEAD
```

Expected: the branch is clean; commits after the design cover only relationship parsing, snapshot/cache enrichment, and their tests.

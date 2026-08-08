# Immediate Space Publication Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make a successfully added GitHub repository appear in Stellr's sidebar through the next authoritative model publication without requiring another space action or restart.

**Architecture:** Keep the persisted `SpaceStore` and polled `Model` as the only sources of truth. After a successful add has been saved and the store lock released, notify the existing poller just as remove and manual refresh already do; the poller derives and broadcasts the new model through the watch hub.

**Tech Stack:** Rust 2024, Tokio `Notify` and `watch`, Axum, native Windows PowerShell, Cargo.

## Global Constraints

- Run development and validation with native Windows executables; do not use WSL or Linux tooling.
- Keep the frontend authoritative-model architecture and the add response `{ "id": "..." }` unchanged.
- Do not notify after validation, duplicate, or persistence failures.
- Add no dependencies and make no unrelated refactors.
- Keep `CHANGELOG.md` append-only, newest-first, and add pending work only under `Unreleased`.

## File Structure

- `crates/server/src/routes.rs` owns the add endpoint and will wake the existing poller after persistence.
- `crates/server/tests/api_test.rs` proves that add alone publishes a derived model.
- `CHANGELOG.md` records the user-visible correction under `Unreleased`.

---

### Task 1: Publish a model after adding a space

**Files:**
- Modify: `crates/server/src/routes.rs:101-115`
- Test: `crates/server/tests/api_test.rs:183-255`
- Modify: `CHANGELOG.md:3`

**Interfaces:**
- Consumes: `AppState.refresh: Arc<tokio::sync::Notify>` and the existing `run_poller` notification branch.
- Produces: a refresh notification after a successful `SpaceStore::save`, causing `AppState.hub` to publish a `Model` containing the added space.

- [x] **Step 1: Write the failing integration test**

Rename the existing add/refresh test to `add_repo_space_immediately_populates_the_model`, remove its explicit `POST /api/spaces/o-r/refresh`, and retain these independently derived assertions:

```rust
assert_eq!(model.spaces[0].id, "o-r");
assert_eq!(model.spaces[0].repo, "o/r");
assert_eq!(model.spaces[0].stars[0].number, 1);
```

- [x] **Step 2: Run the test and verify the exact failure**

Run:

```powershell
cargo.exe test -p stellr-server --test api_test add_repo_space_immediately_populates_the_model -- --exact --nocapture
```

Expected before implementation: FAIL after the two-second model wait with `adding a space should publish the derived model: Elapsed(())`.

- [x] **Step 3: Notify the poller after successful persistence**

In `add_space`, place the existing notification after `drop(spaces)` and before returning the response:

```rust
drop(spaces);
state.refresh.notify_one();

Json(AddSpaceResponse { id }).into_response()
```

All validation and save error returns remain before the notification.

- [x] **Step 4: Verify the focused red-green cycle**

Run:

```powershell
cargo.exe test -p stellr-server --test api_test add_repo_space_immediately_populates_the_model -- --exact --nocapture
```

Expected: PASS with one test run and no failure output.

- [x] **Step 5: Record the user-visible fix**

Add this as the first bullet under `## Unreleased` in `CHANGELOG.md`:

```markdown
- Made newly added repositories appear in the sidebar without restarting
  Stellr or performing another space action.
```

- [x] **Step 6: Run affected and workspace validation**

Run, in order:

```powershell
cargo.exe fmt --all -- --check
cargo.exe test -p stellr-server --test api_test --locked -- --test-threads=1
cargo.exe clippy --workspace --all-targets --locked -- -D warnings
cargo.exe test --workspace --locked -- --test-threads=1
```

Expected: every command exits zero with no formatting differences, lint warnings, or failed tests.

- [x] **Step 7: Review and commit the implementation**

Review `git diff --check`, confirm only the regression test, route notification, changelog, and this plan are in scope, then commit:

```powershell
git add -- crates/server/src/routes.rs crates/server/tests/api_test.rs CHANGELOG.md docs/superpowers/plans/2026-08-08-immediate-space-publication.md
git commit -m "fix(server): publish newly added spaces"
```

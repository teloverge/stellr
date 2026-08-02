# README Release Constellation Design

**Date:** 2026-08-02
**Status:** Approved design, pending written-spec review
**Repository:** Stellr

## Purpose

Stellr's README will lead with a living constellation that shows how the latest
release came together. The constellation uses the release's real GitHub issues,
final blocker topology, and lifecycle events. It replays meaningful status
changes, then settles on the release-time state.

The artifact is a product showcase first. It demonstrates Stellr's actual
visual and lifecycle grammar rather than presenting a generic SDLC diagram.

## Goals

- Show a truthful, release-scoped history using Stellr's real GitHub data.
- Preserve one deterministic graph layout for the entire animation.
- Use Stellr's existing status semantics and visual hierarchy.
- Refresh the showcase explicitly at release time, never continuously.
- Animate directly in GitHub's README when supported.
- Provide a broadly compatible PNG poster and reduced-motion path.
- Make the source story reviewable independently of the rendered assets.
- Preserve every accepted release showcase as a versioned artifact.

## Non-goals

- Do not display the entire repository graph.
- Do not invent events or simulate a representative lifecycle.
- Do not reconstruct historical blocker or hierarchy edits.
- Do not move stars, pan, zoom, or recompute layout during the replay.
- Do not make asset generation an automatic side effect of publishing a tag.
- Do not mutate GitHub issues, milestones, releases, or other tracker state.
- Do not silently publish stale, partial, or static-only output after a failure.
- Do not replace the interactive Stellr renderer with the SVG renderer.

## Chosen Approach

Add a dedicated native Windows release tool as the workspace component
`crates/showcase`. The component reads GitHub release evidence, builds an
auditable release-story manifest, computes one scene, and exports a self-contained
animated SVG plus a static PNG poster.

This is preferred over capturing the Canvas renderer because it produces a
sharp, lightweight, inspectable vector artifact with deterministic timing. It
is preferred over a static poster with a linked animation because the living
constellation should appear directly in GitHub's README.

The SVG renderer is a small release-artifact renderer alongside the interactive
Canvas renderer. It does not become a second application renderer or interaction
surface.

## Artifact Contract

Each accepted release adds three immutable files:

```text
docs/assets/readme-showcase/<version>.svg
docs/assets/readme-showcase/<version>.png
docs/assets/readme-showcase/<version>-story.json
```

`README.md` points to the newest version. Older files remain unchanged so an
older release can continue to reference its own story.

The three files are one publication unit:

- the JSON manifest records the evidence and derived story;
- the SVG animates that story;
- the PNG renders the same story's final release state.

The README reference is the publication commit point. The generator first
validates and writes all three versioned files, then updates the README last. A
failure before that final update leaves the previous showcase published. It may
leave complete but unreferenced files for the attempted version; the command
reports those paths and they are safe to remove or inspect before retrying.

## Architecture

```text
GitHub milestone, current issues, and lifecycle timelines
                         |
                         v
                GitHub release source
                         |
                         v
                 ReleaseStory builder
                         |
                         v
          deterministic scene and fixed layout
                    /             \
                   v               v
           animated SVG        final-frame PNG
                    \             /
                     v           v
                 review and accept
                         |
                         v
        versioned assets plus README reference
```

### GitHub release source

This unit owns read-only acquisition of:

- the repository and selected milestone;
- the previous release timestamp;
- the explicit current release cutoff;
- the current issue snapshot and blocker relationships;
- lifecycle timeline events required by the story.

The current-snapshot `Provider` remains unchanged. Release-history acquisition
is a different responsibility and uses a showcase-specific
`ReleaseHistorySource` seam. Its GitHub implementation may reuse the existing
authentication, pagination, and typed-error machinery without expanding the
runtime provider contract.

### ReleaseStory builder

This pure unit owns graph scoping, event normalization, historical state
reconstruction, status derivation, meaningful-change detection, beat grouping,
and manifest creation. It has no SVG or PNG knowledge.

### Scene and layout

This pure unit turns a `ReleaseStory` into stable node coordinates, edge curves,
label anchors, focus annotations, and animation timing. Layout depends on issue
numbers and final topology, never status or event order.

### SVG exporter

This unit emits a self-contained, script-free SVG from the scene. It owns only
README-specific paint and motion. It does not query GitHub or derive statuses.

### PNG exporter

This unit rasterizes the scene's final release state at a fixed size. It does
not attempt to sample an animated frame from a browser; the static scene is an
explicit output of the same scene model.

### Release command

The command orchestrates source acquisition, generation, validation, preview,
and explicit acceptance. It is the only unit that writes artifact files.

## Graph Scope

The visible release constellation contains:

1. every issue assigned to the selected release milestone at the cutoff;
2. each directly referenced blocker outside that milestone.

Milestone issues use full visual prominence. External prerequisite issues use a
`0.35` context multiplier and never compete with the release path.

Status derivation may require blockers that are not visible. The builder
therefore computes a hidden transitive blocker support set. Timeline state is
reconstructed across that support set, Stellr's ordinary derivation runs on the
complete support set, and only the selected visible nodes are projected into
the story. This prevents an omitted upstream blocker from making a context star
appear frontier when it was actually blocked.

The final release-time blocker edges remain fixed for the entire replay.
Historical dependency edits are not shown or inferred.

## Lifecycle Reconstruction

The story window begins at the previous release timestamp and ends at the
explicit current release cutoff. The generator refuses an implicit local-clock
cutoff so the same command remains reproducible.

For each issue in the derivation support set, the source provides the lifecycle
events needed to reconstruct:

- open, closed, and reopened state;
- closed-as-not-planned state only when the provider supplies explicit
  state-reason evidence; an absent reason never defaults to `out_of_scope`;
- assignment and unassignment.

Events have a stable ordering key of timestamp followed by provider event ID.
The builder reconstructs the state at the start of the window, applies events
chronologically, and invokes `stellr_core::derive` after each candidate event.
Events that do not change any visible star's derived status are retained as
evidence in the manifest but do not create an animation beat.

Stellr's existing status precedence remains authoritative:

1. closed as not planned -> `out_of_scope`;
2. closed -> `resolved`;
3. open with one or more assignees -> `claimed`;
4. open with an unresolved blocker -> `blocked`;
5. any other open issue -> `frontier`.

This allows one real blocker closure to resolve one star and move multiple
dependents onto the frontier in the same beat.

## Beat Construction

The replay has at most eight meaningful beats inside its eight-second story
window.

The deterministic grouping algorithm is:

1. create a candidate beat at each ordered event that changes a visible status;
2. merge candidates whose source timestamps are less than ten minutes apart;
3. if more than eight candidates remain, repeatedly merge the adjacent pair
   with the smallest time gap until eight remain;
4. break equal-gap ties by the earlier timestamp and then provider event ID;
5. distribute the resulting beats evenly across the eight-second replay window.

The manifest retains every source event and records which animation beat owns
it. Compression changes presentation timing only; it never changes event order
or derived state.

A release story must contain at least one visible status change. If it does not,
generation fails with an explanation instead of presenting an ambient snapshot
as a historical replay.

## Story Manifest

The versioned JSON manifest records:

- schema version and generator version;
- repository identity and release version;
- milestone ID and title;
- previous-release and current-cutoff timestamps;
- visible issue numbers and hidden derivation-support issue numbers;
- final visible topology;
- normalized source events and provider event IDs;
- candidate-to-beat grouping;
- every visible star's derived status at every beat;
- final deterministic node coordinates;
- output dimensions and timing constants.

The manifest excludes GitHub tokens, local filesystem paths, API response
headers, and unrelated issue bodies.

## Visual Composition

The SVG uses a `1200 x 675` 16:9 view box and scales responsively in the README.
The PNG poster is rendered at `1600 x 900`.

The composition uses Stellr's approved grammar:

- a pure-black star-map field;
- solid cores for `resolved` stars;
- hollow, status-colored cores for incomplete stars;
- solid mint resolved edges;
- dashed pale unresolved edges;
- arrows from blocker to dependent;
- motion only on a newly traversable resolved edge;
- full prominence for the active release path;
- reduced prominence for unrelated and external-context nodes and edges.

The release version and the phrase "How the release constellation came
together" appear inside the hero. Labels use an issue number plus a title
bounded to forty Unicode grapheme clusters, with an ellipsis when truncated.
Only stars changing in the current beat receive event labels.

The story playhead uses the existing CURRENT ring treatment on the beat's
primary changed issue. READY emphasis applies to issues newly entering the
frontier. For a beat with multiple changes, the primary issue is selected in
this order: newly claimed, newly resolved, newly frontier, then lowest issue
number. This focus is presentation metadata and never changes the issue's
derived status.

## Motion Grammar

The animation is a twelve-second seamless loop:

| Time | Phase | Behavior |
| --- | --- | --- |
| 0-1 seconds | Reveal | Fade in the fixed topology and release title. |
| 1-9 seconds | Replay | Apply up to eight real lifecycle beats. |
| 9-11 seconds | Settle | Hold the final release state for inspection. |
| 11-12 seconds | Loop | Crossfade softly to the opening state. |

The camera and node coordinates never move. A beat may animate:

- a hollow/solid or status-rim transition;
- resolved-edge particles toward newly available work;
- CURRENT and READY rings;
- one bounded issue label and short event caption.

Transitions use opacity, stroke, fill, and existing glow treatments. They do not
use zoom, rotation, positional movement, or flashing.

## SVG and PNG Behavior

The SVG is self-contained and contains no JavaScript, `foreignObject`, remote
fonts, external styles, or remote image references. SVG elements, gradients,
filters, CSS, and declarative SVG animation are the only allowed mechanisms.

If declarative animation is not executed, the document's static presentation
must remain a meaningful release image. Reduced-motion CSS disables replay and
shows the final release state.

The PNG poster is generated from the explicit final scene, not from a timed
browser screenshot. The SVG final state and PNG scene must have identical node
positions, statuses, visible edges, title, and release summary.

Accepted output budgets are `750 KiB` for the SVG, `1.5 MiB` for the PNG, and
`1 MiB` for the JSON manifest. Exceeding any budget fails generation and reports
the measured size.

## README Integration

GitHub-first presentation uses a `<picture>` element with:

- the animated SVG when the viewer permits motion;
- the PNG poster when `prefers-reduced-motion` is active;
- descriptive alternative text on the fallback `<img>`.

A nearby ordinary Markdown link exposes the PNG for strict renderers that strip
raw HTML. A short adjacent sentence summarizes the release issue count and
resolved outcome so the release story is not available only as an image.

GitHub documents repository SVG display, relative image paths, and the
`<picture>` element, but it does not promise animation behavior across every
browser. A compatibility spike against a rendered GitHub README is therefore a
completion gate. The spike must include Firefox because GitHub documents an SVG
rendering caveat there.

References:

- <https://docs.github.com/en/get-started/writing-on-github/getting-started-with-writing-and-formatting-on-github/basic-writing-and-formatting-syntax>
- <https://docs.github.com/en/repositories/working-with-files/using-files/working-with-non-code-files>

## Native Windows Release Workflow

Generation is an explicit release checklist step. A representative preview
command is:

```powershell
cargo.exe run -p stellr-showcase -- preview `
  --milestone "v0.1.0" `
  --from-release "v0.0.0" `
  --cutoff "2026-08-02T19:00:00Z"
```

`preview` performs read-only GitHub queries and writes only to:

```text
target/readme-showcase/<version>/
```

That directory contains the SVG, PNG, manifest, and a local HTML review page.
It does not modify tracked files.

After the maintainer reviews the visual and manifest, an explicit command
accepts the exact preview:

```powershell
cargo.exe run -p stellr-showcase -- accept `
  --preview target/readme-showcase/v0.1.0
```

`accept` verifies the preview digest, writes each versioned artifact through a
temporary sibling and an atomic rename, verifies all three destination digests,
and updates the README reference last through its own atomic replacement. An
interruption may leave unreferenced versioned artifacts, but the README never
points to a missing or partially written set.

The implementation and validation workflow uses native Windows PowerShell and
native Windows executables only.

## Failure Behavior

The generator fails closed for:

- missing or ambiguous milestone or release boundaries;
- authentication rejection or rate limiting;
- incomplete pagination or a partial timeline;
- an issue state that cannot be reconstructed from available evidence;
- missing state-reason evidence required to display `out_of_scope`;
- invalid normalized topology or a deterministic-layout failure;
- a non-deterministic repeated layout result;
- invalid SVG structure or forbidden content;
- PNG rendering failure;
- an empty release constellation or a story with no visible status change;
- SVG over `750 KiB`, PNG over `1.5 MiB`, or manifest over `1 MiB`.

Failure reports the exact issue, event, API stage, or output check. It does not
reuse stale caches, drop problematic issues, invent an event, or publish only
one of the required assets.

The accepted showcase from the previous release remains unchanged until a new
preview passes and is explicitly accepted.

## Security and Privacy

- GitHub access is read-only and uses the existing token-resolution policy.
- Tokens and response headers never enter the manifest or assets.
- Issue bodies are not fetched for or embedded in the showcase.
- Labels and assignee names are used only for derivation and are not rendered.
- Rendered titles are XML-escaped and bounded in length.
- The SVG contains no script-capable or externally loaded content.
- Acceptance validates output paths under the repository's showcase directory
  before replacing files.

## Accessibility

- Completion is communicated by solid versus hollow shape, not color alone.
- The SVG contains `<title>` and `<desc>` metadata.
- The README image has concise alternative text and adjacent text summary.
- No element flashes or rapidly alternates luminance.
- `prefers-reduced-motion` selects the final static state.
- The final release frame holds for two seconds before the loop transition.
- Event captions supplement status color and edge motion.
- Context fading never makes the active release path illegible.

## Testing Strategy

### Source and story tests

- Previous/current cutoff validation.
- Milestone selection and visible-node scoping.
- Hidden transitive support-set construction.
- Open, closed, reopened, assigned, and unassigned reconstruction.
- `closed_not_planned` evidence handling.
- Stable timestamp/event-ID ordering.
- Existing five-status precedence through `stellr_core::derive`.
- One blocker closure moving multiple dependents to `frontier`.
- Non-status events retained as evidence but omitted as beats.
- Ten-minute grouping and deterministic reduction to eight beats.
- No-change releases failing with a clear error.

### Determinism and layout tests

- Reversed issue and event input order produces the same manifest and assets.
- Repeated generation produces byte-identical outputs.
- Status-only beat changes preserve every coordinate.
- Visible nodes contain milestone issues plus direct external blockers only.
- Hidden support nodes affect derivation without appearing in the scene.
- Blocker cycles remain cycle-safe, deterministically laid out, and retain the
  core model's blocked semantics.

### SVG tests

- Required view box, title, description, release label, nodes, and edges.
- Twelve-second timing and four motion phases.
- No node-position animation or camera transform.
- Motion occurs only on newly traversable resolved edges.
- Solid/hollow completion grammar and CURRENT/READY precedence.
- Reduced-motion final-state presentation.
- XML escaping and bounded labels.
- Absence of scripts, `foreignObject`, and external URLs.

### PNG and integration tests

- Exact output dimensions and final-state scene equivalence.
- Golden-image regression with an explicit review workflow for changes.
- Asset-size budget enforcement.
- Preview never modifies tracked files.
- Accept writes and verifies all three artifacts before atomically replacing the
  README reference as the publication commit point.
- Interrupted or invalid acceptance leaves prior files untouched.
- Native Windows workspace tests and release-command smoke tests.

### Live compatibility acceptance

With explicit authorization, publish a minimal probe on a review branch and
inspect the rendered README on GitHub. Confirm:

- animation runs in the primary supported GitHub browser path;
- reduced-motion presents the PNG/final state;
- the PNG link remains usable when raw HTML is unavailable;
- Firefox behavior matches the documented caveat or degrades to the fallback;
- no GitHub sanitization removes required SVG animation elements;
- the full-width hero remains readable at desktop and narrow README widths.

## Acceptance Criteria

- The visible graph is the selected release milestone plus direct external
  blockers, with hidden transitive support used for truthful derivation.
- Every animation beat maps to recorded GitHub lifecycle events.
- The existing Stellr status derivation is authoritative at every beat.
- Historical blocker edits are neither reconstructed nor implied.
- The graph layout is fixed and deterministic for the complete twelve-second
  loop.
- The replay contains no more than eight deterministic status beats.
- The SVG uses the approved Stellr node, edge, focus, and motion grammar.
- The final SVG state and PNG poster depict the same release scene.
- Reduced-motion and strict-Markdown readers have a static path.
- Preview is read-only with respect to tracked files and GitHub state.
- Accept promotes versioned SVG, PNG, and manifest assets as one unit and updates
  the README reference.
- Failed generation or acceptance leaves the previous release showcase intact.
- Outputs are deterministic, validated, accessible, and within size budgets.
- GitHub README compatibility is proven with an authorized live probe before
  the feature is declared complete.

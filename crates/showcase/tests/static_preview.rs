use stellr_showcase::{
    ClosureReason, IssueSnapshot, LifecycleEvent, LifecycleEventKind, MilestoneIdentity,
    PreviewRenderError, PreviousRelease, RecordedIssue, ReleaseBoundaries, ReleaseEvidence,
    ReleaseStory, SnapshotState, StartingSnapshot, UtcTimestamp, render_static_preview,
    validate_svg_safety,
};

fn sha256(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};

    format!("{:x}", Sha256::digest(bytes))
}

fn rgba(pixmap: &resvg::tiny_skia::Pixmap, x: u32, y: u32) -> [u8; 4] {
    let index = ((y * pixmap.width() + x) * 4) as usize;
    pixmap.data()[index..index + 4].try_into().unwrap()
}

fn ts(value: &str) -> UtcTimestamp {
    value.parse().expect("valid UTC fixture timestamp")
}

fn snapshot(state: SnapshotState, assignees: &[&str]) -> IssueSnapshot {
    IssueSnapshot {
        state,
        assignees: assignees.iter().map(|login| (*login).to_owned()).collect(),
    }
}

fn issue(
    number: u64,
    title: &str,
    milestone_id: Option<&str>,
    blocked_by: &[u64],
    final_snapshot: IssueSnapshot,
) -> RecordedIssue {
    RecordedIssue {
        number,
        title: title.to_owned(),
        url: format!("https://github.com/teloverge/stellr/issues/{number}"),
        milestone_id: milestone_id.map(str::to_owned),
        blocked_by: blocked_by.to_vec(),
        starting_snapshot: StartingSnapshot::Existing(snapshot(SnapshotState::Open, &[])),
        final_snapshot,
    }
}

fn event(
    id: &str,
    occurred_at: &str,
    issue_number: u64,
    kind: LifecycleEventKind,
) -> LifecycleEvent {
    LifecycleEvent {
        provider_event_id: id.to_owned(),
        occurred_at: ts(occurred_at),
        issue_number,
        kind,
    }
}

fn story() -> ReleaseStory {
    ReleaseStory::build(
        ReleaseEvidence {
            repository: "teloverge/stellr".to_owned(),
            release_version: "v0.2.0".to_owned(),
            milestone: MilestoneIdentity {
                id: "M1".to_owned(),
                title: "M1 — the chart".to_owned(),
            },
            issues: vec![
                issue(
                    10,
                    "External prerequisite",
                    None,
                    &[],
                    snapshot(SnapshotState::Closed, &[]),
                ),
                issue(
                    20,
                    "Render the final scene",
                    Some("M1"),
                    &[10],
                    snapshot(SnapshotState::Closed, &["teloverge"]),
                ),
                issue(
                    30,
                    "Escape <unsafe> & bound this deliberately long constellation title after forty grapheme clusters",
                    Some("M1"),
                    &[20],
                    snapshot(SnapshotState::Open, &[]),
                ),
                issue(
                    40,
                    "Publish the release",
                    Some("M1"),
                    &[30],
                    snapshot(SnapshotState::Open, &[]),
                ),
            ],
            events: vec![
                event(
                    "C10",
                    "2026-07-01T00:30:00Z",
                    10,
                    LifecycleEventKind::Closed {
                        reason: ClosureReason::Completed,
                    },
                ),
                event(
                    "A20",
                    "2026-07-01T00:45:00Z",
                    20,
                    LifecycleEventKind::Assigned {
                        login: "teloverge".to_owned(),
                    },
                ),
                event(
                    "C20",
                    "2026-07-01T01:00:00Z",
                    20,
                    LifecycleEventKind::Closed {
                        reason: ClosureReason::Completed,
                    },
                ),
            ],
        },
        ReleaseBoundaries {
            starting_cutoff: None,
            previous_release: Some(PreviousRelease {
                version: "v0.1.0".to_owned(),
                released_at: ts("2026-07-01T00:00:00Z"),
            }),
            ending_cutoff: Some(ts("2026-07-01T02:00:00Z")),
        },
    )
    .unwrap()
}

#[test]
fn one_story_produces_byte_identical_safe_static_assets() {
    let story = story();
    let first = render_static_preview(&story).unwrap();
    let second = render_static_preview(&story).unwrap();

    assert_eq!(first.svg, second.svg);
    assert_eq!(first.png, second.png);
    assert_eq!(first.manifest, second.manifest);
    assert_eq!(first.review_html, second.review_html);
    assert_eq!(first.manifest, serde_json::to_vec(&story).unwrap());
    assert_eq!(
        sha256(&first.svg),
        "47c93ba16d1bb4f9e0d584502abe28e31dd0c065123c339fde4e45d1a8299a9c"
    );
    assert_eq!(
        sha256(&first.png),
        "91e57921a15999a63f0c434ffd731ebd0e9efdd5f38a3d6ea1643fc585c1a32e"
    );
    assert_eq!(
        sha256(&first.manifest),
        "fd9c285d50b774703164b277d3c3dac24e1761004672a4273c8e6576be5a8796"
    );
    assert_eq!(
        sha256(&first.review_html),
        "b96885ffc43b0948aebfd48fa7f3d8b1453eeb5c3466f371a606538adea161fe"
    );

    let svg = std::str::from_utf8(&first.svg).unwrap();
    validate_svg_safety(svg).unwrap();
    for required in [
        "viewBox=\"0 0 1200 675\"",
        "<title ",
        "<desc ",
        "Release v0.2.0",
        "How the release constellation came together",
        "3 release issues · 1 resolved · 1 external prerequisite",
        "data-issue=\"10\"",
        "data-issue=\"20\"",
        "data-issue=\"30\"",
        "data-issue=\"40\"",
        "data-status=\"resolved\"",
        "data-status=\"frontier\"",
        "data-completion=\"solid\"",
        "data-completion=\"hollow\"",
        "data-blocker=\"10\" data-dependent=\"20\"",
        "marker-end=\"url(#arrow-resolved)\"",
        "marker-end=\"url(#arrow-unresolved)\"",
        "opacity=\"0.35\"",
        "#30 Escape &lt;unsafe&gt; &amp; bound this deliberatel…",
    ] {
        assert!(svg.contains(required), "static SVG is missing {required:?}");
    }
    assert!(!svg.contains("<unsafe>"));

    assert_eq!(
        first.png.get(..8),
        Some(&[137, 80, 78, 71, 13, 10, 26, 10][..])
    );
    assert_eq!(&first.png[12..16], b"IHDR");
    assert_eq!(
        u32::from_be_bytes(first.png[16..20].try_into().unwrap()),
        1600
    );
    assert_eq!(
        u32::from_be_bytes(first.png[20..24].try_into().unwrap()),
        900
    );
    let pixmap = resvg::tiny_skia::Pixmap::decode_png(&first.png).unwrap();
    assert_eq!(rgba(&pixmap, 320, 449), [65, 75, 69, 255]);
    assert_eq!(rgba(&pixmap, 640, 449), [185, 214, 196, 255]);
    assert_eq!(rgba(&pixmap, 797, 470), [190, 225, 200, 255]);
    assert_eq!(rgba(&pixmap, 960, 449), [0, 0, 0, 255]);
    assert_eq!(rgba(&pixmap, 975, 449), [138, 216, 255, 255]);
    assert_eq!(rgba(&pixmap, 1_280, 449), [0, 0, 0, 255]);
    assert_eq!(rgba(&pixmap, 1_289, 449), [226, 195, 195, 255]);

    let html = std::str::from_utf8(&first.review_html).unwrap();
    assert!(html.starts_with("<!doctype html>"));
    assert!(html.contains("Release v0.2.0"));
    assert!(html.contains("3 release issues · 1 resolved · 1 external prerequisite"));
    assert!(html.contains(svg));

    assert!(first.svg.len() <= 750 * 1024);
    assert!(first.png.len() <= 1536 * 1024);
    assert!(first.manifest.len() <= 1024 * 1024);
}

#[test]
fn an_oversized_manifest_fails_before_assets_are_returned() {
    let mut story = story();
    story.repository = "x".repeat(1024 * 1024);

    let error = render_static_preview(&story).unwrap_err();

    assert!(matches!(
        error,
        PreviewRenderError::AssetTooLarge {
            kind: "manifest",
            limit: 1_048_576,
            ..
        }
    ));
}

#[test]
#[ignore = "writes local issue #50 visual-review artifacts"]
fn write_local_static_preview_for_visual_review() {
    let preview = render_static_preview(&story()).unwrap();
    let output = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .unwrap()
        .join("target/readme-showcase/issue-50-review");
    std::fs::create_dir_all(&output).unwrap();
    std::fs::write(output.join("final.svg"), preview.svg).unwrap();
    std::fs::write(output.join("final.png"), preview.png).unwrap();
    std::fs::write(output.join("story.json"), preview.manifest).unwrap();
    std::fs::write(output.join("review.html"), preview.review_html).unwrap();
}

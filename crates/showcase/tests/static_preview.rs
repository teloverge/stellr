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
fn one_story_produces_byte_identical_safe_release_assets() {
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
        "0f33cf093df3766f4c75de84275ddca04f23d9572eafbe27ea13178f3620fb52"
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
        "42f27f42957b2fb623dea532ba4dea42629b6fcc8cdd9e0a9a5965b7234f4c52"
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
fn animated_replay_uses_truthful_fixed_twelve_second_motion() {
    let preview = render_static_preview(&story()).unwrap();
    let svg = std::str::from_utf8(&preview.svg).unwrap();
    let document = roxmltree::Document::parse(svg).unwrap();

    let replay = document
        .descendants()
        .find(|node| node.attribute("id") == Some("animated-replay"))
        .expect("animated replay group");
    assert_eq!(replay.attribute("data-loop-ms"), Some("12000"));
    assert_eq!(replay.attribute("data-reveal-ms"), Some("1000"));
    assert_eq!(replay.attribute("data-replay-ms"), Some("8000"));
    assert_eq!(replay.attribute("data-final-hold-ms"), Some("2000"));
    assert_eq!(replay.attribute("data-soft-reset-ms"), Some("1000"));

    let beats = document
        .descendants()
        .filter(|node| node.attribute("data-beat").is_some())
        .filter(|node| node.attribute("data-role") == Some("beat-focus"))
        .collect::<Vec<_>>();
    assert_eq!(beats.len(), 3);
    assert_eq!(beats[0].attribute("data-beat"), Some("0"));
    assert_eq!(beats[0].attribute("data-replay-offset-ms"), Some("2666"));
    assert_eq!(beats[0].attribute("data-event-ids"), Some("C10"));
    assert_eq!(beats[0].attribute("data-changed-issues"), Some("10 20"));
    assert_eq!(beats[0].attribute("data-primary-issue"), Some("10"));
    assert_eq!(beats[0].attribute("data-ready-issues"), Some("20"));
    assert_eq!(beats[2].attribute("data-beat"), Some("2"));
    assert_eq!(beats[2].attribute("data-replay-offset-ms"), Some("8000"));
    assert_eq!(beats[2].attribute("data-event-ids"), Some("C20"));
    assert_eq!(beats[2].attribute("data-changed-issues"), Some("20 30"));
    assert_eq!(beats[2].attribute("data-primary-issue"), Some("20"));
    assert_eq!(beats[2].attribute("data-ready-issues"), Some("30"));

    let focus = beats[2]
        .descendants()
        .filter_map(|node| Some((node.attribute("data-focus")?, node.attribute("data-issue")?)))
        .collect::<Vec<_>>();
    assert!(focus.contains(&("current", "20")));
    assert!(focus.contains(&("ready", "30")));
    assert!(beats[2].descendants().any(|node| {
        node.attribute("data-caption") == Some("Resolved")
            && node.attribute("data-issue") == Some("20")
    }));

    let traversals = document
        .descendants()
        .filter(|node| node.attribute("data-motion") == Some("newly-traversable"))
        .map(|node| {
            (
                node.attribute("data-beat").unwrap(),
                node.attribute("data-blocker").unwrap(),
                node.attribute("data-dependent").unwrap(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(traversals, vec![("0", "10", "20"), ("2", "20", "30")]);

    let style = document
        .descendants()
        .find(|node| node.has_tag_name(("http://www.w3.org/2000/svg", "style")))
        .and_then(|node| node.text())
        .expect("embedded motion CSS");
    assert!(style.contains("animation-duration:12s"));
    assert!(style.contains("@media (prefers-reduced-motion:reduce)"));
    assert!(style.contains("#animated-replay{display:none}"));
    assert!(style.contains("#final-scene{animation:none;opacity:1}"));
    assert!(!style.contains("transform"));
    assert!(!svg.contains("<animate"));
    assert!(!svg.contains("<set"));

    let final_scene = document
        .descendants()
        .find(|node| node.attribute("id") == Some("final-scene"))
        .expect("final scene fallback");
    assert_eq!(final_scene.attribute("data-static-state"), Some("final"));
    assert_eq!(final_scene.attribute("data-state-after-beat"), Some("2"));
}

#[test]
fn animation_beat_without_exact_manifest_evidence_fails_closed() {
    let mut story = story();
    story.beats[0].source_event_ids = vec!["invented-event".to_owned()];

    let error = render_static_preview(&story).unwrap_err();

    assert!(matches!(error, PreviewRenderError::IncompleteStory(_)));
    assert!(
        error
            .to_string()
            .contains("animation beat 0 does not map exactly to its manifest evidence")
    );
}

#[test]
fn animation_beat_with_statuses_not_derived_from_evidence_fails_closed() {
    let mut story = story();
    story.beats[0]
        .statuses
        .iter_mut()
        .find(|status| status.issue_number == 10)
        .unwrap()
        .status = Some(stellr_core::Status::Blocked);

    let error = render_static_preview(&story).unwrap_err();

    assert!(matches!(error, PreviewRenderError::IncompleteStory(_)));
    assert!(
        error
            .to_string()
            .contains("release story does not match the canonical replay derived from evidence")
    );
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

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use stellr_core::RepoRef;
use stellr_showcase::{
    ClosureReason, DefaultPreviewRenderer, IssueSnapshot, LifecycleEvent, LifecycleEventKind,
    LiveReleaseRequest, MilestoneIdentity, PreviewOperationError, PreviewRenderError,
    PreviewRenderer, PreviousRelease, RecordedIssue, ReleaseBoundaries, ReleaseEvidence,
    ReleaseHistoryError, ReleaseHistorySource, ReleaseStory, ReleaseWindowStart, SnapshotState,
    StartingSnapshot, StaticPreview, UtcTimestamp, generate_release_preview,
};

fn ts(value: &str) -> UtcTimestamp {
    value.parse().expect("valid UTC fixture timestamp")
}

fn story() -> ReleaseStory {
    ReleaseStory::build(
        ReleaseEvidence {
            repository: "teloverge/stellr".to_owned(),
            release_version: "v0.2.0".to_owned(),
            milestone: MilestoneIdentity {
                id: "M1".to_owned(),
                title: "M1".to_owned(),
            },
            issues: vec![
                RecordedIssue {
                    number: 10,
                    title: "External prerequisite".to_owned(),
                    url: "https://github.com/teloverge/stellr/issues/10".to_owned(),
                    milestone_id: None,
                    blocked_by: vec![],
                    starting_snapshot: StartingSnapshot::Existing(IssueSnapshot {
                        state: SnapshotState::Open,
                        assignees: vec![],
                    }),
                    final_snapshot: IssueSnapshot {
                        state: SnapshotState::Closed,
                        assignees: vec![],
                    },
                },
                RecordedIssue {
                    number: 20,
                    title: "Release work".to_owned(),
                    url: "https://github.com/teloverge/stellr/issues/20".to_owned(),
                    milestone_id: Some("M1".to_owned()),
                    blocked_by: vec![10],
                    starting_snapshot: StartingSnapshot::Existing(IssueSnapshot {
                        state: SnapshotState::Open,
                        assignees: vec![],
                    }),
                    final_snapshot: IssueSnapshot {
                        state: SnapshotState::Open,
                        assignees: vec![],
                    },
                },
            ],
            events: vec![LifecycleEvent {
                provider_event_id: "C10".to_owned(),
                occurred_at: ts("2026-07-01T00:30:00Z"),
                issue_number: 10,
                kind: LifecycleEventKind::Closed {
                    reason: ClosureReason::Completed,
                },
            }],
        },
        ReleaseBoundaries {
            starting_cutoff: None,
            previous_release: Some(PreviousRelease {
                version: "v0.1.0".to_owned(),
                released_at: ts("2026-07-01T00:00:00Z"),
            }),
            ending_cutoff: Some(ts("2026-07-01T01:00:00Z")),
        },
    )
    .unwrap()
}

fn request() -> LiveReleaseRequest {
    LiveReleaseRequest {
        release_version: "v0.2.0".to_owned(),
        milestone_title: "M1".to_owned(),
        start: ReleaseWindowStart::PreviousRelease {
            tag: "v0.1.0".to_owned(),
        },
        ending_cutoff: ts("2026-07-01T01:00:00Z"),
    }
}

struct FakeSource {
    story: Option<ReleaseStory>,
}

#[async_trait]
impl ReleaseHistorySource for FakeSource {
    async fn build_story(
        &self,
        _repository: &RepoRef,
        _request: LiveReleaseRequest,
    ) -> Result<ReleaseStory, ReleaseHistoryError> {
        self.story
            .clone()
            .ok_or_else(|| ReleaseHistoryError::Partial {
                stage: "timeline pagination",
                detail: "fixture ended before the final page".to_owned(),
            })
    }
}

struct FailingRenderer;

impl PreviewRenderer for FailingRenderer {
    fn render(&self, _story: &ReleaseStory) -> Result<StaticPreview, PreviewRenderError> {
        Err(PreviewRenderError::Rasterization(
            "fixture rasterizer stopped".to_owned(),
        ))
    }
}

struct AlternatingRenderer {
    calls: AtomicUsize,
}

#[derive(Clone, Copy)]
enum Corruption {
    UnsafeSvg,
    OversizedSvg,
    InvalidPng,
    SafeButWrongSvg,
    ValidButWrongPng,
    ActiveReviewHtml,
}

struct CorruptingRenderer(Corruption);

impl PreviewRenderer for CorruptingRenderer {
    fn render(&self, story: &ReleaseStory) -> Result<StaticPreview, PreviewRenderError> {
        let mut preview = DefaultPreviewRenderer.render(story)?;
        match self.0 {
            Corruption::UnsafeSvg => {
                let svg = String::from_utf8(preview.svg).unwrap();
                preview.svg = svg.replace("</svg>", "<script/></svg>").into_bytes();
            }
            Corruption::OversizedSvg => preview.svg.resize(750 * 1024 + 1, b' '),
            Corruption::InvalidPng => preview.png.truncate(16),
            Corruption::SafeButWrongSvg => {
                preview.svg = String::from_utf8(preview.svg)
                    .unwrap()
                    .replacen("Release v0.2.0", "Release v9.9.9", 1)
                    .into_bytes();
            }
            Corruption::ValidButWrongPng => {
                let mut pixmap = resvg::tiny_skia::Pixmap::decode_png(&preview.png).unwrap();
                pixmap.data_mut()[..4].copy_from_slice(&[255, 0, 0, 255]);
                preview.png = pixmap.encode_png().unwrap();
            }
            Corruption::ActiveReviewHtml => {
                let html = String::from_utf8(preview.review_html).unwrap();
                preview.review_html = html
                    .replace("</body>", "<script>alert('active')</script></body>")
                    .into_bytes();
            }
        }
        Ok(preview)
    }
}

impl PreviewRenderer for AlternatingRenderer {
    fn render(&self, story: &ReleaseStory) -> Result<StaticPreview, PreviewRenderError> {
        let mut preview = DefaultPreviewRenderer.render(story)?;
        if self.calls.fetch_add(1, Ordering::SeqCst) == 1 {
            preview.review_html.push(b' ');
        }
        Ok(preview)
    }
}

struct TempRepository(PathBuf);

impl TempRepository {
    fn new(test_name: &str) -> Self {
        static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "stellr-{test_name}-{}-{}",
            std::process::id(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempRepository {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn repo() -> RepoRef {
    RepoRef {
        owner: "teloverge".to_owned(),
        name: "stellr".to_owned(),
    }
}

#[tokio::test]
async fn preview_publishes_all_four_artifacts_and_repeats_byte_identically() {
    let repository = TempRepository::new("preview-success");
    let source = FakeSource {
        story: Some(story()),
    };

    let first = generate_release_preview(
        &source,
        &DefaultPreviewRenderer,
        &repo(),
        request(),
        repository.path(),
    )
    .await
    .unwrap();
    let first_bytes = ["release.svg", "release.png", "story.json", "review.html"]
        .map(|name| fs::read(first.directory.join(name)).unwrap());
    let entries = fs::read_dir(&first.directory)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect::<Vec<_>>();
    assert_eq!(entries.len(), 4);

    let second = generate_release_preview(
        &source,
        &DefaultPreviewRenderer,
        &repo(),
        request(),
        repository.path(),
    )
    .await
    .unwrap();
    let second_bytes = ["release.svg", "release.png", "story.json", "review.html"]
        .map(|name| fs::read(second.directory.join(name)).unwrap());

    assert_eq!(first.directory, second.directory);
    assert_eq!(first_bytes, second_bytes);
    assert!(first.directory.ends_with("target/readme-showcase/v0.2.0"));
}

#[tokio::test]
async fn partial_live_history_publishes_nothing_and_names_the_source_stage() {
    let repository = TempRepository::new("preview-source-failure");
    let source = FakeSource { story: None };

    let error = generate_release_preview(
        &source,
        &DefaultPreviewRenderer,
        &repo(),
        request(),
        repository.path(),
    )
    .await
    .unwrap_err();

    assert!(matches!(error, PreviewOperationError::History(_)));
    assert!(
        error
            .to_string()
            .contains("partial GitHub timeline pagination")
    );
    assert!(!repository.path().join("target/readme-showcase").exists());
}

#[tokio::test]
async fn renderer_failure_publishes_nothing_and_names_the_render_stage() {
    let repository = TempRepository::new("preview-render-failure");
    let source = FakeSource {
        story: Some(story()),
    };

    let error = generate_release_preview(
        &source,
        &FailingRenderer,
        &repo(),
        request(),
        repository.path(),
    )
    .await
    .unwrap_err();

    assert!(matches!(error, PreviewOperationError::Render(_)));
    assert!(error.to_string().contains("fixture rasterizer stopped"));
    assert!(!repository.path().join("target/readme-showcase").exists());
}

#[tokio::test]
async fn nondeterministic_generation_publishes_nothing() {
    let repository = TempRepository::new("preview-nondeterministic");
    let source = FakeSource {
        story: Some(story()),
    };
    let renderer = AlternatingRenderer {
        calls: AtomicUsize::new(0),
    };

    let error = generate_release_preview(&source, &renderer, &repo(), request(), repository.path())
        .await
        .unwrap_err();

    assert!(matches!(error, PreviewOperationError::Nondeterministic));
    assert!(!repository.path().join("target/readme-showcase").exists());
}

#[tokio::test]
async fn invalid_outputs_publish_nothing_and_name_the_exact_validation_stage() {
    for (index, (corruption, expected_stage)) in [
        (Corruption::UnsafeSvg, "SVG safety"),
        (Corruption::OversizedSvg, "SVG budget"),
        (Corruption::InvalidPng, "PNG dimensions"),
        (Corruption::SafeButWrongSvg, "SVG semantic agreement"),
        (Corruption::ValidButWrongPng, "PNG final-scene agreement"),
        (Corruption::ActiveReviewHtml, "review page agreement"),
    ]
    .into_iter()
    .enumerate()
    {
        let repository = TempRepository::new(&format!("preview-invalid-{index}"));
        let source = FakeSource {
            story: Some(story()),
        };

        let error = generate_release_preview(
            &source,
            &CorruptingRenderer(corruption),
            &repo(),
            request(),
            repository.path(),
        )
        .await
        .unwrap_err();

        assert!(matches!(
            error,
            PreviewOperationError::OutputValidation { .. }
        ));
        assert!(error.to_string().contains(expected_stage));
        assert!(!repository.path().join("target/readme-showcase").exists());
    }
}

#[cfg(windows)]
fn create_junction(link: &Path, target: &Path) {
    let status = std::process::Command::new("cmd.exe")
        .args([
            "/d",
            "/c",
            "mklink",
            "/J",
            &link.display().to_string(),
            &target.display().to_string(),
        ])
        .status()
        .unwrap();
    assert!(status.success());
}

#[cfg(windows)]
#[tokio::test]
async fn target_junction_is_rejected_before_any_external_write() {
    let repository = TempRepository::new("preview-target-junction");
    let external = TempRepository::new("preview-target-junction-external");
    create_junction(&repository.path().join("target"), external.path());
    let source = FakeSource {
        story: Some(story()),
    };

    let error = generate_release_preview(
        &source,
        &DefaultPreviewRenderer,
        &repo(),
        request(),
        repository.path(),
    )
    .await
    .unwrap_err();

    assert!(error.to_string().contains("target directory"));
    assert!(!external.path().join("readme-showcase").exists());
}

#[cfg(windows)]
#[tokio::test]
async fn destination_junction_is_rejected_and_left_untouched() {
    let repository = TempRepository::new("preview-destination-junction");
    let external = TempRepository::new("preview-destination-junction-external");
    let preview_root = repository.path().join("target").join("readme-showcase");
    fs::create_dir_all(&preview_root).unwrap();
    create_junction(&preview_root.join("v0.2.0"), external.path());
    let source = FakeSource {
        story: Some(story()),
    };

    let error = generate_release_preview(
        &source,
        &DefaultPreviewRenderer,
        &repo(),
        request(),
        repository.path(),
    )
    .await
    .unwrap_err();

    assert!(error.to_string().contains("preview destination"));
    assert!(external.path().read_dir().unwrap().next().is_none());
}

#[tokio::test]
async fn a_different_existing_preview_is_preserved_and_not_replaced() {
    let repository = TempRepository::new("preview-existing-differs");
    let source = FakeSource {
        story: Some(story()),
    };
    let receipt = generate_release_preview(
        &source,
        &DefaultPreviewRenderer,
        &repo(),
        request(),
        repository.path(),
    )
    .await
    .unwrap();
    let svg = receipt.directory.join("release.svg");
    fs::write(&svg, b"maintainer review copy").unwrap();

    let error = generate_release_preview(
        &source,
        &DefaultPreviewRenderer,
        &repo(),
        request(),
        repository.path(),
    )
    .await
    .unwrap_err();

    assert!(matches!(
        error,
        PreviewOperationError::ExistingPreviewDiffers { .. }
    ));
    assert_eq!(fs::read(svg).unwrap(), b"maintainer review copy");
}

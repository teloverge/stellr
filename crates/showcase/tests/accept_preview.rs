use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use stellr_showcase::{
    ClosureReason, IssueSnapshot, LifecycleEvent, LifecycleEventKind, MilestoneIdentity,
    PreviousRelease, RecordedIssue, ReleaseBoundaries, ReleaseEvidence, ReleaseStory,
    SnapshotState, StartingSnapshot, StaticPreview, UtcTimestamp, accept_release_preview,
    preview_digest, render_static_preview,
};

const ORIGINAL_README: &str = "# stellr\n\n## Release constellation compatibility probe\n\nold showcase\n\n## Lineage & acknowledgements\n\nlineage\n";

fn ts(value: &str) -> UtcTimestamp {
    value.parse().expect("valid UTC fixture timestamp")
}

fn story_with_version(release_version: &str) -> ReleaseStory {
    ReleaseStory::build(
        ReleaseEvidence {
            repository: "teloverge/stellr".to_owned(),
            release_version: release_version.to_owned(),
            milestone: MilestoneIdentity {
                id: "M1".to_owned(),
                title: "M1 — the chart".to_owned(),
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

struct TempRepository(PathBuf);

impl TempRepository {
    fn new(test_name: &str) -> Self {
        static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "stellr-accept-{test_name}-{}-{}",
            std::process::id(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        fs::write(path.join("README.md"), ORIGINAL_README).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    fn write_preview(&self) -> (PathBuf, StaticPreview) {
        self.write_preview_with_version("v0.2.0")
    }

    fn write_preview_with_version(&self, version: &str) -> (PathBuf, StaticPreview) {
        let preview = render_static_preview(&story_with_version(version)).unwrap();
        let directory = self
            .path()
            .join("target")
            .join("readme-showcase")
            .join(version);
        fs::create_dir_all(&directory).unwrap();
        for (name, bytes) in [
            ("release.svg", preview.svg.as_slice()),
            ("release.png", preview.png.as_slice()),
            ("story.json", preview.manifest.as_slice()),
            ("review.html", preview.review_html.as_slice()),
        ] {
            fs::write(directory.join(name), bytes).unwrap();
        }
        (directory, preview)
    }
}

impl Drop for TempRepository {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
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

#[test]
fn reviewed_preview_digest_has_a_stable_domain_separated_format() {
    let preview = StaticPreview {
        svg: vec![1],
        png: vec![2, 3],
        manifest: vec![4, 5, 6],
        review_html: vec![],
    };

    assert_eq!(
        preview_digest(&preview),
        "sha256:1053a8995629ae425b85617dd1060553c7aa3e90f4c30c1ef590ee5a9d91e6d6"
    );
}

#[test]
fn a_wrong_reviewed_digest_changes_no_tracked_output() {
    let repository = TempRepository::new("wrong-digest");
    let (preview_directory, _) = repository.write_preview();

    let error = accept_release_preview(
        repository.path(),
        &preview_directory,
        "sha256:0000000000000000000000000000000000000000000000000000000000000000",
    )
    .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("reviewed preview digest mismatch")
    );
    assert_eq!(
        fs::read_to_string(repository.path().join("README.md")).unwrap(),
        ORIGINAL_README
    );
    assert!(!repository.path().join("docs/assets").exists());
}

#[test]
fn an_unexpected_review_file_fails_before_tracked_output() {
    let repository = TempRepository::new("unexpected-file");
    let (preview_directory, preview) = repository.write_preview();
    fs::write(preview_directory.join("notes.txt"), b"not reviewed").unwrap();

    let error = accept_release_preview(
        repository.path(),
        &preview_directory,
        &preview_digest(&preview),
    )
    .unwrap_err();

    assert!(error.to_string().contains("exact four-file preview"));
    assert_eq!(
        fs::read_to_string(repository.path().join("README.md")).unwrap(),
        ORIGINAL_README
    );
    assert!(!repository.path().join("docs/assets").exists());
}

#[test]
fn a_digest_matching_semantically_wrong_artifacts_still_fails_closed() {
    let repository = TempRepository::new("wrong-artifact");
    let (preview_directory, mut preview) = repository.write_preview();
    preview.svg = String::from_utf8(preview.svg)
        .unwrap()
        .replacen("Release v0.2.0", "Release v9.9.9", 1)
        .into_bytes();
    fs::write(preview_directory.join("release.svg"), &preview.svg).unwrap();

    let error = accept_release_preview(
        repository.path(),
        &preview_directory,
        &preview_digest(&preview),
    )
    .unwrap_err();

    assert!(error.to_string().contains("SVG semantic agreement"));
    assert_eq!(
        fs::read_to_string(repository.path().join("README.md")).unwrap(),
        ORIGINAL_README
    );
    assert!(!repository.path().join("docs/assets").exists());
}

#[test]
fn preview_directory_version_must_match_the_reviewed_story() {
    let repository = TempRepository::new("version-mismatch");
    let (preview_directory, preview) = repository.write_preview();
    let mismatched_directory = preview_directory.with_file_name("v0.2.1");
    fs::rename(&preview_directory, &mismatched_directory).unwrap();

    let error = accept_release_preview(
        repository.path(),
        &mismatched_directory,
        &preview_digest(&preview),
    )
    .unwrap_err();

    assert!(error.to_string().contains("release version 'v0.2.0'"));
    assert!(error.to_string().contains("directory 'v0.2.1'"));
    assert!(!repository.path().join("docs/assets").exists());
}

#[cfg(windows)]
#[test]
fn preview_directory_junction_is_rejected_before_tracked_output() {
    let repository = TempRepository::new("preview-junction");
    let external = TempRepository::new("preview-junction-external");
    let (external_preview, preview) = external.write_preview();
    let preview_root = repository.path().join("target").join("readme-showcase");
    fs::create_dir_all(&preview_root).unwrap();
    let linked_preview = preview_root.join("v0.2.0");
    create_junction(&linked_preview, &external_preview);

    let error = accept_release_preview(
        repository.path(),
        &linked_preview,
        &preview_digest(&preview),
    )
    .unwrap_err();

    assert!(error.to_string().contains("preview directory"));
    assert!(error.to_string().contains("reparse point"));
    assert!(!repository.path().join("docs/assets").exists());
}

#[test]
fn accepted_preview_writes_three_versioned_assets_before_the_readme_reference() {
    let repository = TempRepository::new("accept-success");
    let (preview_directory, preview) = repository.write_preview();
    let digest = preview_digest(&preview);

    let receipt = accept_release_preview(repository.path(), &preview_directory, &digest).unwrap();

    let asset_root = repository
        .path()
        .join("docs")
        .join("assets")
        .join("readme-showcase");
    let expected_assets = [
        asset_root.join("v0.2.0.svg"),
        asset_root.join("v0.2.0.png"),
        asset_root.join("v0.2.0-story.json"),
    ];
    assert_eq!(receipt.assets, expected_assets);
    assert_eq!(receipt.digest, digest);
    assert_eq!(fs::read(&receipt.assets[0]).unwrap(), preview.svg);
    assert_eq!(fs::read(&receipt.assets[1]).unwrap(), preview.png);
    assert_eq!(fs::read(&receipt.assets[2]).unwrap(), preview.manifest);

    let readme = fs::read_to_string(&receipt.readme).unwrap();
    for required in [
        "<!-- stellr-release-constellation:start -->",
        "media=\"(prefers-reduced-motion: reduce)\" srcset=\"docs/assets/readme-showcase/v0.2.0.png\"",
        "src=\"docs/assets/readme-showcase/v0.2.0.svg\"",
        "alt=\"Stellr v0.2.0 release constellation: 2 issues, 1 resolved\"",
        "[View the static v0\\.2\\.0 release constellation](docs/assets/readme-showcase/v0.2.0.png)",
        "Release v0\\.2\\.0 charts 2 visible issues, with 1 resolved at the recorded cutoff.",
        "<!-- stellr-release-constellation:end -->",
        "## Lineage & acknowledgements",
    ] {
        assert!(readme.contains(required), "README is missing {required:?}");
    }
    assert!(!readme.contains("old showcase"));
}

#[test]
fn a_conflicting_versioned_asset_is_preserved_and_the_readme_is_unchanged() {
    let repository = TempRepository::new("asset-conflict");
    let (preview_directory, preview) = repository.write_preview();
    let asset_root = repository
        .path()
        .join("docs")
        .join("assets")
        .join("readme-showcase");
    fs::create_dir_all(&asset_root).unwrap();
    let svg = asset_root.join("v0.2.0.svg");
    fs::write(&svg, b"previous immutable release bytes").unwrap();

    let error = accept_release_preview(
        repository.path(),
        &preview_directory,
        &preview_digest(&preview),
    )
    .unwrap_err();

    assert!(error.to_string().contains("different bytes"));
    assert_eq!(fs::read(&svg).unwrap(), b"previous immutable release bytes");
    assert!(!asset_root.join("v0.2.0.png").exists());
    assert_eq!(
        fs::read_to_string(repository.path().join("README.md")).unwrap(),
        ORIGINAL_README
    );
}

#[test]
fn accepting_the_same_review_twice_is_byte_identical() {
    let repository = TempRepository::new("accept-idempotent");
    let (preview_directory, preview) = repository.write_preview();
    let digest = preview_digest(&preview);

    let first = accept_release_preview(repository.path(), &preview_directory, &digest).unwrap();
    let first_bytes = [
        fs::read(&first.assets[0]).unwrap(),
        fs::read(&first.assets[1]).unwrap(),
        fs::read(&first.assets[2]).unwrap(),
        fs::read(&first.readme).unwrap(),
    ];
    let second = accept_release_preview(repository.path(), &preview_directory, &digest).unwrap();
    let second_bytes = [
        fs::read(&second.assets[0]).unwrap(),
        fs::read(&second.assets[1]).unwrap(),
        fs::read(&second.assets[2]).unwrap(),
        fs::read(&second.readme).unwrap(),
    ];

    assert_eq!(first, second);
    assert_eq!(first_bytes, second_bytes);
}

#[cfg(windows)]
#[test]
fn readme_replacement_failure_reports_complete_unreferenced_assets() {
    let repository = TempRepository::new("readme-failure");
    let (preview_directory, preview) = repository.write_preview();
    let readme = repository.path().join("README.md");
    let original_permissions = fs::metadata(&readme).unwrap().permissions();
    let mut permissions = original_permissions.clone();
    permissions.set_readonly(true);
    fs::set_permissions(&readme, permissions).unwrap();

    let error = accept_release_preview(
        repository.path(),
        &preview_directory,
        &preview_digest(&preview),
    )
    .unwrap_err();

    let asset_root = repository
        .path()
        .join("docs")
        .join("assets")
        .join("readme-showcase");
    let expected_assets = vec![
        asset_root.join("v0.2.0.svg"),
        asset_root.join("v0.2.0.png"),
        asset_root.join("v0.2.0-story.json"),
    ];
    assert_eq!(error.unreferenced_assets(), expected_assets.as_slice());
    for path in &expected_assets {
        assert!(path.is_file());
        assert!(error.to_string().contains(&path.display().to_string()));
    }
    assert_eq!(fs::read_to_string(&readme).unwrap(), ORIGINAL_README);

    fs::set_permissions(readme, original_permissions).unwrap();
}

#[cfg(windows)]
#[test]
fn failed_reaccept_does_not_report_assets_already_referenced_by_the_readme() {
    let repository = TempRepository::new("reaccept-readme-failure");
    let (preview_directory, preview) = repository.write_preview();
    let digest = preview_digest(&preview);
    let receipt = accept_release_preview(repository.path(), &preview_directory, &digest).unwrap();
    let readme = receipt.readme;
    let altered = fs::read_to_string(&readme)
        .unwrap()
        .replace("with 1 resolved", "with one resolved");
    fs::write(&readme, &altered).unwrap();
    let original_permissions = fs::metadata(&readme).unwrap().permissions();
    let mut permissions = original_permissions.clone();
    permissions.set_readonly(true);
    fs::set_permissions(&readme, permissions).unwrap();

    let error = accept_release_preview(repository.path(), &preview_directory, &digest).unwrap_err();

    assert!(error.unreferenced_assets().is_empty());
    assert_eq!(fs::read_to_string(&readme).unwrap(), altered);
    fs::set_permissions(readme, original_permissions).unwrap();
}

#[cfg(windows)]
#[test]
fn failed_reaccept_reports_only_assets_missing_from_a_partial_reference() {
    let repository = TempRepository::new("partial-reference-failure");
    let (preview_directory, preview) = repository.write_preview();
    let digest = preview_digest(&preview);
    let receipt = accept_release_preview(repository.path(), &preview_directory, &digest).unwrap();
    let readme = receipt.readme;
    let altered = fs::read_to_string(&readme).unwrap().replace(
        "docs/assets/readme-showcase/v0.2.0.png",
        "docs/assets/readme-showcase/missing.png",
    );
    fs::write(&readme, &altered).unwrap();
    let original_permissions = fs::metadata(&readme).unwrap().permissions();
    let mut permissions = original_permissions.clone();
    permissions.set_readonly(true);
    fs::set_permissions(&readme, permissions).unwrap();

    let error = accept_release_preview(repository.path(), &preview_directory, &digest).unwrap_err();

    let asset_root = repository
        .path()
        .join("docs")
        .join("assets")
        .join("readme-showcase");
    assert_eq!(
        error.unreferenced_assets(),
        &[
            asset_root.join("v0.2.0.png"),
            asset_root.join("v0.2.0-story.json"),
        ]
    );
    assert_eq!(fs::read_to_string(&readme).unwrap(), altered);
    fs::set_permissions(readme, original_permissions).unwrap();
}

#[cfg(windows)]
#[test]
fn tracked_showcase_junction_cannot_redirect_accepted_assets() {
    let repository = TempRepository::new("asset-junction");
    let external = TempRepository::new("asset-junction-external");
    let (preview_directory, preview) = repository.write_preview();
    let assets = repository.path().join("docs").join("assets");
    fs::create_dir_all(&assets).unwrap();
    create_junction(&assets.join("readme-showcase"), external.path());

    let error = accept_release_preview(
        repository.path(),
        &preview_directory,
        &preview_digest(&preview),
    )
    .unwrap_err();

    assert!(error.to_string().contains("showcase asset directory"));
    assert!(error.to_string().contains("reparse point"));
    assert!(!external.path().join("v0.2.0.svg").exists());
    assert_eq!(
        fs::read_to_string(repository.path().join("README.md")).unwrap(),
        ORIGINAL_README
    );
}

#[cfg(windows)]
#[test]
fn reviewed_artifact_symlink_is_rejected_before_tracked_output() {
    use std::os::windows::fs::symlink_file;

    let repository = TempRepository::new("artifact-symlink");
    let external = TempRepository::new("artifact-symlink-external");
    let (preview_directory, preview) = repository.write_preview();
    let external_svg = external.path().join("reviewed.svg");
    fs::write(&external_svg, &preview.svg).unwrap();
    let linked_svg = preview_directory.join("release.svg");
    fs::remove_file(&linked_svg).unwrap();
    symlink_file(&external_svg, &linked_svg).unwrap();

    let error = accept_release_preview(
        repository.path(),
        &preview_directory,
        &preview_digest(&preview),
    )
    .unwrap_err();

    assert!(error.to_string().contains("preview artifact"));
    assert!(error.to_string().contains("reparse point"));
    assert!(!repository.path().join("docs/assets").exists());
}

#[test]
fn readme_escapes_a_windows_safe_release_name_for_html_and_markdown_urls() {
    let repository = TempRepository::new("escaped-version");
    let version = "M1 [launch] (beta) & `stars_~`#1%";
    let (preview_directory, preview) = repository.write_preview_with_version(version);

    let receipt = accept_release_preview(
        repository.path(),
        &preview_directory,
        &preview_digest(&preview),
    )
    .unwrap();

    let readme = fs::read_to_string(receipt.readme).unwrap();
    let encoded = "M1%20%5Blaunch%5D%20%28beta%29%20%26%20%60stars_~%60%231%25";
    assert!(readme.contains(&format!(
        "srcset=\"docs/assets/readme-showcase/{encoded}.png\""
    )));
    assert!(readme.contains(&format!(
        "src=\"docs/assets/readme-showcase/{encoded}.svg\""
    )));
    assert!(
        readme.contains("alt=\"Stellr M1 [launch] (beta) &amp; `stars_~`#1% release constellation")
    );
    assert!(readme.contains(
        "[View the static M1 \\[launch\\] \\(beta\\) \\& \\`stars\\_\\~\\`\\#1\\% release"
    ));
    assert!(readme.contains(&format!("](docs/assets/readme-showcase/{encoded}.png)")));
}

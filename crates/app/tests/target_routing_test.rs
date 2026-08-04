use std::{path::Path, process::Command};

use stellr_app::target::{RouteTarget, TargetResolver};

fn git(repo: &Path, arguments: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(arguments)
        .status()
        .unwrap();
    assert!(status.success(), "git command failed: {arguments:?}");
}

#[test]
fn paths_slugs_github_urls_and_stellr_links_use_one_normalized_target() {
    let profile = tempfile::tempdir().unwrap();
    let repo = profile.path().join("stellr");
    std::fs::create_dir(&repo).unwrap();
    git(&repo, &["init", "--quiet"]);
    git(
        &repo,
        &[
            "remote",
            "add",
            "origin",
            "git@github.com:teloverge/stellr.git",
        ],
    );
    let resolver = TargetResolver::new(profile.path().to_path_buf());
    let expected_repo = RouteTarget {
        space_id: "teloverge-stellr".into(),
        repo: "teloverge/stellr".into(),
        path: None,
        issue: None,
    };

    assert_eq!(
        resolver.resolve(repo.to_str().unwrap()).unwrap(),
        RouteTarget {
            path: Some(repo.clone()),
            ..expected_repo.clone()
        }
    );
    assert_eq!(
        resolver.resolve("stellr").unwrap(),
        RouteTarget {
            path: Some(repo),
            ..expected_repo.clone()
        }
    );
    assert_eq!(resolver.resolve("teloverge/stellr").unwrap(), expected_repo);
    assert_eq!(
        resolver
            .resolve("https://github.com/teloverge/stellr")
            .unwrap(),
        expected_repo
    );
    assert_eq!(
        resolver
            .resolve("https://github.com/teloverge/stellr/issues/62")
            .unwrap(),
        RouteTarget {
            issue: Some(62),
            ..expected_repo.clone()
        }
    );
    assert_eq!(
        resolver
            .resolve("stellr://space?repo=teloverge%2Fstellr&issue=62")
            .unwrap(),
        RouteTarget {
            issue: Some(62),
            ..expected_repo
        }
    );
}

#[test]
fn invalid_targets_are_rejected_without_producing_a_route() {
    let profile = tempfile::tempdir().unwrap();
    let resolver = TargetResolver::new(profile.path().to_path_buf());

    for target in [
        "",
        "not a target",
        "owner/repo/extra",
        "http://github.com/teloverge/stellr",
        "https://example.com/teloverge/stellr",
        "https://github.com/teloverge/stellr/pulls/1",
        "stellr://other?repo=teloverge%2Fstellr",
        "stellr://space?repo=teloverge%2Fstellr&issue=0",
    ] {
        assert!(
            resolver.resolve(target).is_err(),
            "{target} should be invalid"
        );
    }
}

use serde_json::{Value, json};
use stellr_core::{ProviderError, RepoRef, Status};
use stellr_showcase::{
    GithubReleaseHistorySource, LiveReleaseRequest, ReleaseHistoryError, ReleaseHistorySource,
    ReleaseWindowStart, UtcTimestamp,
};
use wiremock::matchers::{body_partial_json, body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn ts(value: &str) -> UtcTimestamp {
    value.parse().expect("valid UTC fixture timestamp")
}

fn page(nodes: Value) -> Value {
    page_with(nodes, false, None, None)
}

fn page_with(
    nodes: Value,
    has_next_page: bool,
    end_cursor: Option<&str>,
    total_count: Option<usize>,
) -> Value {
    json!({
        "pageInfo": { "hasNextPage": has_next_page, "endCursor": end_cursor },
        "totalCount": total_count.unwrap_or_else(|| nodes.as_array().map_or(0, Vec::len)),
        "nodes": nodes
    })
}

#[derive(Clone, Copy)]
enum TimelineMode {
    Complete,
    Paginated,
    MissingCursor,
}

#[derive(Clone, Copy)]
struct MountOptions {
    paginate_issues: bool,
    timeline: TimelineMode,
    milestone_updated_at: &'static str,
    snapshot_mutation_at: Option<&'static str>,
}

impl Default for MountOptions {
    fn default() -> Self {
        Self {
            paginate_issues: false,
            timeline: TimelineMode::Complete,
            milestone_updated_at: "2026-06-30T00:00:00Z",
            snapshot_mutation_at: None,
        }
    }
}

async fn mount_query(server: &MockServer, operation: &str, variables: Value, repository: Value) {
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .and(body_string_contains(operation))
        .and(body_partial_json(json!({ "variables": variables })))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({ "data": { "repository": repository } })),
        )
        .mount(server)
        .await;
}

async fn mount_live_release(server: &MockServer) {
    mount_live_release_with(server, MountOptions::default()).await;
}

async fn mount_live_release_with(server: &MockServer, options: MountOptions) {
    let mut issue_2_timeline = vec![
        json!({
            "__typename": "AssignedEvent",
            "id": "A2",
            "createdAt": "2026-07-01T00:45:00Z",
            "assignee": { "login": "teloverge" }
        }),
        json!({
            "__typename": "ClosedEvent",
            "id": "C2",
            "createdAt": "2026-07-01T01:00:00Z",
            "stateReason": "COMPLETED"
        }),
    ];
    if let Some(created_at) = options.snapshot_mutation_at {
        issue_2_timeline.push(json!({
            "__typename": "MilestonedEvent",
            "id": "M2",
            "createdAt": created_at
        }));
    }
    mount_query(
        server,
        "FetchShowcaseMilestones",
        json!({ "owner": "teloverge", "name": "stellr", "cursor": null }),
        json!({
            "milestones": page(json!([{
                "id": "M1",
                "title": "v0.2.0",
                "updatedAt": options.milestone_updated_at
            }]))
        }),
    )
    .await;
    mount_query(
        server,
        "FetchShowcaseReleases",
        json!({ "owner": "teloverge", "name": "stellr", "cursor": null }),
        json!({
            "releases": page(json!([{
                "tagName": "v0.1.0",
                "publishedAt": "2026-07-01T00:00:00Z",
                "isDraft": false
            }]))
        }),
    )
    .await;
    let issue_1 = json!({
        "id": "I1",
        "number": 1,
        "title": "Foundation",
        "url": "https://github.com/teloverge/stellr/issues/1",
        "createdAt": "2026-06-01T00:00:00Z",
        "milestone": null
    });
    let issue_2 = json!({
        "id": "I2",
        "number": 2,
        "title": "Release path",
        "url": "https://github.com/teloverge/stellr/issues/2",
        "createdAt": "2026-06-02T00:00:00Z",
        "milestone": { "id": "M1" }
    });
    if options.paginate_issues {
        mount_query(
            server,
            "FetchShowcaseIssues",
            json!({ "owner": "teloverge", "name": "stellr", "cursor": null }),
            json!({ "issues": page_with(json!([issue_1]), true, Some("I-CUR"), Some(2)) }),
        )
        .await;
        mount_query(
            server,
            "FetchShowcaseIssues",
            json!({ "owner": "teloverge", "name": "stellr", "cursor": "I-CUR" }),
            json!({ "issues": page_with(json!([issue_2]), false, None, Some(2)) }),
        )
        .await;
    } else {
        mount_query(
            server,
            "FetchShowcaseIssues",
            json!({ "owner": "teloverge", "name": "stellr", "cursor": null }),
            json!({ "issues": page(json!([issue_1, issue_2])) }),
        )
        .await;
    }

    for (number, blockers) in [(1, json!([])), (2, json!([{ "number": 1 }]))] {
        mount_query(
            server,
            "FetchShowcaseBlockers",
            json!({
                "owner": "teloverge",
                "name": "stellr",
                "number": number,
                "cursor": null
            }),
            json!({
                "issue": { "blockedBy": page(blockers) }
            }),
        )
        .await;
    }

    let timeline_page_info = match options.timeline {
        TimelineMode::Complete => json!({ "hasNextPage": false, "endCursor": null }),
        TimelineMode::Paginated => json!({ "hasNextPage": true, "endCursor": "T-CUR" }),
        TimelineMode::MissingCursor => json!({ "hasNextPage": true, "endCursor": null }),
    };
    let timeline_count = match options.timeline {
        TimelineMode::Complete => 1,
        TimelineMode::Paginated | TimelineMode::MissingCursor => 2,
    };
    mount_query(
        server,
        "FetchShowcaseTimeline",
        json!({
            "owner": "teloverge",
            "name": "stellr",
            "number": 1,
            "cursor": null
        }),
        json!({
            "issue": {
                "timelineItems": {
                    "pageInfo": timeline_page_info,
                    "totalCount": timeline_count,
                    "nodes": [{
                        "__typename": "ClosedEvent",
                        "id": "C1",
                        "createdAt": "2026-07-01T00:30:00Z",
                        "stateReason": "COMPLETED"
                    }]
                }
            }
        }),
    )
    .await;
    if matches!(options.timeline, TimelineMode::Paginated) {
        mount_query(
            server,
            "FetchShowcaseTimeline",
            json!({
                "owner": "teloverge",
                "name": "stellr",
                "number": 1,
                "cursor": "T-CUR"
            }),
            json!({
                "issue": {
                    "timelineItems": {
                        "pageInfo": { "hasNextPage": false, "endCursor": null },
                        "totalCount": 2,
                        "nodes": [{
                            "__typename": "AssignedEvent",
                            "id": "A1",
                            "createdAt": "2026-07-01T00:40:00Z",
                            "assignee": { "login": "teloverge" }
                        }]
                    }
                }
            }),
        )
        .await;
    }
    mount_query(
        server,
        "FetchShowcaseTimeline",
        json!({
            "owner": "teloverge",
            "name": "stellr",
            "number": 2,
            "cursor": null
        }),
        json!({
            "issue": {
                "timelineItems": page(Value::Array(issue_2_timeline))
            }
        }),
    )
    .await;
}

fn repository() -> RepoRef {
    RepoRef {
        owner: "teloverge".to_owned(),
        name: "stellr".to_owned(),
    }
}

fn later_release_request() -> LiveReleaseRequest {
    LiveReleaseRequest {
        release_version: "v0.2.0".to_owned(),
        milestone_title: "v0.2.0".to_owned(),
        start: ReleaseWindowStart::PreviousRelease {
            tag: "v0.1.0".to_owned(),
        },
        ending_cutoff: ts("2026-07-01T02:00:00Z"),
    }
}

#[tokio::test]
async fn live_github_evidence_builds_the_recorded_manifest_shape_without_leaks() {
    let server = MockServer::start().await;
    mount_live_release(&server).await;
    let source =
        GithubReleaseHistorySource::with_base_uri("secret-token".to_owned(), &server.uri())
            .unwrap();
    let request = later_release_request();

    let story = source.build_story(&repository(), request).await.unwrap();

    assert_eq!(story.visible_issue_numbers, vec![1, 2]);
    assert_eq!(story.hidden_support_issue_numbers, Vec::<u64>::new());
    assert_eq!(story.evidence.events.len(), 3);
    assert_eq!(
        story
            .final_statuses
            .iter()
            .map(|status| (status.issue_number, status.status))
            .collect::<Vec<_>>(),
        vec![(1, Some(Status::Resolved)), (2, Some(Status::Resolved))]
    );

    let manifest = serde_json::to_string(&story).unwrap();
    for forbidden in [
        "secret-token",
        "response_headers",
        "issue_body",
        "ready-for-agent",
        "D:\\\\tmp",
    ] {
        assert!(!manifest.contains(forbidden), "manifest leaked {forbidden}");
    }
}

#[tokio::test]
async fn live_source_completes_issue_and_timeline_pagination_before_building() {
    let server = MockServer::start().await;
    mount_live_release_with(
        &server,
        MountOptions {
            paginate_issues: true,
            timeline: TimelineMode::Paginated,
            ..MountOptions::default()
        },
    )
    .await;
    let source =
        GithubReleaseHistorySource::with_base_uri("tok".to_owned(), &server.uri()).unwrap();

    let story = source
        .build_story(&repository(), later_release_request())
        .await
        .unwrap();

    assert_eq!(story.visible_issue_numbers, vec![1, 2]);
    assert_eq!(story.evidence.events.len(), 4);
    assert!(
        story
            .evidence
            .events
            .iter()
            .any(|event| event.provider_event_id == "A1" && event.beat_index.is_none())
    );
}

#[tokio::test]
async fn missing_pagination_cursor_fails_as_partial_history() {
    let server = MockServer::start().await;
    mount_live_release_with(
        &server,
        MountOptions {
            paginate_issues: false,
            timeline: TimelineMode::MissingCursor,
            ..MountOptions::default()
        },
    )
    .await;
    let source =
        GithubReleaseHistorySource::with_base_uri("tok".to_owned(), &server.uri()).unwrap();

    let error = source
        .build_story(&repository(), later_release_request())
        .await
        .unwrap_err();

    assert_eq!(
        error.to_string(),
        "partial GitHub issue timeline: next page has no end cursor"
    );
}

#[tokio::test]
async fn milestone_changed_after_the_release_cutoff_fails_closed() {
    let server = MockServer::start().await;
    mount_live_release_with(
        &server,
        MountOptions {
            milestone_updated_at: "2026-07-01T03:00:00Z",
            ..MountOptions::default()
        },
    )
    .await;
    let source =
        GithubReleaseHistorySource::with_base_uri("tok".to_owned(), &server.uri()).unwrap();

    let error = source
        .build_story(&repository(), later_release_request())
        .await
        .unwrap_err();

    assert_eq!(
        error.to_string(),
        "current GitHub snapshot is newer than cutoff 2026-07-01T02:00:00Z: milestone 'v0.2.0' changed at 2026-07-01T03:00:00Z"
    );
}

#[tokio::test]
async fn issue_topology_changed_after_the_release_cutoff_fails_closed() {
    let server = MockServer::start().await;
    mount_live_release_with(
        &server,
        MountOptions {
            snapshot_mutation_at: Some("2026-07-01T03:00:00Z"),
            ..MountOptions::default()
        },
    )
    .await;
    let source =
        GithubReleaseHistorySource::with_base_uri("tok".to_owned(), &server.uri()).unwrap();

    let error = source
        .build_story(&repository(), later_release_request())
        .await
        .unwrap_err();

    assert_eq!(
        error.to_string(),
        "current GitHub snapshot is newer than cutoff 2026-07-01T02:00:00Z: issue #2 milestone changed at 2026-07-01T03:00:00Z (event M2)"
    );
}

#[tokio::test]
async fn first_release_uses_its_explicit_start_without_a_release_lookup() {
    let server = MockServer::start().await;
    mount_live_release(&server).await;
    let source =
        GithubReleaseHistorySource::with_base_uri("tok".to_owned(), &server.uri()).unwrap();
    let mut request = later_release_request();
    request.release_version = "v0.1.0".to_owned();
    request.start = ReleaseWindowStart::FirstRelease {
        starting_cutoff: ts("2026-07-01T00:00:00Z"),
    };

    let story = source.build_story(&repository(), request).await.unwrap();

    assert_eq!(story.boundaries.starting_cutoff, ts("2026-07-01T00:00:00Z"));
    assert!(story.boundaries.previous_release.is_none());
}

async fn single_response_error(response: ResponseTemplate) -> ReleaseHistoryError {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .respond_with(response)
        .mount(&server)
        .await;
    let source =
        GithubReleaseHistorySource::with_base_uri("tok".to_owned(), &server.uri()).unwrap();
    source
        .build_story(&repository(), later_release_request())
        .await
        .unwrap_err()
}

#[tokio::test]
async fn live_source_preserves_auth_rate_limit_and_parse_errors() {
    let auth = single_response_error(
        ResponseTemplate::new(401).set_body_json(json!({ "message": "Bad credentials" })),
    )
    .await;
    assert!(matches!(
        auth,
        ReleaseHistoryError::Provider(ProviderError::Auth(_))
    ));

    let rate_limited = single_response_error(
        ResponseTemplate::new(429)
            .insert_header("x-ratelimit-remaining", "0")
            .insert_header("x-ratelimit-reset", "1785700000")
            .set_body_json(json!({ "message": "secondary rate limit" })),
    )
    .await;
    assert!(matches!(
        rate_limited,
        ReleaseHistoryError::Provider(ProviderError::RateLimited {
            reset_epoch: Some(1_785_700_000)
        })
    ));

    let parse =
        single_response_error(ResponseTemplate::new(200).set_body_string("{not json")).await;
    assert!(matches!(
        parse,
        ReleaseHistoryError::Provider(ProviderError::Parse(_))
    ));
}

#[tokio::test]
#[ignore = "read-only live GitHub evidence smoke"]
async fn live_stellr_m1_builds_through_the_public_source_seam() {
    let source = GithubReleaseHistorySource::new().unwrap();
    let now: chrono::DateTime<chrono::Utc> = std::time::SystemTime::now().into();
    let ending_cutoff = ts(&now.to_rfc3339_opts(chrono::SecondsFormat::Secs, true));
    let story = source
        .build_story(
            &repository(),
            LiveReleaseRequest {
                release_version: "M1-source-smoke".to_owned(),
                milestone_title: "M1 — the chart".to_owned(),
                start: ReleaseWindowStart::FirstRelease {
                    starting_cutoff: ts("2026-07-31T00:00:00Z"),
                },
                ending_cutoff,
            },
        )
        .await
        .unwrap();

    assert_eq!(story.repository, "teloverge/stellr");
    assert!(!story.visible_issue_numbers.is_empty());
    assert!(!story.beats.is_empty());
}

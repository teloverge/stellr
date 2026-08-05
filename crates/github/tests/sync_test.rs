use serde_json::{Value, json};
use stellr_core::{IssueState, Provider, ProviderError, RawIssue, RepoRef};
use stellr_github::sync::GithubProvider;
use wiremock::matchers::{body_partial_json, body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn repo() -> RepoRef {
    RepoRef {
        owner: "o".into(),
        name: "r".into(),
    }
}

fn page(nodes: Value) -> Value {
    page_with_pagination(nodes, false, None)
}

fn page_with_pagination(nodes: Value, has_next_page: bool, end_cursor: Option<&str>) -> Value {
    json!({
        "data": {
            "repository": {
                "issues": {
                    "pageInfo": {
                        "hasNextPage": has_next_page,
                        "endCursor": end_cursor
                    },
                    "nodes": nodes
                }
            }
        }
    })
}

#[tokio::test]
async fn fetch_follows_pagination_until_the_repository_is_complete() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .and(body_partial_json(json!({
            "variables": {
                "owner": "o",
                "name": "r",
                "cursor": "CUR1"
            }
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(page(json!([node(
            1,
            "Second page",
            None,
            "https://example.test/o/r/issues/1",
            "OPEN",
            None,
            &[],
            None,
            &[],
            &[],
            None,
        )]))))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(page_with_pagination(
                json!([node(
                    2,
                    "First page",
                    None,
                    "https://example.test/o/r/issues/2",
                    "OPEN",
                    None,
                    &[],
                    None,
                    &[],
                    &[],
                    None,
                )]),
                true,
                Some("CUR1"),
            )),
        )
        .mount(&server)
        .await;

    let provider = GithubProvider::with_base_uri("tok".into(), &server.uri()).unwrap();
    let issues = provider.fetch(&repo()).await.unwrap();

    assert_eq!(
        issues.iter().map(|issue| issue.number).collect::<Vec<_>>(),
        vec![1, 2]
    );
}

#[tokio::test]
async fn fetch_maps_a_rejected_token_to_auth() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .respond_with(
            ResponseTemplate::new(401).set_body_json(json!({ "message": "Bad credentials" })),
        )
        .mount(&server)
        .await;

    let provider = GithubProvider::with_base_uri("rejected".into(), &server.uri()).unwrap();
    let error = provider.fetch(&repo()).await.unwrap_err();

    match error {
        ProviderError::Auth(message) => assert_eq!(message, "token rejected"),
        other => panic!("expected Auth error, got {other:?}"),
    }
}

#[tokio::test]
async fn fetch_maps_rate_limit_exhaustion_with_its_reset_time() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .respond_with(
            ResponseTemplate::new(403)
                .insert_header("x-ratelimit-remaining", "0")
                .insert_header("x-ratelimit-reset", "1753000000")
                .set_body_json(json!({ "message": "API rate limit exceeded" })),
        )
        .mount(&server)
        .await;

    let provider = GithubProvider::with_base_uri("tok".into(), &server.uri()).unwrap();
    let error = provider.fetch(&repo()).await.unwrap_err();

    assert!(matches!(
        error,
        ProviderError::RateLimited {
            reset_epoch: Some(1_753_000_000)
        }
    ));
}

#[tokio::test]
async fn fetch_maps_http_429_rate_limit_exhaustion() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .respond_with(
            ResponseTemplate::new(429)
                .insert_header("x-ratelimit-remaining", "0")
                .insert_header("x-ratelimit-reset", "1753000001")
                .set_body_json(json!({ "message": "secondary rate limit" })),
        )
        .mount(&server)
        .await;

    let provider = GithubProvider::with_base_uri("tok".into(), &server.uri()).unwrap();
    let error = provider.fetch(&repo()).await.unwrap_err();

    assert!(matches!(
        error,
        ProviderError::RateLimited {
            reset_epoch: Some(1_753_000_001)
        }
    ));
}

#[allow(clippy::too_many_arguments)]
fn node(
    number: u64,
    title: &str,
    body: Option<&str>,
    url: &str,
    state: &str,
    state_reason: Option<&str>,
    assignees: &[&str],
    milestone: Option<&str>,
    labels: &[&str],
    blocked_by: &[u64],
    parent: Option<u64>,
) -> Value {
    json!({
        "number": number,
        "title": title,
        "body": body,
        "url": url,
        "state": state,
        "stateReason": state_reason,
        "assignees": {
            "nodes": assignees
                .iter()
                .map(|login| json!({ "login": login }))
                .collect::<Vec<_>>()
        },
        "milestone": milestone.map(|title| json!({ "title": title })),
        "labels": {
            "nodes": labels
                .iter()
                .map(|name| json!({ "name": name }))
                .collect::<Vec<_>>()
        },
        "blockedBy": {
            "nodes": blocked_by
                .iter()
                .map(|number| json!({ "number": number }))
                .collect::<Vec<_>>()
        },
        "parent": parent.map(|number| json!({ "number": number }))
    })
}

async fn mount_graphql_response(server: &MockServer, response: Value) {
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .and(body_partial_json(json!({
            "variables": {
                "owner": "o",
                "name": "r",
                "cursor": null
            }
        })))
        .and(body_string_contains(
            "issues(first: 100, after: $cursor, states: [OPEN, CLOSED])",
        ))
        .and(body_string_contains("parent { number }"))
        .respond_with(ResponseTemplate::new(200).set_body_json(response))
        .mount(server)
        .await;
}

#[tokio::test]
async fn fetch_maps_complete_issue_shape_and_merges_dependency_sources() {
    let server = MockServer::start().await;
    mount_graphql_response(
        &server,
        page(json!([
            node(
                1,
                "Completed",
                Some(""),
                "https://example.test/o/r/issues/1",
                "CLOSED",
                Some("COMPLETED"),
                &["ada", "grace"],
                Some("M1"),
                &["bug", "urgent"],
                &[],
                None,
            ),
            node(
                2,
                "Not planned",
                None,
                "https://example.test/o/r/issues/2",
                "CLOSED",
                Some("NOT_PLANNED"),
                &[],
                None,
                &[],
                &[],
                None,
            ),
            node(
                3,
                "Merged dependencies",
                Some("Blocked by #1, #2, #7\nBlocked by #1\nBlocks #4, #999"),
                "https://example.test/o/r/issues/3",
                "OPEN",
                None,
                &["linus"],
                None,
                &["feature"],
                &[7, 2, 2],
                Some(16),
            ),
            node(
                4,
                "Inversion target",
                Some("Blocked by #8"),
                "https://example.test/o/r/issues/4",
                "OPEN",
                None,
                &[],
                Some("M2"),
                &["planning"],
                &[3],
                None,
            ),
        ])),
    )
    .await;

    let provider = GithubProvider::with_base_uri("tok".into(), &server.uri()).unwrap();
    let issues = provider.fetch(&repo()).await.unwrap();

    assert_eq!(issues[0].parent_issue, None);
    assert_eq!(issues[2].parent_issue, Some(16));

    assert_eq!(
        issues,
        vec![
            RawIssue {
                number: 1,
                parent_issue: None,
                title: "Completed".into(),
                body: "".into(),
                state: IssueState::Closed,
                assignees: vec!["ada".into(), "grace".into()],
                milestone: Some("M1".into()),
                labels: vec!["bug".into(), "urgent".into()],
                blocked_by: vec![],
                url: "https://example.test/o/r/issues/1".into(),
            },
            RawIssue {
                number: 2,
                parent_issue: None,
                title: "Not planned".into(),
                body: "".into(),
                state: IssueState::ClosedNotPlanned,
                assignees: vec![],
                milestone: None,
                labels: vec![],
                blocked_by: vec![],
                url: "https://example.test/o/r/issues/2".into(),
            },
            RawIssue {
                number: 3,
                parent_issue: Some(16),
                title: "Merged dependencies".into(),
                body: "Blocked by #1, #2, #7\nBlocked by #1\nBlocks #4, #999".into(),
                state: IssueState::Open,
                assignees: vec!["linus".into()],
                milestone: None,
                labels: vec!["feature".into()],
                blocked_by: vec![1, 2, 7],
                url: "https://example.test/o/r/issues/3".into(),
            },
            RawIssue {
                number: 4,
                parent_issue: None,
                title: "Inversion target".into(),
                body: "Blocked by #8".into(),
                state: IssueState::Open,
                assignees: vec![],
                milestone: Some("M2".into()),
                labels: vec!["planning".into()],
                blocked_by: vec![3, 8],
                url: "https://example.test/o/r/issues/4".into(),
            },
        ]
    );
}

#[tokio::test]
async fn fetch_enriches_markdown_relationship_sections() {
    let server = MockServer::start().await;
    mount_graphql_response(
        &server,
        page(json!([
            node(
                1,
                "Root",
                Some(""),
                "https://example.test/o/r/issues/1",
                "OPEN",
                None,
                &[],
                None,
                &[],
                &[],
                None,
            ),
            node(
                2,
                "Markdown child",
                Some("## Parent\n\n#1\n## Blocked by\n\n- #1"),
                "https://example.test/o/r/issues/2",
                "OPEN",
                None,
                &[],
                None,
                &[],
                &[],
                None,
            ),
            node(
                3,
                "Native parent wins",
                Some("## Parent\n\n#1\n## Blocks\n\n- #2"),
                "https://example.test/o/r/issues/3",
                "OPEN",
                None,
                &[],
                None,
                &[],
                &[],
                Some(9),
            ),
            node(
                4,
                "Ambiguous parent",
                Some("## Parent\n\n- #1\n- #2"),
                "https://example.test/o/r/issues/4",
                "OPEN",
                None,
                &[],
                None,
                &[],
                &[],
                None,
            ),
        ])),
    )
    .await;

    let provider = GithubProvider::with_base_uri("tok".into(), &server.uri()).unwrap();
    let issues = provider.fetch(&repo()).await.unwrap();

    assert_eq!(issues[1].parent_issue, Some(1));
    assert_eq!(issues[1].blocked_by, vec![1, 3]);
    assert_eq!(issues[2].parent_issue, Some(9));
    assert_eq!(issues[3].parent_issue, None);
}

#[tokio::test]
async fn fetch_maps_the_first_graphql_error_to_parse() {
    let server = MockServer::start().await;
    mount_graphql_response(
        &server,
        json!({
            "data": null,
            "errors": [
                { "message": "first failure" },
                { "message": "second failure" }
            ]
        }),
    )
    .await;

    let provider = GithubProvider::with_base_uri("tok".into(), &server.uri()).unwrap();
    let error = provider.fetch(&repo()).await.unwrap_err();

    match error {
        ProviderError::Parse(message) => assert_eq!(message, "first failure"),
        other => panic!("expected Parse error, got {other:?}"),
    }
}

#[tokio::test]
async fn fetch_prioritizes_the_first_graphql_error_over_malformed_data() {
    let server = MockServer::start().await;
    mount_graphql_response(
        &server,
        json!({
            "data": {
                "repository": {
                    "issues": {
                        "nodes": [{ "number": "not a number" }]
                    }
                }
            },
            "errors": [
                { "message": "first failure" },
                { "message": "second failure" }
            ]
        }),
    )
    .await;

    let provider = GithubProvider::with_base_uri("tok".into(), &server.uri()).unwrap();
    let error = provider.fetch(&repo()).await.unwrap_err();

    match error {
        ProviderError::Parse(message) => assert_eq!(message, "first failure"),
        other => panic!("expected Parse error, got {other:?}"),
    }
}

#[tokio::test]
async fn fetch_rejects_a_malformed_graphql_error_collection() {
    let server = MockServer::start().await;
    mount_graphql_response(
        &server,
        json!({
            "data": {
                "repository": {
                    "issues": {
                        "pageInfo": { "hasNextPage": false, "endCursor": null },
                        "nodes": []
                    }
                }
            },
            "errors": [
                {},
                { "message": "denied" }
            ]
        }),
    )
    .await;

    let provider = GithubProvider::with_base_uri("tok".into(), &server.uri()).unwrap();
    let error = provider.fetch(&repo()).await.unwrap_err();

    match error {
        ProviderError::Parse(message) => {
            assert_eq!(message, "malformed GraphQL error response")
        }
        other => panic!("expected Parse error, got {other:?}"),
    }
}

#[tokio::test]
async fn fetch_maps_a_malformed_response_shape_to_parse() {
    let server = MockServer::start().await;
    mount_graphql_response(&server, json!({ "data": { "repository": null } })).await;

    let provider = GithubProvider::with_base_uri("tok".into(), &server.uri()).unwrap();
    let error = provider.fetch(&repo()).await.unwrap_err();

    match error {
        ProviderError::Parse(_) => {}
        other => panic!("expected Parse error, got {other:?}"),
    }
}

#[tokio::test]
async fn fetch_maps_invalid_json_to_parse() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .respond_with(ResponseTemplate::new(200).set_body_string("{not json"))
        .mount(&server)
        .await;

    let provider = GithubProvider::with_base_uri("tok".into(), &server.uri()).unwrap();
    let error = provider.fetch(&repo()).await.unwrap_err();

    match error {
        ProviderError::Parse(_) => {}
        other => panic!("expected Parse error, got {other:?}"),
    }
}

#[tokio::test]
async fn fetch_maps_transport_failures_to_http() {
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .unwrap();
    let base_uri = format!("http://{}", listener.local_addr().unwrap());
    let close_connection = tokio::spawn(async move {
        let (connection, _) = listener.accept().await.unwrap();
        drop(connection);
    });

    let provider = GithubProvider::with_base_uri("tok".into(), &base_uri).unwrap();
    let error = provider.fetch(&repo()).await.unwrap_err();
    close_connection.await.unwrap();

    match error {
        ProviderError::Http(_) => {}
        other => panic!("expected Http error, got {other:?}"),
    }
}

use serde_json::{Value, json};
use stellr_core::{
    HistoryEventKind, HistoryPageRequest, MilestoneRef, Provider, ProviderError, RepoRef,
};
use stellr_github::sync::GithubProvider;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn repo() -> RepoRef {
    RepoRef {
        owner: "o".into(),
        name: "r".into(),
    }
}

fn request(cursor: Option<&str>) -> HistoryPageRequest {
    HistoryPageRequest {
        issue_id: "I_78".into(),
        issue_number: 78,
        cursor: cursor.map(str::to_owned),
        cutoff: 1_785_850_000,
    }
}

#[tokio::test]
async fn fetches_one_targeted_history_page_and_normalizes_tracked_events() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "repository": {
                    "id": "R_repo",
                    "issue": {
                        "id": "I_78",
                        "timelineItems": {
                            "pageInfo": { "hasNextPage": true, "endCursor": "CUR2" },
                            "edges": [
                                { "cursor": "EDGE1", "node": {
                                    "__typename": "ReopenedEvent",
                                    "id": "E_reopen",
                                    "createdAt": "2026-08-04T13:10:00Z"
                                }},
                                { "cursor": "EDGE2", "node": {
                                    "__typename": "AssignedEvent",
                                    "id": "E_ignore",
                                    "createdAt": "2026-08-04T13:05:00Z"
                                }},
                                { "cursor": "EDGE3", "node": {
                                    "__typename": "ClosedEvent",
                                    "id": "E_close",
                                    "createdAt": "2026-08-04T13:00:00Z"
                                }},
                                { "cursor": "EDGE4", "node": {
                                    "__typename": "DemilestonedEvent",
                                    "id": "E_demilestone",
                                    "createdAt": "2026-08-04T13:15:00Z",
                                    "milestoneTitle": "Alpha"
                                }},
                                { "cursor": "EDGE5", "node": {
                                    "__typename": "MilestonedEvent",
                                    "id": "E_milestone",
                                    "createdAt": "2026-08-04T13:15:00Z",
                                    "milestoneTitle": "Beta"
                                }},
                                { "cursor": "EDGE6", "node": {
                                    "__typename": "ClosedEvent",
                                    "id": "E_after_cutoff",
                                    "createdAt": "2026-08-04T14:00:00Z"
                                }}
                            ]
                        }
                    }
                }
            }
        })))
        .expect(1)
        .mount(&server)
        .await;
    let provider = GithubProvider::with_base_uri("tok".into(), &server.uri()).unwrap();

    let page = provider
        .fetch_history_page(&repo(), &request(Some("CUR1")))
        .await
        .unwrap();
    let received = server.received_requests().await.unwrap();
    let body: Value = serde_json::from_slice(&received[0].body).unwrap();
    let query = body["query"].as_str().unwrap();

    assert!(query.contains("issue(number: $number)"));
    assert!(query.contains(
        "itemTypes: [CLOSED_EVENT, REOPENED_EVENT, DEMILESTONED_EVENT, MILESTONED_EVENT]"
    ));
    assert_eq!(body["variables"]["owner"], "o");
    assert_eq!(body["variables"]["name"], "r");
    assert_eq!(body["variables"]["number"], 78);
    assert_eq!(body["variables"]["cursor"], "CUR1");
    assert_eq!(page.next_cursor, None);
    assert_eq!(page.resume_cursor.as_deref(), Some("EDGE5"));
    assert!(page.complete);
    assert_eq!(page.events.len(), 4);
    assert_eq!(page.events[0].provider_event_id, "E_close");
    assert!(matches!(page.events[0].kind, HistoryEventKind::IssueClosed));
    assert_eq!(page.events[1].provider_event_id, "E_reopen");
    assert!(matches!(
        page.events[1].kind,
        HistoryEventKind::IssueReopened
    ));
    assert_eq!(page.events[2].provider_event_id, "E_demilestone");
    assert_eq!(
        page.events[2].kind,
        HistoryEventKind::MilestoneChanged {
            from: Some(MilestoneRef {
                id: None,
                title: "Alpha".into(),
            }),
            to: None,
        }
    );
    assert_eq!(page.events[3].provider_event_id, "E_milestone");
    assert_eq!(
        page.events[3].kind,
        HistoryEventKind::MilestoneChanged {
            from: None,
            to: Some(MilestoneRef {
                id: None,
                title: "Beta".into(),
            }),
        }
    );
}

#[tokio::test]
async fn preserves_the_terminal_cursor_for_delta_only_resume() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "repository": {
                    "id": "R_repo",
                    "issue": {
                        "id": "I_78",
                        "timelineItems": {
                            "pageInfo": { "hasNextPage": false, "endCursor": "CUR_END" },
                            "edges": []
                        }
                    }
                }
            }
        })))
        .mount(&server)
        .await;
    let provider = GithubProvider::with_base_uri("tok".into(), &server.uri()).unwrap();

    let page = provider
        .fetch_history_page(&repo(), &request(Some("CUR1")))
        .await
        .unwrap();

    assert!(page.complete);
    assert_eq!(page.next_cursor, None);
    assert_eq!(page.resume_cursor.as_deref(), Some("CUR_END"));
}

#[tokio::test]
async fn malformed_tracked_event_reports_issue_cursor_and_stage() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "repository": {
                    "id": "R_repo",
                    "issue": {
                        "id": "I_78",
                        "timelineItems": {
                            "pageInfo": { "hasNextPage": false, "endCursor": null },
                            "edges": [{
                                "cursor": "EDGE1",
                                "node": {
                                    "__typename": "ClosedEvent",
                                    "id": null,
                                    "createdAt": "2026-08-04T13:00:00Z"
                                }
                            }]
                        }
                    }
                }
            }
        })))
        .mount(&server)
        .await;
    let provider = GithubProvider::with_base_uri("tok".into(), &server.uri()).unwrap();

    let error = provider
        .fetch_history_page(&repo(), &request(Some("CUR1")))
        .await
        .unwrap_err();

    match error {
        ProviderError::Parse(message) => {
            assert!(message.contains("issue #78"));
            assert!(message.contains("CUR1"));
            assert!(message.contains("normalizing lifecycle event"));
        }
        other => panic!("expected parse error, got {other:?}"),
    }
}

#[tokio::test]
async fn malformed_milestone_event_reports_issue_cursor_and_stage() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "repository": {
                    "id": "R_repo",
                    "issue": {
                        "id": "I_78",
                        "timelineItems": {
                            "pageInfo": { "hasNextPage": false, "endCursor": null },
                            "edges": [{
                                "cursor": "EDGE1",
                                "node": {
                                    "__typename": "MilestonedEvent",
                                    "id": "E_milestone",
                                    "createdAt": "2026-08-04T13:00:00Z",
                                    "milestoneTitle": null
                                }
                            }]
                        }
                    }
                }
            }
        })))
        .mount(&server)
        .await;
    let provider = GithubProvider::with_base_uri("tok".into(), &server.uri()).unwrap();

    let error = provider
        .fetch_history_page(&repo(), &request(Some("CUR1")))
        .await
        .unwrap_err();

    match error {
        ProviderError::Parse(message) => {
            assert!(message.contains("issue #78"));
            assert!(message.contains("CUR1"));
            assert!(message.contains("normalizing milestone event"));
        }
        other => panic!("expected parse error, got {other:?}"),
    }
}

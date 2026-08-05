use stellr_core::{
    HistoryEvent, HistoryEventKind, HistoryImportState, HistorySummary, IssueSyncMetadata,
    MilestoneRef, Model, Provider, ProviderError, ProviderSnapshot, RawIssue, RepoRef, SpaceModel,
};

struct CurrentOnlyProvider;

#[async_trait::async_trait]
impl Provider for CurrentOnlyProvider {
    async fn fetch(&self, _repo: &RepoRef) -> Result<Vec<RawIssue>, ProviderError> {
        Ok(Vec::new())
    }
}

#[tokio::test]
async fn current_only_providers_get_a_backward_compatible_snapshot() {
    let snapshot = CurrentOnlyProvider
        .fetch_snapshot(&RepoRef {
            owner: "octocat".into(),
            name: "hello".into(),
        })
        .await
        .unwrap();

    assert_eq!(
        snapshot,
        ProviderSnapshot {
            repository_id: None,
            issues: Vec::new(),
            history: Vec::new(),
        }
    );
}

#[test]
fn creation_events_serialize_with_minimal_provider_evidence() {
    let event = HistoryEvent {
        sequence: 0,
        repository_id: "R_repo".into(),
        issue_id: "I_issue".into(),
        issue_number: 78,
        provider_event_id: HistoryEvent::creation_id("I_issue"),
        occurred_at: 1_754_300_000,
        kind: HistoryEventKind::IssueCreated {
            milestone: Some(MilestoneRef {
                id: Some("M_v1".into()),
                title: "v1".into(),
            }),
        },
    };

    assert_eq!(event.provider_event_id, "I_issue:issue_created");
    assert_eq!(
        serde_json::to_value(&event).unwrap(),
        serde_json::json!({
            "sequence": 0,
            "repository_id": "R_repo",
            "issue_id": "I_issue",
            "issue_number": 78,
            "provider_event_id": "I_issue:issue_created",
            "occurred_at": 1_754_300_000,
            "kind": "issue_created",
            "milestone": { "id": "M_v1", "title": "v1" }
        })
    );
}

#[test]
fn events_have_a_stable_timestamp_then_provider_id_order() {
    let event = |provider_event_id: &str, occurred_at| HistoryEvent {
        sequence: 0,
        repository_id: "R_repo".into(),
        issue_id: "I_issue".into(),
        issue_number: 78,
        provider_event_id: provider_event_id.into(),
        occurred_at,
        kind: HistoryEventKind::IssueClosed,
    };
    let mut events = [event("z", 4), event("b", 3), event("a", 3)];

    events.sort();

    assert_eq!(
        events
            .iter()
            .map(|event| (event.occurred_at, event.provider_event_id.as_str()))
            .collect::<Vec<_>>(),
        vec![(3, "a"), (3, "b"), (4, "z")]
    );
}

#[test]
fn old_models_default_to_unavailable_history() {
    let old = serde_json::json!({
        "spaces": [{
            "id": "abc",
            "repo": "octocat/hello",
            "name": "hello",
            "stars": [],
            "synced_at": null,
            "stale": false,
            "error": null
        }]
    });

    let model: Model = serde_json::from_value(old).unwrap();

    assert_eq!(model.spaces[0].history, HistorySummary::default());
    assert_eq!(
        model.spaces[0].history.state,
        HistoryImportState::Unavailable
    );
}

#[test]
fn history_metadata_stays_out_of_the_current_issue_shape() {
    let metadata = IssueSyncMetadata {
        issue_id: "I_issue".into(),
        number: 78,
        created_at: 1_754_300_000,
        updated_at: 1_754_300_100,
        milestone: None,
    };

    assert_eq!(metadata.number, 78);
    assert_eq!(metadata.milestone, None);

    let _space_shape = SpaceModel {
        id: "abc".into(),
        repo: "octocat/hello".into(),
        name: "hello".into(),
        stars: Vec::new(),
        synced_at: None,
        stale: false,
        error: None,
        history: HistorySummary::default(),
    };
}

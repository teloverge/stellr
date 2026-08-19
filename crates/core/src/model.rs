use serde::{Deserialize, Serialize};

use crate::HistorySummary;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Blocked,
    Frontier,
    Claimed,
    Resolved,
    OutOfScope,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Star {
    pub number: u64,
    #[serde(default)]
    pub parent_issue: Option<u64>,
    pub title: String,
    pub status: Status,
    #[serde(default)]
    pub ready_for_agent: bool,
    #[serde(default)]
    pub blocked: bool,
    pub blocked_by: Vec<u64>,
    pub milestone: Option<String>,
    pub labels: Vec<String>,
    pub assignees: Vec<String>,
    pub url: String,
    pub body: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueState {
    Open,
    Closed,
    ClosedNotPlanned,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RawIssue {
    pub number: u64,
    #[serde(default)]
    pub parent_issue: Option<u64>,
    pub title: String,
    pub body: String,
    pub state: IssueState,
    pub assignees: Vec<String>,
    pub milestone: Option<String>,
    pub labels: Vec<String>,
    pub blocked_by: Vec<u64>,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpaceModel {
    pub id: String,
    pub repo: String,
    pub name: String,
    #[serde(default)]
    pub viewer_login: Option<String>,
    pub stars: Vec<Star>,
    pub synced_at: Option<i64>,
    pub stale: bool,
    pub error: Option<String>,
    #[serde(default)]
    pub history: HistorySummary,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Model {
    pub spaces: Vec<SpaceModel>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_serializes_snake_case() {
        assert_eq!(
            serde_json::to_string(&Status::OutOfScope).unwrap(),
            "\"out_of_scope\""
        );
    }

    #[test]
    fn model_round_trips() {
        let model = Model {
            spaces: vec![SpaceModel {
                id: "abc".into(),
                repo: "octocat/hello".into(),
                name: "hello".into(),
                viewer_login: Some("octocat".into()),
                stars: vec![Star {
                    number: 7,
                    parent_issue: Some(16),
                    title: "Fix login".into(),
                    status: Status::Frontier,
                    ready_for_agent: false,
                    blocked: false,
                    blocked_by: vec![],
                    milestone: Some("v1".into()),
                    labels: vec!["research".into()],
                    assignees: vec![],
                    url: "https://github.com/octocat/hello/issues/7".into(),
                    body: "…".into(),
                }],
                synced_at: Some(1_753_000_000),
                stale: false,
                error: None,
                history: HistorySummary::default(),
            }],
        };

        let json = serde_json::to_string(&model).unwrap();
        let round_tripped: Model = serde_json::from_str(&json).unwrap();

        assert_eq!(round_tripped, model);
        assert_eq!(
            round_tripped.spaces[0].viewer_login.as_deref(),
            Some("octocat")
        );
    }

    #[test]
    fn old_models_without_parent_issue_deserialize_as_none() {
        let old_model = r#"
        {
          "spaces": [{
            "id": "abc",
            "repo": "octocat/hello",
            "name": "hello",
            "stars": [{
              "number": 7,
              "title": "Fix login",
              "status": "frontier",
              "blocked_by": [],
              "milestone": null,
              "labels": [],
              "assignees": [],
              "url": "https://github.com/octocat/hello/issues/7",
              "body": ""
            }],
            "synced_at": null,
            "stale": false,
            "error": null
          }]
        }
        "#;

        let model: Model = serde_json::from_str(old_model).unwrap();

        assert_eq!(model.spaces[0].viewer_login, None);
        assert_eq!(model.spaces[0].stars[0].parent_issue, None);
        assert!(!model.spaces[0].stars[0].blocked);
    }
}

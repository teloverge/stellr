use std::collections::HashMap;

use octocrab::Octocrab;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use stellr_core::{IssueState, Provider, ProviderError, RawIssue, RepoRef};

use crate::textref;

const DEFAULT_BASE_URI: &str = "https://api.github.com";

const FETCH_ISSUES_QUERY: &str = r#"
query FetchIssues($owner: String!, $name: String!, $cursor: String) {
  repository(owner: $owner, name: $name) {
    issues(first: 100, after: $cursor, states: [OPEN, CLOSED]) {
      pageInfo {
        hasNextPage
        endCursor
      }
      nodes {
        number
        title
        body
        url
        state
        stateReason
        assignees(first: 10) {
          nodes { login }
        }
        milestone { title }
        labels(first: 20) {
          nodes { name }
        }
        blockedBy(first: 50) {
          nodes {
            ... on Issue { number }
          }
        }
      }
    }
  }
}
"#;

pub struct GithubProvider {
    client: Octocrab,
}

impl GithubProvider {
    pub fn new(token: String) -> Result<Self, ProviderError> {
        Self::with_base_uri(token, DEFAULT_BASE_URI)
    }

    pub fn with_base_uri(token: String, base: &str) -> Result<Self, ProviderError> {
        let client = Octocrab::builder()
            .personal_token(token)
            .base_uri(base)
            .map_err(|error| ProviderError::Http(error.to_string()))?;
        let client = client
            .build()
            .map_err(|error| ProviderError::Http(error.to_string()))?;

        Ok(Self { client })
    }
}

#[async_trait::async_trait]
impl Provider for GithubProvider {
    async fn fetch(&self, repo: &RepoRef) -> Result<Vec<RawIssue>, ProviderError> {
        let request = GraphqlRequest {
            query: FETCH_ISSUES_QUERY,
            variables: Variables {
                owner: &repo.owner,
                name: &repo.name,
                cursor: None,
            },
        };

        let response: Value = self
            .client
            .post("/graphql", Some(&request))
            .await
            .map_err(map_octocrab_error)?;

        let response: GraphqlEnvelope = serde_json::from_value(response)
            .map_err(|error| ProviderError::Parse(error.to_string()))?;

        if let Some(error) = response.errors.and_then(|errors| errors.into_iter().next()) {
            return Err(ProviderError::Parse(error.message));
        }

        let nodes = response
            .data
            .ok_or_else(|| ProviderError::Parse("missing data.repository.issues".into()))
            .and_then(|data| {
                serde_json::from_value::<GraphqlData>(data)
                    .map_err(|error| ProviderError::Parse(error.to_string()))
            })?
            .repository
            .map(|repository| repository.issues.nodes)
            .ok_or_else(|| ProviderError::Parse("missing data.repository.issues".into()))?;

        Ok(map_issues(nodes))
    }
}

fn map_octocrab_error(error: octocrab::Error) -> ProviderError {
    match error {
        error @ octocrab::Error::Json { .. } => ProviderError::Parse(error.to_string()),
        error => ProviderError::Http(error.to_string()),
    }
}

fn map_issues(nodes: Vec<IssueNode>) -> Vec<RawIssue> {
    let mut issues = Vec::with_capacity(nodes.len());
    let mut inversions = Vec::new();

    for node in nodes {
        let body = node.body.unwrap_or_default();
        let refs = textref::scan(&body);
        let mut blocked_by = node
            .blocked_by
            .nodes
            .into_iter()
            .map(|issue| issue.number)
            .chain(refs.blocked_by)
            .collect::<Vec<_>>();
        blocked_by.sort_unstable();
        blocked_by.dedup();

        inversions.extend(refs.blocks.into_iter().map(|target| (node.number, target)));
        issues.push(RawIssue {
            number: node.number,
            title: node.title,
            body,
            state: match node.state {
                GithubIssueState::Open => IssueState::Open,
                GithubIssueState::Closed if node.state_reason.as_deref() == Some("NOT_PLANNED") => {
                    IssueState::ClosedNotPlanned
                }
                GithubIssueState::Closed => IssueState::Closed,
            },
            assignees: node
                .assignees
                .nodes
                .into_iter()
                .map(|assignee| assignee.login)
                .collect(),
            milestone: node.milestone.map(|milestone| milestone.title),
            labels: node
                .labels
                .nodes
                .into_iter()
                .map(|label| label.name)
                .collect(),
            blocked_by,
            url: node.url,
        });
    }

    let positions = issues
        .iter()
        .enumerate()
        .map(|(index, issue)| (issue.number, index))
        .collect::<HashMap<_, _>>();
    for (blocker, target) in inversions {
        if let Some(&index) = positions.get(&target) {
            issues[index].blocked_by.push(blocker);
        }
    }
    for issue in &mut issues {
        issue.blocked_by.sort_unstable();
        issue.blocked_by.dedup();
    }

    issues
}

#[derive(Serialize)]
struct GraphqlRequest<'a> {
    query: &'static str,
    variables: Variables<'a>,
}

#[derive(Serialize)]
struct Variables<'a> {
    owner: &'a str,
    name: &'a str,
    cursor: Option<&'a str>,
}

#[derive(Deserialize)]
struct GraphqlEnvelope {
    data: Option<Value>,
    errors: Option<Vec<GraphqlError>>,
}

#[derive(Deserialize)]
struct GraphqlError {
    message: String,
}

#[derive(Deserialize)]
struct GraphqlData {
    repository: Option<Repository>,
}

#[derive(Deserialize)]
struct Repository {
    issues: IssueConnection,
}

#[derive(Deserialize)]
struct IssueConnection {
    nodes: Vec<IssueNode>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct IssueNode {
    number: u64,
    title: String,
    body: Option<String>,
    url: String,
    state: GithubIssueState,
    state_reason: Option<String>,
    assignees: AssigneeConnection,
    milestone: Option<Milestone>,
    labels: LabelConnection,
    blocked_by: BlockedByConnection,
}

#[derive(Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum GithubIssueState {
    Open,
    Closed,
}

#[derive(Deserialize)]
struct AssigneeConnection {
    nodes: Vec<Assignee>,
}

#[derive(Deserialize)]
struct Assignee {
    login: String,
}

#[derive(Deserialize)]
struct Milestone {
    title: String,
}

#[derive(Deserialize)]
struct LabelConnection {
    nodes: Vec<Label>,
}

#[derive(Deserialize)]
struct Label {
    name: String,
}

#[derive(Deserialize)]
struct BlockedByConnection {
    nodes: Vec<BlockedByIssue>,
}

#[derive(Deserialize)]
struct BlockedByIssue {
    number: u64,
}

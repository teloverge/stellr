use octocrab::{FromResponse, Octocrab};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use stellr_core::{IssueState, Provider, ProviderError, ProviderSnapshot, RawIssue, RepoRef};

use crate::textref;

const DEFAULT_BASE_URI: &str = "https://api.github.com";

const FETCH_ISSUES_QUERY: &str = r#"
query FetchIssues($owner: String!, $name: String!, $cursor: String) {
  viewer { login }
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
        parent { number }
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

/// Shared authenticated GraphQL transport with Stellr's typed error mapping.
#[derive(Clone)]
pub struct GithubGraphqlClient {
    client: Octocrab,
}

impl GithubGraphqlClient {
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

    pub async fn post_value<T: Serialize + ?Sized>(
        &self,
        request: &T,
    ) -> Result<Value, ProviderError> {
        let response = self
            .client
            ._post("/graphql", Some(request))
            .await
            .map_err(map_octocrab_error)?;
        if response.status().as_u16() == 401 {
            return Err(ProviderError::Auth("token rejected".into()));
        }
        let status = response.status().as_u16();
        let rate_limit_exhausted = response
            .headers()
            .get("x-ratelimit-remaining")
            .and_then(|value| value.to_str().ok())
            == Some("0");
        if matches!(status, 403 | 429) && rate_limit_exhausted {
            let reset_epoch = response
                .headers()
                .get("x-ratelimit-reset")
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse().ok());
            return Err(ProviderError::RateLimited { reset_epoch });
        }
        let response = octocrab::map_github_error(response)
            .await
            .map_err(map_octocrab_error)?;
        let value = Value::from_response(response)
            .await
            .map_err(map_octocrab_error)?;
        if let Some(errors) = value.get("errors") {
            let errors = errors.as_array().ok_or_else(|| {
                ProviderError::Parse("malformed GraphQL error response".to_owned())
            })?;
            if !errors.is_empty() {
                let messages = errors
                    .iter()
                    .map(|error| error.get("message").and_then(Value::as_str))
                    .collect::<Option<Vec<_>>>()
                    .ok_or_else(|| {
                        ProviderError::Parse("malformed GraphQL error response".to_owned())
                    })?;
                return Err(ProviderError::Parse(messages[0].to_owned()));
            }
        }
        Ok(value)
    }
}

pub struct GithubProvider {
    client: GithubGraphqlClient,
}

impl GithubProvider {
    pub fn new(token: String) -> Result<Self, ProviderError> {
        Ok(Self {
            client: GithubGraphqlClient::new(token)?,
        })
    }

    pub fn with_base_uri(token: String, base: &str) -> Result<Self, ProviderError> {
        Ok(Self {
            client: GithubGraphqlClient::with_base_uri(token, base)?,
        })
    }
}

#[async_trait::async_trait]
impl Provider for GithubProvider {
    async fn fetch(&self, repo: &RepoRef) -> Result<ProviderSnapshot, ProviderError> {
        let mut cursor = None;
        let mut nodes = Vec::new();
        let mut viewer_login: Option<String> = None;

        loop {
            let request = GraphqlRequest {
                query: FETCH_ISSUES_QUERY,
                variables: Variables {
                    owner: &repo.owner,
                    name: &repo.name,
                    cursor: cursor.as_deref(),
                },
            };

            let response = self.client.post_value(&request).await?;

            let response: GraphqlEnvelope = serde_json::from_value(response)
                .map_err(|error| ProviderError::Parse(error.to_string()))?;

            let data = response
                .data
                .ok_or_else(|| ProviderError::Parse("missing data.repository.issues".into()))
                .and_then(|data| {
                    serde_json::from_value::<GraphqlData>(data)
                        .map_err(|error| ProviderError::Parse(error.to_string()))
                })?;

            if data.viewer.login.trim().is_empty() {
                return Err(ProviderError::Parse("viewer login is empty".into()));
            }

            match viewer_login.as_deref() {
                Some(login) if login != data.viewer.login => {
                    return Err(ProviderError::Parse(
                        "viewer login changed during issue pagination".into(),
                    ));
                }
                Some(_) => {}
                None => viewer_login = Some(data.viewer.login.clone()),
            }

            let connection = data
                .repository
                .map(|repository| repository.issues)
                .ok_or_else(|| ProviderError::Parse("missing data.repository.issues".into()))?;

            nodes.extend(connection.nodes);
            if !connection.page_info.has_next_page {
                break;
            }
            cursor = Some(connection.page_info.end_cursor.ok_or_else(|| {
                ProviderError::Parse("missing end cursor for next issues page".into())
            })?);
        }

        let mut issues = map_issues(nodes);
        issues.sort_by_key(|issue| issue.number);
        Ok(ProviderSnapshot::new(viewer_login, issues))
    }
}

fn map_octocrab_error(error: octocrab::Error) -> ProviderError {
    match error {
        error @ octocrab::Error::Json { .. } => ProviderError::Parse(error.to_string()),
        error => ProviderError::Http(error.to_string()),
    }
}

fn map_issues(nodes: Vec<IssueNode>) -> Vec<RawIssue> {
    let mut issues = nodes
        .into_iter()
        .map(|node| {
            let body = node.body.unwrap_or_default();
            let blocked_by = node
                .blocked_by
                .nodes
                .into_iter()
                .map(|issue| issue.number)
                .collect::<Vec<_>>();
            RawIssue {
                number: node.number,
                parent_issue: node.parent.map(|parent| parent.number),
                title: node.title,
                body,
                state: match node.state {
                    GithubIssueState::Open => IssueState::Open,
                    GithubIssueState::Closed
                        if node.state_reason.as_deref() == Some("NOT_PLANNED") =>
                    {
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
            }
        })
        .collect::<Vec<_>>();

    textref::enrich_relationships(&mut issues);

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
}

#[derive(Deserialize)]
struct GraphqlData {
    viewer: Viewer,
    repository: Option<Repository>,
}

#[derive(Deserialize)]
struct Viewer {
    login: String,
}

#[derive(Deserialize)]
struct Repository {
    issues: IssueConnection,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct IssueConnection {
    page_info: PageInfo,
    nodes: Vec<IssueNode>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PageInfo {
    has_next_page: bool,
    end_cursor: Option<String>,
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
    parent: Option<ParentIssue>,
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
struct ParentIssue {
    number: u64,
}

#[derive(Deserialize)]
struct BlockedByConnection {
    nodes: Vec<BlockedByIssue>,
}

#[derive(Deserialize)]
struct BlockedByIssue {
    number: u64,
}

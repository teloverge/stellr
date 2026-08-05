use chrono::DateTime;
use serde::{Deserialize, Serialize};
use stellr_core::{
    HistoryEvent, HistoryEventKind, HistoryPage, HistoryPageRequest, MilestoneRef, ProviderError,
    RepoRef,
};

use crate::sync::GithubGraphqlClient;

const FETCH_HISTORY_PAGE_QUERY: &str = r#"
query FetchIssueHistory(
  $owner: String!
  $name: String!
  $number: Int!
  $cursor: String
) {
  repository(owner: $owner, name: $name) {
    id
    issue(number: $number) {
      id
      timelineItems(
        first: 100
        after: $cursor
        itemTypes: [CLOSED_EVENT, REOPENED_EVENT, DEMILESTONED_EVENT, MILESTONED_EVENT]
      ) {
        pageInfo {
          hasNextPage
          endCursor
        }
        nodes {
          __typename
          ... on ClosedEvent { id createdAt }
          ... on ReopenedEvent { id createdAt }
          ... on DemilestonedEvent { id createdAt milestoneTitle }
          ... on MilestonedEvent { id createdAt milestoneTitle }
        }
      }
    }
  }
}
"#;

pub(crate) async fn fetch_history_page(
    client: &GithubGraphqlClient,
    repo: &RepoRef,
    request: &HistoryPageRequest,
) -> Result<HistoryPage, ProviderError> {
    let body = GraphqlRequest {
        query: FETCH_HISTORY_PAGE_QUERY,
        variables: Variables {
            owner: &repo.owner,
            name: &repo.name,
            number: request.issue_number,
            cursor: request.cursor.as_deref(),
        },
    };
    let response = client.post_value(&body).await?;
    let envelope: GraphqlEnvelope = serde_json::from_value(response)
        .map_err(|error| contextual_parse(request, "decoding history page", error))?;
    let repository = envelope
        .data
        .and_then(|data| data.repository)
        .ok_or_else(|| {
            contextual_message(request, "decoding history page", "missing repository")
        })?;
    let issue = repository.issue.ok_or_else(|| {
        contextual_message(request, "decoding history page", "missing repository issue")
    })?;
    if issue.id != request.issue_id {
        return Err(contextual_message(
            request,
            "validating history issue",
            "provider issue identity did not match the snapshot",
        ));
    }

    let mut events = Vec::new();
    for node in issue.timeline_items.nodes {
        let kind = match node.typename.as_str() {
            "ClosedEvent" => HistoryEventKind::IssueClosed,
            "ReopenedEvent" => HistoryEventKind::IssueReopened,
            "DemilestonedEvent" => HistoryEventKind::MilestoneChanged {
                from: Some(historical_milestone(request, &node)?),
                to: None,
            },
            "MilestonedEvent" => HistoryEventKind::MilestoneChanged {
                from: None,
                to: Some(historical_milestone(request, &node)?),
            },
            _ => continue,
        };
        let provider_event_id = node.id.ok_or_else(|| {
            contextual_message(
                request,
                "normalizing lifecycle event",
                "tracked event is missing its provider identity",
            )
        })?;
        let occurred_at = node
            .created_at
            .as_deref()
            .ok_or_else(|| {
                contextual_message(
                    request,
                    "normalizing lifecycle event",
                    "tracked event is missing its timestamp",
                )
            })
            .and_then(|timestamp| {
                DateTime::parse_from_rfc3339(timestamp)
                    .map(|timestamp| timestamp.timestamp())
                    .map_err(|error| {
                        contextual_parse(request, "normalizing lifecycle event", error)
                    })
            })?;
        if occurred_at <= request.cutoff {
            events.push(HistoryEvent {
                sequence: 0,
                repository_id: repository.id.clone(),
                issue_id: issue.id.clone(),
                issue_number: request.issue_number,
                provider_event_id,
                occurred_at,
                kind,
            });
        }
    }
    events.sort();

    let page_info = issue.timeline_items.page_info;
    let resume_cursor = page_info
        .end_cursor
        .clone()
        .or_else(|| request.cursor.clone());
    let next_cursor = if page_info.has_next_page {
        Some(page_info.end_cursor.clone().ok_or_else(|| {
            contextual_message(
                request,
                "advancing history page",
                "missing end cursor for next page",
            )
        })?)
    } else {
        None
    };
    Ok(HistoryPage {
        events,
        next_cursor,
        resume_cursor,
        complete: !page_info.has_next_page,
    })
}

fn historical_milestone(
    request: &HistoryPageRequest,
    node: &TimelineNode,
) -> Result<MilestoneRef, ProviderError> {
    let title = node.milestone_title.clone().ok_or_else(|| {
        contextual_message(
            request,
            "normalizing milestone event",
            "tracked event is missing its milestone title",
        )
    })?;
    Ok(MilestoneRef { id: None, title })
}

fn contextual_parse(
    request: &HistoryPageRequest,
    stage: &str,
    error: impl std::fmt::Display,
) -> ProviderError {
    contextual_message(request, stage, &error.to_string())
}

fn contextual_message(request: &HistoryPageRequest, stage: &str, message: &str) -> ProviderError {
    ProviderError::Parse(format!(
        "issue #{} at cursor {} while {stage}: {message}",
        request.issue_number,
        request.cursor.as_deref().unwrap_or("<start>")
    ))
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
    number: u64,
    cursor: Option<&'a str>,
}

#[derive(Deserialize)]
struct GraphqlEnvelope {
    data: Option<GraphqlData>,
}

#[derive(Deserialize)]
struct GraphqlData {
    repository: Option<Repository>,
}

#[derive(Deserialize)]
struct Repository {
    id: String,
    issue: Option<Issue>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Issue {
    id: String,
    timeline_items: TimelineConnection,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TimelineConnection {
    page_info: PageInfo,
    nodes: Vec<TimelineNode>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PageInfo {
    has_next_page: bool,
    end_cursor: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TimelineNode {
    #[serde(rename = "__typename")]
    typename: String,
    id: Option<String>,
    created_at: Option<String>,
    milestone_title: Option<String>,
}

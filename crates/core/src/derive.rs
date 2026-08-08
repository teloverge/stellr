use crate::model::{IssueState, RawIssue, Star, Status};
use std::collections::BTreeMap;

pub fn derive(issues: &[RawIssue]) -> Vec<Star> {
    let states: BTreeMap<u64, IssueState> = issues
        .iter()
        .map(|issue| (issue.number, issue.state))
        .collect();
    let mut stars: Vec<Star> = issues
        .iter()
        .map(|issue| {
            let mut blocked_by: Vec<u64> = issue
                .blocked_by
                .iter()
                .copied()
                .filter(|number| *number != issue.number && states.contains_key(number))
                .collect();
            blocked_by.sort_unstable();
            blocked_by.dedup();

            let has_open_blocker = blocked_by
                .iter()
                .any(|number| states[number] == IssueState::Open);
            let status = match issue.state {
                IssueState::ClosedNotPlanned => Status::OutOfScope,
                IssueState::Closed => Status::Resolved,
                IssueState::Open if !issue.assignees.is_empty() => Status::Claimed,
                IssueState::Open if has_open_blocker => Status::Blocked,
                IssueState::Open => Status::Frontier,
            };
            let ready_for_agent = issue.state == IssueState::Open
                && !has_open_blocker
                && issue
                    .labels
                    .iter()
                    .any(|label| label.eq_ignore_ascii_case("ready-for-agent"));

            Star {
                number: issue.number,
                parent_issue: issue.parent_issue.filter(|parent| *parent != issue.number),
                title: issue.title.clone(),
                status,
                ready_for_agent,
                blocked: has_open_blocker,
                blocked_by,
                milestone: issue.milestone.clone(),
                labels: issue.labels.clone(),
                assignees: issue.assignees.clone(),
                url: issue.url.clone(),
                body: issue.body.clone(),
            }
        })
        .collect();
    stars.sort_unstable_by_key(|star| star.number);
    stars
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;

    fn issue(
        number: u64,
        state: IssueState,
        assignees: &[&str],
        blocked_by: &[u64],
        parent_issue: Option<u64>,
    ) -> RawIssue {
        RawIssue {
            number,
            parent_issue,
            title: format!("issue {number}"),
            body: String::new(),
            state,
            assignees: assignees
                .iter()
                .map(|assignee| assignee.to_string())
                .collect(),
            milestone: None,
            labels: vec![],
            blocked_by: blocked_by.to_vec(),
            url: format!("https://github.com/o/r/issues/{number}"),
        }
    }

    fn status_of(stars: &[Star], number: u64) -> Status {
        stars
            .iter()
            .find(|star| star.number == number)
            .unwrap()
            .status
    }

    #[test]
    fn derives_statuses_in_precedence_order() {
        let stars = derive(&[
            issue(1, IssueState::Closed, &[], &[], None),
            issue(2, IssueState::ClosedNotPlanned, &[], &[], None),
            issue(3, IssueState::Open, &["me"], &[4], None),
            issue(4, IssueState::Open, &[], &[5], None),
            issue(5, IssueState::Open, &[], &[1, 2], None),
        ]);

        assert_eq!(status_of(&stars, 1), Status::Resolved);
        assert_eq!(status_of(&stars, 2), Status::OutOfScope);
        assert_eq!(status_of(&stars, 3), Status::Claimed);
        assert_eq!(status_of(&stars, 4), Status::Blocked);
        assert_eq!(status_of(&stars, 5), Status::Frontier);
    }

    #[test]
    fn keeps_claimed_compatibility_while_exposing_agent_readiness() {
        let mut ready_claimed = issue(1, IssueState::Open, &["octocat"], &[], None);
        ready_claimed.labels = vec!["READY-FOR-AGENT".into()];
        let mut blocked_claimed = issue(2, IssueState::Open, &["octocat"], &[3], None);
        blocked_claimed.labels = vec!["ready-for-agent".into()];
        let blocker = issue(3, IssueState::Open, &[], &[], None);

        let stars = derive(&[ready_claimed, blocked_claimed, blocker]);

        assert_eq!(status_of(&stars, 1), Status::Claimed);
        assert!(
            stars
                .iter()
                .find(|star| star.number == 1)
                .unwrap()
                .ready_for_agent
        );
        assert_eq!(status_of(&stars, 2), Status::Claimed);
        assert!(
            !stars
                .iter()
                .find(|star| star.number == 2)
                .unwrap()
                .ready_for_agent
        );
        assert!(!stars.iter().find(|star| star.number == 1).unwrap().blocked);
        assert!(stars.iter().find(|star| star.number == 2).unwrap().blocked);
    }

    #[test]
    fn removes_self_unknown_and_duplicate_blocker_references() {
        let stars = derive(&[
            issue(1, IssueState::Open, &[], &[1, 99, 2, 2], None),
            issue(2, IssueState::Open, &[], &[], None),
        ]);

        assert_eq!(stars[0].blocked_by, vec![2]);
        assert_eq!(status_of(&stars, 1), Status::Blocked);
    }

    #[test]
    fn copies_parent_identity_without_affecting_blocker_status() {
        let stars = derive(&[
            issue(16, IssueState::Open, &[], &[], None),
            issue(39, IssueState::Open, &[], &[], Some(16)),
            issue(40, IssueState::Open, &[], &[39], Some(40)),
        ]);
        assert_eq!(stars[1].parent_issue, Some(16));
        assert_eq!(stars[2].parent_issue, None);
        assert_eq!(stars[2].status, Status::Blocked);
    }

    #[test]
    fn classifies_mutual_open_blockers_as_blocked() {
        let stars = derive(&[
            issue(1, IssueState::Open, &[], &[2], None),
            issue(2, IssueState::Open, &[], &[1], None),
        ]);

        assert_eq!(status_of(&stars, 1), Status::Blocked);
        assert_eq!(status_of(&stars, 2), Status::Blocked);
    }

    #[test]
    fn sorts_output_by_issue_number() {
        let stars = derive(&[
            issue(9, IssueState::Open, &[], &[], None),
            issue(1, IssueState::Open, &[], &[], None),
        ]);

        assert_eq!(
            stars.iter().map(|star| star.number).collect::<Vec<_>>(),
            vec![1, 9]
        );
    }

    #[test]
    fn closing_a_blocker_never_shrinks_the_frontier() {
        let mut seed = 0x2545_F491_4F6C_DD1D_u64;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };

        for _ in 0..200 {
            let issue_count = 2 + (next() % 12);
            let mut issues: Vec<RawIssue> = (1..=issue_count)
                .map(|number| {
                    let blockers: Vec<u64> = (1..number).filter(|_| next() % 3 == 0).collect();
                    let state = if next() % 4 == 0 {
                        IssueState::Closed
                    } else {
                        IssueState::Open
                    };
                    issue(number, state, &[], &blockers, None)
                })
                .collect();
            let frontier_before: Vec<u64> = derive(&issues)
                .iter()
                .filter(|star| star.status == Status::Frontier)
                .map(|star| star.number)
                .collect();

            if let Some(open_issue) = issues
                .iter_mut()
                .find(|issue| issue.state == IssueState::Open)
            {
                open_issue.state = IssueState::Closed;
                let closed_number = open_issue.number;
                let after = derive(&issues);

                for number in frontier_before
                    .iter()
                    .filter(|number| **number != closed_number)
                {
                    assert_eq!(
                        status_of(&after, *number),
                        Status::Frontier,
                        "frontier lost star {number}"
                    );
                }
            }
        }
    }
}

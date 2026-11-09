use git_viewme_core::{
    Issue, IssueCache, IssueState, PullRequestCache, PullRequestState, PullRequestStatus,
    issue_changes_from_pages, issues_from_pages, pull_request_changes_from_pages,
    pull_requests_from_pages,
};
use serde_json::{Value, json};

const SINCE: &str = "2026-11-08T12:00:00Z";

fn issue(number: u32, state: &str, updated_at: &str) -> Value {
    json!({
        "html_url": format!("https://github.com/sadermine/git-viewme/issues/{number}"),
        "number": number,
        "title": format!("Issue {number}"),
        "user": { "login": "sader-agents" },
        "labels": [],
        "state": state,
        "updated_at": updated_at,
    })
}

fn pull(number: u32, state: &str, merged_at: Option<&str>, updated_at: &str) -> Value {
    json!({
        "html_url": format!("https://github.com/sadermine/git-viewme/pull/{number}"),
        "number": number,
        "title": format!("Pull request {number}"),
        "user": { "login": "sader-agents" },
        "labels": [],
        "state": state,
        "draft": false,
        "merged_at": merged_at,
        "updated_at": updated_at,
        "head": { "ref": format!("card/{number}") },
        "base": { "ref": "main" },
    })
}

fn page(nodes: &[Value]) -> String {
    Value::Array(nodes.to_vec()).to_string()
}

fn numbers(issues: Option<Vec<Issue>>) -> Option<Vec<u32>> {
    issues.map(|issues| issues.iter().map(|issue| issue.number).collect())
}

/// Open #1 to #3 and closed #4, as two full fetches.
fn cached_issues() -> IssueCache {
    let open = page(&[
        issue(3, "open", "2026-11-07T10:00:00Z"),
        issue(2, "open", "2026-11-06T10:00:00Z"),
        issue(1, "open", "2026-11-05T10:00:00Z"),
    ]);
    let closed = page(&[issue(4, "closed", "2026-11-04T10:00:00Z")]);
    let mut cache = IssueCache::default();
    cache.replace(
        IssueState::Open,
        issues_from_pages([open.as_str()]).unwrap(),
        SINCE.into(),
    );
    cache.replace(
        IssueState::Closed,
        issues_from_pages([closed.as_str()]).unwrap(),
        SINCE.into(),
    );
    cache
}

#[test]
fn merge_moves_a_closed_issue_from_open_to_closed_and_adds_new_and_updated_ones() {
    let mut cache = cached_issues();
    let mut pull_request = issue(6, "open", "2026-11-08T12:01:30Z");
    pull_request["pull_request"] = json!({ "merged_at": null });
    let changes = page(&[
        issue(5, "open", "2026-11-08T12:03:00Z"),
        issue(2, "closed", "2026-11-08T12:02:00Z"),
        pull_request,
        issue(3, "open", "2026-11-08T12:01:00Z"),
    ]);

    let changed = cache.merge(issue_changes_from_pages([changes.as_str()]).unwrap());

    assert!(changed);
    assert_eq!(numbers(cache.list(IssueState::Open)), Some(vec![5, 3, 1]));
    assert_eq!(numbers(cache.list(IssueState::Closed)), Some(vec![2, 4]));
}

#[test]
fn merge_of_issues_already_held_changes_nothing() {
    let mut cache = cached_issues();
    let overlap = page(&[
        issue(3, "open", "2026-11-07T10:00:00Z"),
        issue(4, "closed", "2026-11-04T10:00:00Z"),
    ]);

    let changed = cache.merge(issue_changes_from_pages([overlap.as_str()]).unwrap());

    assert!(!changed);
    assert_eq!(numbers(cache.list(IssueState::Open)), Some(vec![3, 2, 1]));
}

#[test]
fn merge_drops_an_issue_closed_since_when_closed_was_never_fetched() {
    let open = page(&[
        issue(2, "open", "2026-11-06T10:00:00Z"),
        issue(1, "open", "2026-11-05T10:00:00Z"),
    ]);
    let mut cache = IssueCache::default();
    cache.replace(
        IssueState::Open,
        issues_from_pages([open.as_str()]).unwrap(),
        SINCE.into(),
    );
    let changes = page(&[issue(2, "closed", "2026-11-08T12:02:00Z")]);

    cache.merge(issue_changes_from_pages([changes.as_str()]).unwrap());

    assert_eq!(numbers(cache.list(IssueState::Open)), Some(vec![1]));
    assert_eq!(numbers(cache.list(IssueState::Closed)), None);
}

#[test]
fn since_keeps_the_earliest_full_fetch_and_drops_the_etag_when_it_moves() {
    let mut cache = IssueCache::default();
    cache.replace(IssueState::Open, vec![], "2026-11-08T12:00:00Z".into());
    cache.set_etag(Some("W/\"a\"".into()));

    cache.replace(IssueState::Closed, vec![], "2026-11-08T12:30:00Z".into());
    assert_eq!(cache.since(), Some("2026-11-08T12:00:00Z"));
    assert_eq!(cache.etag(), Some("W/\"a\""));

    cache.advance("2026-11-08T13:00:00Z".into());
    assert_eq!(cache.since(), Some("2026-11-08T13:00:00Z"));
    assert_eq!(cache.etag(), None);
}

#[test]
fn answered_moves_since_only_past_one_page_even_when_no_list_changed() {
    let mut cache = cached_issues();

    cache.answered(1, "2026-11-08T13:00:00Z".into(), Some("W/\"a\"".into()));
    assert_eq!(cache.since(), Some(SINCE));
    assert_eq!(cache.etag(), Some("W/\"a\""));

    // Two pages of pull request activity: no issue list changes, but the next refresh must not
    // read both pages again.
    let pull_requests = page(&[{
        let mut pull_request = issue(9, "open", "2026-11-08T13:30:00Z");
        pull_request["pull_request"] = json!({ "merged_at": null });
        pull_request
    }]);
    assert!(!cache.merge(
        issue_changes_from_pages([pull_requests.as_str(), pull_requests.as_str()]).unwrap()
    ));
    cache.answered(2, "2026-11-08T13:59:00Z".into(), Some("W/\"b\"".into()));
    assert_eq!(cache.since(), Some("2026-11-08T13:59:00Z"));
    assert_eq!(cache.etag(), None);
}

#[test]
fn merge_moves_a_merged_pull_request_from_open_to_closed() {
    let open = page(&[
        pull(8, "open", None, "2026-11-07T10:00:00Z"),
        pull(7, "open", None, "2026-11-06T10:00:00Z"),
    ]);
    let closed = page(&[pull(5, "closed", None, "2026-11-01T10:00:00Z")]);
    let mut cache = PullRequestCache::default();
    cache.replace(
        PullRequestState::Open,
        pull_requests_from_pages([open.as_str()]).unwrap(),
        SINCE.into(),
    );
    cache.replace(
        PullRequestState::Closed,
        pull_requests_from_pages([closed.as_str()]).unwrap(),
        SINCE.into(),
    );
    let changes = page(&[pull(
        7,
        "closed",
        Some("2026-11-08T12:05:00Z"),
        "2026-11-08T12:05:00Z",
    )]);

    cache.merge(pull_request_changes_from_pages([changes.as_str()]).unwrap());

    let open: Vec<u32> = cache
        .list(PullRequestState::Open)
        .unwrap()
        .iter()
        .map(|pull| pull.number)
        .collect();
    let closed: Vec<(u32, PullRequestStatus)> = cache
        .list(PullRequestState::Closed)
        .unwrap()
        .iter()
        .map(|pull| (pull.number, pull.status))
        .collect();
    assert_eq!(open, [8]);
    assert_eq!(
        closed,
        [
            (7, PullRequestStatus::Merged),
            (5, PullRequestStatus::Closed)
        ]
    );
}

#[test]
fn forget_drops_one_state_and_keeps_the_others() {
    let mut cache = cached_issues();

    cache.forget(IssueState::Open);

    assert_eq!(numbers(cache.list(IssueState::Open)), None);
    assert_eq!(numbers(cache.list(IssueState::Closed)), Some(vec![4]));
    assert_eq!(cache.since(), Some(SINCE));
}

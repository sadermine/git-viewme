use git_viewme_core::{Issue, IssueState, issues_from_pages};
use serde_json::{Value, json};

fn issue(number: u32, updated_at: &str) -> Value {
    json!({
        "url": format!("https://api.github.com/repos/sadermine/git-viewme/issues/{number}"),
        "html_url": format!("https://github.com/sadermine/git-viewme/issues/{number}"),
        "id": 3_000_000_000_u64 + u64::from(number),
        "number": number,
        "title": format!("Issue {number}"),
        "user": { "login": "sader-agents", "id": 1, "type": "User" },
        "labels": [{ "id": 1, "name": "feature", "color": "a2eeef", "default": false }],
        "state": "open",
        "comments": 0,
        "created_at": "2026-11-01T09:00:00Z",
        "updated_at": updated_at,
        "closed_at": null,
        "body": "### Goal\n\nSomething."
    })
}

fn pull_request(number: u32, updated_at: &str) -> Value {
    let mut node = issue(number, updated_at);
    node["html_url"] = json!(format!(
        "https://github.com/sadermine/git-viewme/pull/{number}"
    ));
    node["pull_request"] = json!({
        "url": format!("https://api.github.com/repos/sadermine/git-viewme/pulls/{number}"),
        "html_url": format!("https://github.com/sadermine/git-viewme/pull/{number}"),
        "merged_at": null
    });
    node
}

fn numbers(issues: &[Issue]) -> Vec<u32> {
    issues.iter().map(|issue| issue.number).collect()
}

#[test]
fn issues_from_pages_leaves_out_pull_requests() {
    let body = json!([
        issue(4, "2026-11-06T10:00:00Z"),
        pull_request(6, "2026-11-06T11:00:00Z"),
        issue(5, "2026-11-05T10:00:00Z")
    ])
    .to_string();

    let issues = issues_from_pages([body.as_str()]).unwrap();

    assert_eq!(numbers(&issues), [4, 5]);
}

#[test]
fn issues_from_pages_keeps_one_copy_of_an_issue_repeated_across_pages() {
    let first = json!([
        issue(3, "2026-11-06T10:00:00Z"),
        issue(2, "2026-11-05T10:00:00Z")
    ])
    .to_string();
    let second = json!([
        issue(2, "2026-11-05T10:00:00Z"),
        issue(1, "2026-11-04T10:00:00Z")
    ])
    .to_string();

    let issues = issues_from_pages([first.as_str(), second.as_str()]).unwrap();

    assert_eq!(numbers(&issues), [3, 2, 1]);
}

#[test]
fn issues_from_pages_orders_most_recently_updated_first_across_pages() {
    let first = json!([
        issue(1, "2026-11-02T08:00:00Z"),
        issue(2, "2026-11-06T08:00:00Z")
    ])
    .to_string();
    let second = json!([
        issue(3, "2026-11-04T08:00:00Z"),
        issue(7, "2026-11-04T08:00:00Z")
    ])
    .to_string();

    let issues = issues_from_pages([first.as_str(), second.as_str()]).unwrap();

    assert_eq!(numbers(&issues), [2, 7, 3, 1]);
}

#[test]
fn issues_from_pages_maps_author_labels_and_link() {
    let mut deleted_author = issue(9, "2026-11-01T08:00:00Z");
    deleted_author["user"] = Value::Null;
    deleted_author["labels"] = json!([]);
    let body = json!([issue(8, "2026-11-03T08:00:00Z"), deleted_author]).to_string();

    let issues = issues_from_pages([body.as_str()]).unwrap();

    assert_eq!(
        issues,
        [
            Issue {
                number: 8,
                title: "Issue 8".into(),
                author: "sader-agents".into(),
                labels: vec!["feature".into()],
                updated_at: "2026-11-03T08:00:00Z".into(),
                html_url: "https://github.com/sadermine/git-viewme/issues/8".into(),
            },
            Issue {
                number: 9,
                title: "Issue 9".into(),
                author: "ghost".into(),
                labels: vec![],
                updated_at: "2026-11-01T08:00:00Z".into(),
                html_url: "https://github.com/sadermine/git-viewme/issues/9".into(),
            },
        ]
    );
}

#[test]
fn issues_from_pages_empty_repository_has_no_issues() {
    assert_eq!(issues_from_pages(["[]"]).unwrap(), []);
}

#[test]
fn issues_from_pages_unexpected_body_is_an_error() {
    assert!(issues_from_pages([r#"{"message":"oops"}"#]).is_err());
}

#[test]
fn issue_state_serializes_as_the_query_value() {
    for state in [IssueState::Open, IssueState::Closed] {
        assert_eq!(serde_json::to_value(state).unwrap(), json!(state.as_str()));
    }
}

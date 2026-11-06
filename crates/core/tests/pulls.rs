use git_viewme_core::{PullRequest, PullRequestState, PullRequestStatus, pull_requests_from_pages};
use serde_json::{Value, json};

fn pull(number: u32, updated_at: &str) -> Value {
    json!({
        "url": format!("https://api.github.com/repos/sadermine/git-viewme/pulls/{number}"),
        "html_url": format!("https://github.com/sadermine/git-viewme/pull/{number}"),
        "id": 2_000_000_000_u64 + u64::from(number),
        "number": number,
        "state": "open",
        "locked": false,
        "title": format!("Pull request {number}"),
        "user": { "login": "sader-agents", "id": 1, "type": "User" },
        "body": "Closes #5",
        "labels": [{ "id": 1, "name": "feature", "color": "a2eeef", "default": false }],
        "created_at": "2026-11-01T09:00:00Z",
        "updated_at": updated_at,
        "closed_at": null,
        "merged_at": null,
        "draft": false,
        "head": {
            "label": format!("sadermine:card/{number}"),
            "ref": format!("card/{number}"),
            "sha": "0123456789abcdef0123456789abcdef01234567"
        },
        "base": {
            "label": "sadermine:main",
            "ref": "main",
            "sha": "89abcdef0123456789abcdef0123456789abcdef"
        }
    })
}

fn closed(number: u32, updated_at: &str, merged_at: Option<&str>) -> Value {
    let mut node = pull(number, updated_at);
    node["state"] = json!("closed");
    node["closed_at"] = json!(updated_at);
    node["merged_at"] = json!(merged_at);
    node
}

fn statuses(pulls: &[PullRequest]) -> Vec<(u32, PullRequestStatus)> {
    pulls
        .iter()
        .map(|pull| (pull.number, pull.status))
        .collect()
}

#[test]
fn pull_requests_from_pages_tells_merged_from_closed_without_merging() {
    let body = json!([
        closed(11, "2026-11-06T10:00:00Z", Some("2026-11-06T10:00:00Z")),
        closed(12, "2026-11-05T10:00:00Z", None)
    ])
    .to_string();

    let pulls = pull_requests_from_pages([body.as_str()]).unwrap();

    assert_eq!(
        statuses(&pulls),
        [
            (11, PullRequestStatus::Merged),
            (12, PullRequestStatus::Closed)
        ]
    );
}

#[test]
fn pull_requests_from_pages_marks_open_drafts() {
    let mut draft = pull(21, "2026-11-04T10:00:00Z");
    draft["draft"] = json!(true);
    let mut closed_draft = closed(22, "2026-11-03T10:00:00Z", None);
    closed_draft["draft"] = json!(true);
    let body = json!([pull(20, "2026-11-05T10:00:00Z"), draft, closed_draft]).to_string();

    let pulls = pull_requests_from_pages([body.as_str()]).unwrap();

    assert_eq!(
        statuses(&pulls),
        [
            (20, PullRequestStatus::Open),
            (21, PullRequestStatus::Draft),
            (22, PullRequestStatus::Closed)
        ]
    );
}

#[test]
fn pull_requests_from_pages_orders_most_recently_updated_first_across_pages() {
    let first = json!([
        pull(1, "2026-11-02T08:00:00Z"),
        pull(2, "2026-11-06T08:00:00Z")
    ])
    .to_string();
    let second = json!([
        pull(3, "2026-11-04T08:00:00Z"),
        pull(7, "2026-11-04T08:00:00Z")
    ])
    .to_string();

    let pulls = pull_requests_from_pages([first.as_str(), second.as_str()]).unwrap();

    let numbers: Vec<u32> = pulls.iter().map(|pull| pull.number).collect();
    assert_eq!(numbers, [2, 7, 3, 1]);
}

#[test]
fn pull_requests_from_pages_maps_author_labels_branches_and_link() {
    let mut deleted_author = pull(9, "2026-11-01T08:00:00Z");
    deleted_author["user"] = Value::Null;
    deleted_author["labels"] = json!([]);
    let body = json!([pull(8, "2026-11-03T08:00:00Z"), deleted_author]).to_string();

    let pulls = pull_requests_from_pages([body.as_str()]).unwrap();

    assert_eq!(
        pulls,
        [
            PullRequest {
                number: 8,
                title: "Pull request 8".into(),
                author: "sader-agents".into(),
                labels: vec!["feature".into()],
                head: "card/8".into(),
                base: "main".into(),
                status: PullRequestStatus::Open,
                updated_at: "2026-11-03T08:00:00Z".into(),
                html_url: "https://github.com/sadermine/git-viewme/pull/8".into(),
            },
            PullRequest {
                number: 9,
                title: "Pull request 9".into(),
                author: "ghost".into(),
                labels: vec![],
                head: "card/9".into(),
                base: "main".into(),
                status: PullRequestStatus::Open,
                updated_at: "2026-11-01T08:00:00Z".into(),
                html_url: "https://github.com/sadermine/git-viewme/pull/9".into(),
            },
        ]
    );
}

#[test]
fn pull_requests_from_pages_empty_repository_has_no_pull_requests() {
    assert_eq!(pull_requests_from_pages(["[]"]).unwrap(), []);
}

#[test]
fn pull_requests_from_pages_unexpected_body_is_an_error() {
    assert!(pull_requests_from_pages([r#"{"message":"oops"}"#]).is_err());
}

#[test]
fn pull_request_state_serializes_as_the_query_value() {
    for state in [PullRequestState::Open, PullRequestState::Closed] {
        assert_eq!(serde_json::to_value(state).unwrap(), json!(state.as_str()));
    }
}

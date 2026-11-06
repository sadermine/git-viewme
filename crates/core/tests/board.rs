use git_viewme_core::{Board, Card, Column, CoreError, board_from_pages};
use serde_json::{Value, json};

const PROJECT_URL: &str = "https://github.com/users/sadermine/projects/7";

fn page(field: Value, items: Value) -> String {
    json!({ "data": { "repositoryOwner": { "projectV2": {
        "url": PROJECT_URL,
        "field": field,
        "items": { "pageInfo": { "hasNextPage": false, "endCursor": null }, "nodes": items }
    }}}})
    .to_string()
}

fn status_field(names: &[&str]) -> Value {
    json!({ "options": names.iter().map(|name| json!({ "name": name })).collect::<Vec<_>>() })
}

fn issue(status: Option<&str>, number: u32) -> Value {
    json!({
        "isArchived": false,
        "fullDatabaseId": format!("{number}00"),
        "fieldValueByName": status.map(|name| json!({ "name": name })),
        "content": {
            "title": format!("Issue {number}"),
            "number": number,
            "url": format!("https://github.com/sadermine/repo/issues/{number}"),
            "labels": { "nodes": [{ "name": "bug" }] }
        }
    })
}

fn counts(board: &Board) -> Vec<(&str, usize)> {
    board
        .columns
        .iter()
        .map(|column| (column.name.as_str(), column.cards.len()))
        .collect()
}

#[test]
fn board_from_pages_items_across_pages_follow_status_option_order() {
    let options = status_field(&["Backlog", "Ready", "Done"]);
    let first = page(
        options.clone(),
        json!([issue(Some("Done"), 1), issue(Some("Backlog"), 2)]),
    );
    let second = page(options, json!([issue(Some("Done"), 3)]));

    let board = board_from_pages([first.as_str(), second.as_str()]).unwrap();

    assert_eq!(counts(&board), [("Backlog", 1), ("Ready", 0), ("Done", 2)]);
    assert_eq!(
        board.columns[0].cards[0],
        Card {
            title: "Issue 2".into(),
            number: Some(2),
            url: "https://github.com/sadermine/repo/issues/2".into(),
            labels: vec!["bug".into()],
        }
    );
}

#[test]
fn board_from_pages_items_without_a_known_status_go_first_in_no_status() {
    let body = page(
        status_field(&["Backlog", "Done"]),
        json!([
            issue(Some("Done"), 1),
            issue(None, 2),
            issue(Some("Deleted option"), 3)
        ]),
    );

    let board = board_from_pages([body.as_str()]).unwrap();

    assert_eq!(
        counts(&board),
        [("No status", 2), ("Backlog", 0), ("Done", 1)]
    );
}

#[test]
fn board_from_pages_project_without_status_field_puts_every_item_in_no_status() {
    let body = page(Value::Null, json!([issue(None, 1), issue(None, 2)]));

    let board = board_from_pages([body.as_str()]).unwrap();

    assert_eq!(counts(&board), [("No status", 2)]);
}

#[test]
fn board_from_pages_status_field_that_is_not_single_select_counts_as_no_status_field() {
    let body = page(json!({}), json!([issue(None, 1)]));

    let board = board_from_pages([body.as_str()]).unwrap();

    assert_eq!(counts(&board), [("No status", 1)]);
}

#[test]
fn board_from_pages_archived_items_are_left_out() {
    let mut archived = issue(Some("Backlog"), 2);
    archived["isArchived"] = json!(true);
    let body = page(
        status_field(&["Backlog"]),
        json!([issue(Some("Backlog"), 1), archived]),
    );

    let board = board_from_pages([body.as_str()]).unwrap();

    assert_eq!(counts(&board), [("Backlog", 1)]);
}

#[test]
fn board_from_pages_draft_has_title_only_and_opens_in_the_project() {
    let draft = json!({
        "isArchived": false,
        "fullDatabaseId": "555",
        "fieldValueByName": { "name": "Backlog" },
        "content": { "title": "Draft idea" }
    });
    let body = page(status_field(&["Backlog"]), json!([draft]));

    let board = board_from_pages([body.as_str()]).unwrap();

    assert_eq!(
        board.columns,
        [Column {
            name: "Backlog".into(),
            cards: vec![Card {
                title: "Draft idea".into(),
                number: None,
                url: format!("{PROJECT_URL}?pane=issue&itemId=555"),
                labels: vec![],
            }],
        }]
    );
}

#[test]
fn board_from_pages_missing_project_is_not_found() {
    let body = json!({ "data": { "repositoryOwner": { "projectV2": null } } }).to_string();

    assert_eq!(board_from_pages([body.as_str()]), Err(CoreError::NotFound));
}

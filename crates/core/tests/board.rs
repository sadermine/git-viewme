use git_viewme_core::{
    Board, Card, CardMove, Column, CoreError, board_from_pages, card_move_request,
};
use serde_json::{Value, json};

const PROJECT_URL: &str = "https://github.com/users/sadermine/projects/7";

/// The first page of the board query against GitHub's public roadmap project, trimmed to three
/// items: one `Paused`, one `Up Next` and one with no status.
const RECORDED: &str = include_str!("fixtures/public-roadmap-board.json");

fn page(field: Value, items: Value) -> String {
    json!({ "data": { "repositoryOwner": { "projectV2": {
        "id": "PVT_project",
        "url": PROJECT_URL,
        "field": field,
        "items": { "pageInfo": { "hasNextPage": false, "endCursor": null }, "nodes": items }
    }}}})
    .to_string()
}

fn status_field(names: &[&str]) -> Value {
    let options: Vec<Value> = names
        .iter()
        .map(|name| json!({ "id": format!("id-{name}"), "name": name }))
        .collect();
    json!({ "id": "PVTSSF_status", "options": options })
}

fn issue(status: Option<&str>, number: u32) -> Value {
    json!({
        "id": format!("PVTI_{number}"),
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

    assert_eq!(
        counts(&board),
        [("No status", 0), ("Backlog", 1), ("Ready", 0), ("Done", 2)]
    );
    assert_eq!(
        board.columns[1].cards[0],
        Card {
            item_id: "PVTI_2".into(),
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

    assert_eq!(counts(&board), [("No status", 0), ("Backlog", 1)]);
}

#[test]
fn board_from_pages_draft_has_title_only_and_opens_in_the_project() {
    let draft = json!({
        "id": "PVTI_draft",
        "isArchived": false,
        "fullDatabaseId": "555",
        "fieldValueByName": { "name": "Backlog" },
        "content": { "title": "Draft idea" }
    });
    let body = page(status_field(&["Backlog"]), json!([draft]));

    let board = board_from_pages([body.as_str()]).unwrap();

    assert_eq!(
        board.columns[1],
        Column {
            name: "Backlog".into(),
            option_id: Some("id-Backlog".into()),
            cards: vec![Card {
                item_id: "PVTI_draft".into(),
                title: "Draft idea".into(),
                number: None,
                url: format!("{PROJECT_URL}?pane=issue&itemId=555"),
                labels: vec![],
            }],
        }
    );
}

#[test]
fn board_from_pages_missing_project_is_not_found() {
    let body = json!({ "data": { "repositoryOwner": { "projectV2": null } } }).to_string();

    assert_eq!(board_from_pages([body.as_str()]), Err(CoreError::NotFound));
}

#[test]
fn board_from_pages_project_without_status_field_has_no_field_to_move_with() {
    let body = page(Value::Null, json!([issue(None, 1)]));

    let board = board_from_pages([body.as_str()]).unwrap();

    assert_eq!(board.project_id, "PVT_project");
    assert_eq!(board.status_field_id, None);
}

#[test]
fn board_from_pages_recorded_board_carries_the_ids_a_move_names() {
    let board = board_from_pages([RECORDED]).unwrap();

    assert_eq!(board.project_id, "PVT_kwDNJr_NE10");
    assert_eq!(
        board.status_field_id.as_deref(),
        Some("PVTSSF_lADNJr_NE13Nml4")
    );
    let columns: Vec<(&str, Option<&str>, Vec<&str>)> = board
        .columns
        .iter()
        .map(|column| {
            (
                column.name.as_str(),
                column.option_id.as_deref(),
                column
                    .cards
                    .iter()
                    .map(|card| card.item_id.as_str())
                    .collect(),
            )
        })
        .collect();
    assert_eq!(
        columns,
        [
            ("No status", None, vec!["PVTI_lADNJr_NE13OA0SSnA"]),
            ("Up Next", Some("f75ad846"), vec!["PVTI_lADNJr_NE13OAALQrQ"]),
            ("Exploring", Some("bebfd2a1"), vec![]),
            ("Shipped", Some("7bdf8bc2"), vec![]),
            ("Paused", Some("0b6e37be"), vec!["PVTI_lADNJr_NE13OBakjpw"]),
        ]
    );
}

/// The move a drop of `card` on column `to` sends, built from the board as the interface does.
fn move_on(board: &Board, card: &Card, to: usize) -> CardMove {
    CardMove {
        project_id: board.project_id.clone(),
        item_id: card.item_id.clone(),
        field_id: board.status_field_id.clone().unwrap(),
        option_id: board.columns[to].option_id.clone(),
    }
}

#[test]
fn card_move_request_to_a_lane_sets_status_to_its_option() {
    let board = board_from_pages([RECORDED]).unwrap();
    let paused = &board.columns[4].cards[0];

    let (mutation, variables) = card_move_request(&move_on(&board, paused, 1));

    assert!(
        mutation.contains("updateProjectV2ItemFieldValue"),
        "{mutation}"
    );
    assert!(
        mutation.contains("singleSelectOptionId: $option"),
        "{mutation}"
    );
    assert_eq!(
        variables,
        json!({
            "project": "PVT_kwDNJr_NE10",
            "item": "PVTI_lADNJr_NE13OBakjpw",
            "field": "PVTSSF_lADNJr_NE13Nml4",
            "option": "f75ad846",
        })
    );
}

#[test]
fn card_move_request_to_no_status_clears_status() {
    let board = board_from_pages([RECORDED]).unwrap();
    let up_next = &board.columns[1].cards[0];

    let (mutation, variables) = card_move_request(&move_on(&board, up_next, 0));

    assert!(
        mutation.contains("clearProjectV2ItemFieldValue"),
        "{mutation}"
    );
    assert_eq!(
        variables,
        json!({
            "project": "PVT_kwDNJr_NE10",
            "item": "PVTI_lADNJr_NE13OAALQrQ",
            "field": "PVTSSF_lADNJr_NE13Nml4",
        })
    );
}

#[test]
fn card_move_deserializes_from_the_interface_shape() {
    let card_move: CardMove = serde_json::from_value(json!({
        "project_id": "PVT_1",
        "item_id": "PVTI_1",
        "field_id": "PVTSSF_1",
        "option_id": null,
    }))
    .unwrap();

    assert_eq!(card_move.option_id, None);
}

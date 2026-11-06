use serde::Deserialize;
use serde_json::json;

use crate::{Board, Card, Column, CoreError, GitHub, ProjectRef};

/// The column for items with no status, or every item when the project has no `Status` field.
const NO_STATUS: &str = "No status";

const QUERY: &str = r#"
query($owner: String!, $number: Int!, $after: String) {
  repositoryOwner(login: $owner) {
    ... on ProjectV2Owner {
      projectV2(number: $number) {
        url
        field(name: "Status") { ... on ProjectV2SingleSelectField { options { name } } }
        items(first: 100, after: $after) {
          pageInfo { hasNextPage endCursor }
          nodes {
            isArchived
            fullDatabaseId
            fieldValueByName(name: "Status") { ... on ProjectV2ItemFieldSingleSelectValue { name } }
            content {
              ... on Issue { title number url labels(first: 20) { nodes { name } } }
              ... on PullRequest { title number url labels(first: 20) { nodes { name } } }
              ... on DraftIssue { title }
            }
          }
        }
      }
    }
  }
}"#;

impl GitHub {
    pub async fn project_board(&self, project: &ProjectRef) -> Result<Board, CoreError> {
        let pages = self
            .graphql_pages(
                QUERY,
                json!({ "owner": project.owner, "number": project.number }),
                "/data/repositoryOwner/projectV2/items/pageInfo",
            )
            .await?;
        board_from_pages(pages.iter().map(String::as_str))
    }
}

/// Groups every page of the project-items GraphQL query into columns. The columns follow the
/// options of the project's `Status` field, with "No status" first when any item has no status.
/// Archived items are left out, as on GitHub's board.
pub fn board_from_pages<'a>(pages: impl IntoIterator<Item = &'a str>) -> Result<Board, CoreError> {
    let mut statuses: Option<Vec<String>> = None;
    let mut no_status = Vec::new();
    let mut by_status: Vec<Vec<Card>> = Vec::new();
    for page in pages {
        let page: Page = serde_json::from_str(page).map_err(|error| {
            CoreError::other(format!("GitHub sent an unexpected project: {error}"))
        })?;
        let project = page
            .data
            .repository_owner
            .and_then(|owner| owner.project_v2)
            .ok_or(CoreError::NotFound)?;
        let options = statuses.get_or_insert_with(|| {
            let names: Vec<String> = project
                .field
                .and_then(|field| field.options)
                .unwrap_or_default()
                .into_iter()
                .map(|option| option.name)
                .collect();
            by_status = names.iter().map(|_| Vec::new()).collect();
            names
        });
        for item in project
            .items
            .nodes
            .into_iter()
            .filter(|item| !item.is_archived)
        {
            let status = item.field_value_by_name.and_then(|value| value.name);
            let card = card(item.content, item.full_database_id, &project.url);
            match status.and_then(|name| options.iter().position(|option| *option == name)) {
                Some(index) => by_status[index].push(card),
                None => no_status.push(card),
            }
        }
    }
    let options = statuses.unwrap_or_default();
    let mut columns = Vec::with_capacity(options.len() + 1);
    if options.is_empty() || !no_status.is_empty() {
        columns.push(Column {
            name: NO_STATUS.to_owned(),
            cards: no_status,
        });
    }
    columns.extend(
        options
            .into_iter()
            .zip(by_status)
            .map(|(name, cards)| Column { name, cards }),
    );
    Ok(Board { columns })
}

fn card(content: Option<Content>, item_id: Option<String>, project_url: &str) -> Card {
    // A draft has no page of its own, so its card opens the draft in the project.
    let draft_url = || match item_id {
        Some(id) => format!("{project_url}?pane=issue&itemId={id}"),
        None => project_url.to_owned(),
    };
    match content {
        Some(Content {
            title,
            number: Some(number),
            url: Some(url),
            labels,
        }) => Card {
            title,
            number: Some(number),
            url,
            labels: labels
                .map(|labels| labels.nodes.into_iter().map(|label| label.name).collect())
                .unwrap_or_default(),
        },
        Some(Content { title, .. }) => Card {
            title,
            number: None,
            url: draft_url(),
            labels: Vec::new(),
        },
        // GitHub sends no content for an item the user may not see.
        None => Card {
            title: "Item you do not have access to".to_owned(),
            number: None,
            url: draft_url(),
            labels: Vec::new(),
        },
    }
}

#[derive(Deserialize)]
struct Page {
    data: Data,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Data {
    repository_owner: Option<Owner>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Owner {
    project_v2: Option<Project>,
}

#[derive(Deserialize)]
struct Project {
    url: String,
    field: Option<StatusField>,
    items: Items,
}

#[derive(Deserialize)]
struct StatusField {
    options: Option<Vec<StatusOption>>,
}

#[derive(Deserialize)]
struct StatusOption {
    name: String,
}

#[derive(Deserialize)]
struct Items {
    nodes: Vec<Item>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Item {
    is_archived: bool,
    full_database_id: Option<String>,
    field_value_by_name: Option<StatusValue>,
    content: Option<Content>,
}

#[derive(Deserialize)]
struct StatusValue {
    name: Option<String>,
}

#[derive(Deserialize)]
struct Content {
    title: String,
    number: Option<u32>,
    url: Option<String>,
    labels: Option<Labels>,
}

#[derive(Deserialize)]
struct Labels {
    nodes: Vec<Label>,
}

#[derive(Deserialize)]
struct Label {
    name: String,
}

use serde::Deserialize;
use serde_json::json;

use crate::{CoreError, GitHub, ProjectRef, Repo, RepoRef};

const QUERY: &str = r#"
query($after: String) {
  viewer {
    repositories(affiliations: [OWNER, COLLABORATOR, ORGANIZATION_MEMBER], first: 100, after: $after) {
      pageInfo { hasNextPage endCursor }
      nodes {
        name
        owner { login }
        url
        isPrivate
        defaultBranchRef { name }
        projectsV2(first: 20) {
          nodes { number title owner { ... on User { login } ... on Organization { login } } }
        }
      }
    }
  }
}"#;

impl GitHub {
    /// Every repository the signed-in user owns, collaborates on, or reaches through an
    /// organization, sorted by owner and then name, ignoring case.
    pub async fn list_repos(&self) -> Result<Vec<Repo>, CoreError> {
        let pages = self
            .graphql_pages(QUERY, json!({}), "/data/viewer/repositories/pageInfo")
            .await?;
        repos_from_pages(pages.iter().map(String::as_str))
    }
}

/// Turns the bodies of the repository-list GraphQL query into sorted repositories.
pub fn repos_from_pages<'a>(
    pages: impl IntoIterator<Item = &'a str>,
) -> Result<Vec<Repo>, CoreError> {
    let mut repos = Vec::new();
    for page in pages {
        let page: Page = serde_json::from_str(page).map_err(|error| {
            CoreError::other(format!(
                "GitHub sent an unexpected repository list: {error}"
            ))
        })?;
        repos.extend(page.data.viewer.repositories.nodes.into_iter().map(|node| {
            Repo {
                r#ref: RepoRef {
                    owner: node.owner.login,
                    name: node.name,
                },
                default_branch: node.default_branch_ref.map(|branch| branch.name),
                html_url: node.url,
                private: node.is_private,
                linked_projects: node
                    .projects_v2
                    .nodes
                    .into_iter()
                    .map(|project| ProjectRef {
                        owner: project.owner.login,
                        number: project.number,
                        title: project.title,
                    })
                    .collect(),
            }
        }));
    }
    repos.sort_by_cached_key(|repo| {
        (
            repo.r#ref.owner.to_lowercase(),
            repo.r#ref.name.to_lowercase(),
        )
    });
    Ok(repos)
}

#[derive(Deserialize)]
struct Page {
    data: Data,
}

#[derive(Deserialize)]
struct Data {
    viewer: Viewer,
}

#[derive(Deserialize)]
struct Viewer {
    repositories: Nodes<RepoNode>,
}

#[derive(Deserialize)]
struct Nodes<T> {
    nodes: Vec<T>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RepoNode {
    name: String,
    owner: Login,
    url: String,
    is_private: bool,
    default_branch_ref: Option<BranchName>,
    projects_v2: Nodes<ProjectNode>,
}

#[derive(Deserialize)]
struct Login {
    login: String,
}

#[derive(Deserialize)]
struct BranchName {
    name: String,
}

#[derive(Deserialize)]
struct ProjectNode {
    number: u32,
    title: String,
    owner: Login,
}

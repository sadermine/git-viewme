use git_viewme_core::{ProjectRef, Repo, RepoRef, repos_from_pages};
use serde_json::{Value, json};

fn page(nodes: Value) -> String {
    json!({ "data": { "viewer": { "repositories": {
        "pageInfo": { "hasNextPage": false, "endCursor": null },
        "nodes": nodes
    }}}})
    .to_string()
}

fn node(owner: &str, name: &str, branch: Option<&str>, projects: Value) -> Value {
    json!({
        "name": name,
        "owner": { "login": owner },
        "url": format!("https://github.com/{owner}/{name}"),
        "isPrivate": true,
        "defaultBranchRef": branch.map(|name| json!({ "name": name })),
        "projectsV2": { "nodes": projects }
    })
}

#[test]
fn repos_from_pages_sorts_by_owner_then_name_ignoring_case() {
    let first = page(json!([
        node("sadermine", "git-viewme", Some("main"), json!([])),
        node("Sader-Agents", "scratch", Some("main"), json!([]))
    ]));
    let second = page(json!([node(
        "sadermine",
        "Ai-usage-widget",
        Some("main"),
        json!([])
    )]));

    let repos = repos_from_pages([first.as_str(), second.as_str()]).unwrap();

    let names: Vec<String> = repos
        .iter()
        .map(|repo| format!("{}/{}", repo.r#ref.owner, repo.r#ref.name))
        .collect();
    assert_eq!(
        names,
        [
            "Sader-Agents/scratch",
            "sadermine/Ai-usage-widget",
            "sadermine/git-viewme"
        ]
    );
}

#[test]
fn repos_from_pages_maps_empty_repository_and_linked_projects() {
    let projects =
        json!([{ "number": 7, "title": "Trader board", "owner": { "login": "sadermine" } }]);
    let body = page(json!([node("sadermine", "sader-trader", None, projects)]));

    let repos = repos_from_pages([body.as_str()]).unwrap();

    assert_eq!(
        repos,
        [Repo {
            r#ref: RepoRef {
                owner: "sadermine".into(),
                name: "sader-trader".into()
            },
            default_branch: None,
            html_url: "https://github.com/sadermine/sader-trader".into(),
            private: true,
            linked_projects: vec![ProjectRef {
                owner: "sadermine".into(),
                number: 7,
                title: "Trader board".into()
            }],
        }]
    );
}

#[test]
fn repo_serializes_ref_under_its_spec_name() {
    let body = page(json!([node("o", "n", Some("main"), json!([]))]));

    let json = serde_json::to_value(&repos_from_pages([body.as_str()]).unwrap()[0]).unwrap();

    assert_eq!(json["ref"], json!({ "owner": "o", "name": "n" }));
    assert_eq!(json["default_branch"], json!("main"));
}

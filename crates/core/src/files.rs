use std::cmp::Ordering;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::Deserialize;

use crate::{CoreError, FileContent, GitHub, RepoRef, TreeEntry, TreeEntryKind};

/// Files above this size are not fetched as text. GitHub's contents API stops returning content
/// at the same size.
const MAX_TEXT_SIZE: u64 = 1024 * 1024;

impl GitHub {
    pub async fn file_tree(
        &self,
        repo: &RepoRef,
        branch: &str,
    ) -> Result<Vec<TreeEntry>, CoreError> {
        let body = self
            .get(
                &["repos", &repo.owner, &repo.name, "git", "trees", branch],
                &[("recursive", "1")],
            )
            .await?;
        tree_from_response(&body)
    }

    pub async fn file_content(
        &self,
        repo: &RepoRef,
        branch: &str,
        path: &str,
    ) -> Result<FileContent, CoreError> {
        let mut segments = vec!["repos", &repo.owner, &repo.name, "contents"];
        segments.extend(path.split('/'));
        let body = self.get(&segments, &[("ref", branch)]).await?;
        file_content_from_response(&body)
    }
}

/// Turns a recursive git-trees response into entries in display order: depth first, and within a
/// folder, folders before files, each sorted by name ignoring case. Submodules are left out.
pub fn tree_from_response(body: &str) -> Result<Vec<TreeEntry>, CoreError> {
    let tree: Tree = serde_json::from_str(body).map_err(|error| {
        CoreError::other(format!("GitHub sent an unexpected file tree: {error}"))
    })?;
    let mut entries: Vec<TreeEntry> = tree
        .tree
        .into_iter()
        .filter_map(|node| {
            let kind = match node.kind.as_str() {
                "blob" => TreeEntryKind::File,
                "tree" => TreeEntryKind::Dir,
                _ => return None,
            };
            Some(TreeEntry {
                path: node.path,
                kind,
                size: node.size,
            })
        })
        .collect();
    entries.sort_by(display_order);
    Ok(entries)
}

fn display_order(a: &TreeEntry, b: &TreeEntry) -> Ordering {
    let a_parts: Vec<&str> = a.path.split('/').collect();
    let b_parts: Vec<&str> = b.path.split('/').collect();
    let is_dir = |parts: &[&str], index: usize, kind: TreeEntryKind| {
        index + 1 < parts.len() || kind == TreeEntryKind::Dir
    };
    for (index, (a_part, b_part)) in a_parts.iter().zip(&b_parts).enumerate() {
        if a_part == b_part {
            continue;
        }
        let a_dir = is_dir(&a_parts, index, a.kind);
        let b_dir = is_dir(&b_parts, index, b.kind);
        return b_dir
            .cmp(&a_dir)
            .then_with(|| a_part.to_lowercase().cmp(&b_part.to_lowercase()))
            .then_with(|| a_part.cmp(b_part));
    }
    a_parts.len().cmp(&b_parts.len())
}

/// Turns a contents-API response for one file into text, or says why it is not shown as text.
pub fn file_content_from_response(body: &str) -> Result<FileContent, CoreError> {
    let file: Contents = serde_json::from_str(body).map_err(|error| {
        CoreError::other(format!("GitHub sent unexpected file content: {error}"))
    })?;
    let size = file.size;
    if size > MAX_TEXT_SIZE {
        return Ok(FileContent::TooLarge { size });
    }
    let (Some("base64"), Some(encoded)) = (file.encoding.as_deref(), file.content) else {
        return Ok(FileContent::Binary { size });
    };
    let compact: String = encoded.split_whitespace().collect();
    let bytes = STANDARD.decode(compact).map_err(|error| {
        CoreError::other(format!(
            "GitHub sent file content that is not base64: {error}"
        ))
    })?;
    if bytes.contains(&0) {
        return Ok(FileContent::Binary { size });
    }
    match String::from_utf8(bytes) {
        Ok(text) => Ok(FileContent::Text {
            text: text
                .strip_prefix('\u{feff}')
                .map(str::to_owned)
                .unwrap_or(text),
        }),
        Err(_) => Ok(FileContent::Binary { size }),
    }
}

#[derive(Deserialize)]
struct Tree {
    tree: Vec<TreeNode>,
}

#[derive(Deserialize)]
struct TreeNode {
    path: String,
    #[serde(rename = "type")]
    kind: String,
    size: Option<u64>,
}

#[derive(Deserialize)]
struct Contents {
    size: u64,
    encoding: Option<String>,
    content: Option<String>,
}

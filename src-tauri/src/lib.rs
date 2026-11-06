//! Tauri commands. Each one calls one function in git-viewme-core and returns its result, so the
//! GitHub logic stays in the crate a hosted version can reuse.

use git_viewme_core::{
    Board, ClientId, CoreError, DevicePrompt, FileContent, GitHub, ProjectRef, PullRequest,
    PullRequestState, Repo, RepoRef, TreeEntry,
};
use tauri::State;
use tauri::ipc::Channel;

#[tauri::command]
fn signed_in() -> Result<bool, CoreError> {
    git_viewme_core::is_signed_in()
}

#[tauri::command]
fn client_id() -> Result<Option<ClientId>, CoreError> {
    git_viewme_core::client_id()
}

#[tauri::command]
fn save_client_id(client_id: String) -> Result<ClientId, CoreError> {
    git_viewme_core::save_client_id(&client_id)
}

#[tauri::command]
async fn sign_in(
    github: State<'_, GitHub>,
    on_prompt: Channel<DevicePrompt>,
) -> Result<(), CoreError> {
    github
        .sign_in(|prompt| {
            // The window is gone if this fails, and then nobody is waiting for the code.
            let _ = on_prompt.send(prompt);
        })
        .await
}

#[tauri::command]
fn sign_out() -> Result<(), CoreError> {
    git_viewme_core::delete_token()
}

#[tauri::command]
async fn list_repos(github: State<'_, GitHub>) -> Result<Vec<Repo>, CoreError> {
    github.list_repos().await
}

#[tauri::command]
async fn file_tree(
    github: State<'_, GitHub>,
    repo: RepoRef,
    branch: String,
) -> Result<Vec<TreeEntry>, CoreError> {
    github.file_tree(&repo, &branch).await
}

#[tauri::command]
async fn file_content(
    github: State<'_, GitHub>,
    repo: RepoRef,
    branch: String,
    path: String,
) -> Result<FileContent, CoreError> {
    github.file_content(&repo, &branch, &path).await
}

#[tauri::command]
async fn project_board(github: State<'_, GitHub>, project: ProjectRef) -> Result<Board, CoreError> {
    github.project_board(&project).await
}

#[tauri::command]
async fn list_pull_requests(
    github: State<'_, GitHub>,
    repo: RepoRef,
    state: PullRequestState,
) -> Result<Vec<PullRequest>, CoreError> {
    github.list_pull_requests(&repo, state).await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(GitHub::default())
        .invoke_handler(tauri::generate_handler![
            signed_in,
            client_id,
            save_client_id,
            sign_in,
            sign_out,
            list_repos,
            file_tree,
            file_content,
            project_board,
            list_pull_requests
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

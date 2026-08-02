//! Browse and edit files inside a workspace's worktree.

use std::path::{Component, Path, PathBuf};

use axum::{
    Extension, Json, Router,
    extract::Query,
    response::Json as ResponseJson,
    routing::{get, put},
};
use db::models::workspace::Workspace;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use utils::response::ApiResponse;

use crate::{DeploymentImpl, error::ApiError};

/// Files larger than this are not editable in the browser.
const MAX_EDITABLE_FILE_SIZE: u64 = 2 * 1024 * 1024;

#[derive(Debug, Deserialize)]
pub struct FilePathQuery {
    #[serde(default)]
    pub path: String,
}

#[derive(Debug, Serialize, TS)]
pub struct WorkspaceFileEntry {
    pub name: String,
    /// Path relative to the workspace root.
    pub path: String,
    pub is_dir: bool,
}

#[derive(Debug, Serialize, TS)]
pub struct WorkspaceFileContent {
    pub path: String,
    pub content: String,
}

#[derive(Debug, Deserialize, TS)]
pub struct WriteWorkspaceFile {
    pub path: String,
    pub content: String,
}

fn workspace_root(workspace: &Workspace) -> Result<PathBuf, ApiError> {
    let container_ref = workspace
        .container_ref
        .as_ref()
        .ok_or_else(|| ApiError::BadRequest("Workspace has no directory yet".to_string()))?;
    let root = PathBuf::from(container_ref);
    if !root.is_dir() {
        return Err(ApiError::BadRequest(
            "Workspace directory does not exist".to_string(),
        ));
    }
    Ok(root)
}

/// Join a client-supplied relative path onto the workspace root, rejecting
/// absolute paths and parent-directory traversal.
fn resolve_path(root: &Path, relative: &str) -> Result<PathBuf, ApiError> {
    let mut resolved = root.to_path_buf();
    for component in Path::new(relative).components() {
        match component {
            Component::Normal(part) => resolved.push(part),
            Component::CurDir => {}
            _ => {
                return Err(ApiError::BadRequest(format!("Invalid path: {relative}")));
            }
        }
    }
    Ok(resolved)
}

pub async fn list_files(
    Extension(workspace): Extension<Workspace>,
    Query(query): Query<FilePathQuery>,
) -> Result<ResponseJson<ApiResponse<Vec<WorkspaceFileEntry>>>, ApiError> {
    let root = workspace_root(&workspace)?;
    let dir = resolve_path(&root, &query.path)?;

    let mut read_dir = tokio::fs::read_dir(&dir)
        .await
        .map_err(|e| ApiError::BadRequest(format!("Failed to read directory: {e}")))?;

    let mut entries = Vec::new();
    while let Some(entry) = read_dir
        .next_entry()
        .await
        .map_err(|e| ApiError::BadRequest(format!("Failed to read directory: {e}")))?
    {
        let name = entry.file_name().to_string_lossy().to_string();
        // Hide the git object store; everything else is fair game
        if query.path.is_empty() && name == ".git" {
            continue;
        }
        let is_dir = entry.file_type().await.map(|t| t.is_dir()).unwrap_or(false);
        let path = if query.path.is_empty() {
            name.clone()
        } else {
            format!("{}/{}", query.path.trim_end_matches('/'), name)
        };
        entries.push(WorkspaceFileEntry { name, path, is_dir });
    }

    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    Ok(ResponseJson(ApiResponse::success(entries)))
}

pub async fn read_file(
    Extension(workspace): Extension<Workspace>,
    Query(query): Query<FilePathQuery>,
) -> Result<ResponseJson<ApiResponse<WorkspaceFileContent>>, ApiError> {
    let root = workspace_root(&workspace)?;
    let file_path = resolve_path(&root, &query.path)?;

    let metadata = tokio::fs::metadata(&file_path)
        .await
        .map_err(|e| ApiError::BadRequest(format!("Failed to read file: {e}")))?;
    if metadata.is_dir() {
        return Err(ApiError::BadRequest("Path is a directory".to_string()));
    }
    if metadata.len() > MAX_EDITABLE_FILE_SIZE {
        return Err(ApiError::BadRequest(
            "File is too large to edit in the browser".to_string(),
        ));
    }

    let bytes = tokio::fs::read(&file_path)
        .await
        .map_err(|e| ApiError::BadRequest(format!("Failed to read file: {e}")))?;
    let content = String::from_utf8(bytes)
        .map_err(|_| ApiError::BadRequest("File is not valid UTF-8 text".to_string()))?;

    Ok(ResponseJson(ApiResponse::success(WorkspaceFileContent {
        path: query.path,
        content,
    })))
}

pub async fn write_file(
    Extension(workspace): Extension<Workspace>,
    Json(request): Json<WriteWorkspaceFile>,
) -> Result<ResponseJson<ApiResponse<WorkspaceFileContent>>, ApiError> {
    let root = workspace_root(&workspace)?;
    let file_path = resolve_path(&root, &request.path)?;

    if file_path.is_dir() {
        return Err(ApiError::BadRequest("Path is a directory".to_string()));
    }

    tokio::fs::write(&file_path, request.content.as_bytes())
        .await
        .map_err(|e| ApiError::BadRequest(format!("Failed to write file: {e}")))?;

    Ok(ResponseJson(ApiResponse::success(WorkspaceFileContent {
        path: request.path,
        content: request.content,
    })))
}

pub fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route("/list", get(list_files))
        .route("/read", get(read_file))
        .route("/write", put(write_file))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_path_allows_nested_relative_paths() {
        let root = Path::new("/tmp/ws");
        assert_eq!(
            resolve_path(root, "src/main.rs").unwrap(),
            PathBuf::from("/tmp/ws/src/main.rs")
        );
        assert_eq!(resolve_path(root, "").unwrap(), PathBuf::from("/tmp/ws"));
    }

    #[test]
    fn resolve_path_rejects_traversal_and_absolute() {
        let root = Path::new("/tmp/ws");
        assert!(resolve_path(root, "../etc/passwd").is_err());
        assert!(resolve_path(root, "src/../../etc").is_err());
        assert!(resolve_path(root, "/etc/passwd").is_err());
    }
}

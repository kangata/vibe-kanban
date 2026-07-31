//! Minimal helpers around the GitLab CLI (`glab`).
//!
//! This module provides low-level access to the GitLab CLI for GitLab
//! merge request operations. Merge requests are surfaced through the
//! provider-agnostic `PullRequestDetail` types.

use std::{
    ffi::{OsStr, OsString},
    path::Path,
    process::Command,
};

use chrono::{DateTime, Utc};
use db::models::merge::MergeStatus;
use serde::Deserialize;
use thiserror::Error;
use utils::{command_ext::NoWindowExt, shell::resolve_executable_path_blocking};

use crate::types::{CreatePrRequest, PullRequestDetail, UnifiedPrComment};

#[derive(Debug, Clone)]
pub struct GitLabRepoInfo {
    /// Hostname of the GitLab instance (e.g. `gitlab.com` or a self-managed host).
    pub host: String,
    /// Full project path including groups/subgroups (e.g. `group/subgroup/project`).
    pub full_path: String,
}

impl GitLabRepoInfo {
    /// Parse host and full project path from an HTTPS, SSH, or scp-style remote URL.
    pub fn from_remote_url(remote_url: &str) -> Option<Self> {
        let url = remote_url
            .trim()
            .trim_end_matches('/')
            .trim_end_matches(".git");

        let (host, path) = if let Some(rest) = url
            .strip_prefix("https://")
            .or_else(|| url.strip_prefix("http://"))
        {
            let rest = rest.rsplit_once('@').map(|(_, r)| r).unwrap_or(rest);
            rest.split_once('/')?
        } else if let Some(rest) = url.strip_prefix("ssh://") {
            let rest = rest.rsplit_once('@').map(|(_, r)| r).unwrap_or(rest);
            let (host_port, path) = rest.split_once('/')?;
            let host = host_port
                .split_once(':')
                .map(|(h, _)| h)
                .unwrap_or(host_port);
            (host, path)
        } else if let Some((user_host, path)) = url.split_once(':') {
            // scp-style: git@host:group/project
            let host = user_host
                .rsplit_once('@')
                .map(|(_, h)| h)
                .unwrap_or(user_host);
            (host, path)
        } else {
            return None;
        };

        if host.is_empty() || path.is_empty() || !path.contains('/') {
            return None;
        }

        Some(Self {
            host: host.to_string(),
            full_path: path.to_string(),
        })
    }

    /// Repository spec accepted by `glab --repo` (full URL form).
    pub fn repo_spec(&self) -> String {
        format!("https://{}/{}", self.host, self.full_path)
    }

    /// URL-encoded project path for REST API endpoints.
    fn encoded_path(&self) -> String {
        self.full_path.replace('/', "%2F")
    }
}

#[derive(Deserialize)]
struct GlabMrResponse {
    iid: i64,
    web_url: String,
    state: Option<String>,
    merged_at: Option<String>,
    merge_commit_sha: Option<String>,
    squash_commit_sha: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    source_branch: Option<String>,
    #[serde(default)]
    target_branch: Option<String>,
}

#[derive(Deserialize)]
struct GlabDiscussion {
    notes: Option<Vec<GlabNote>>,
}

#[derive(Deserialize)]
struct GlabNote {
    id: Option<i64>,
    author: Option<GlabAuthor>,
    body: Option<String>,
    created_at: Option<String>,
    system: Option<bool>,
    position: Option<GlabNotePosition>,
}

#[derive(Deserialize)]
struct GlabAuthor {
    username: Option<String>,
}

#[derive(Deserialize)]
struct GlabNotePosition {
    new_path: Option<String>,
    old_path: Option<String>,
    new_line: Option<i64>,
    old_line: Option<i64>,
}

#[derive(Debug, Error)]
pub enum GlabCliError {
    #[error("GitLab CLI (`glab`) executable not found or not runnable")]
    NotAvailable,
    #[error("GitLab CLI command failed: {0}")]
    CommandFailed(String),
    #[error("GitLab CLI authentication failed: {0}")]
    AuthFailed(String),
    #[error("GitLab CLI returned unexpected output: {0}")]
    UnexpectedOutput(String),
}

#[derive(Debug, Clone, Default)]
pub struct GlabCli;

impl GlabCli {
    pub fn new() -> Self {
        Self {}
    }

    /// Ensure the GitLab CLI binary is discoverable.
    fn ensure_available(&self) -> Result<(), GlabCliError> {
        resolve_executable_path_blocking("glab").ok_or(GlabCliError::NotAvailable)?;
        Ok(())
    }

    fn run<I, S>(&self, args: I, dir: Option<&Path>) -> Result<String, GlabCliError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.ensure_available()?;
        let glab = resolve_executable_path_blocking("glab").ok_or(GlabCliError::NotAvailable)?;
        let mut cmd = Command::new(&glab);

        if let Some(d) = dir {
            cmd.current_dir(d);
        }

        for arg in args {
            cmd.arg(arg);
        }
        // Never let glab open interactive prompts or an editor
        cmd.env("GLAB_PROMPT_DISABLED", "true");
        tracing::debug!(
            "Running GitLab CLI command: {:?} {:?}",
            glab,
            cmd.get_args()
        );

        let output = cmd
            .no_window()
            .output()
            .map_err(|err| GlabCliError::CommandFailed(err.to_string()))?;

        if output.status.success() {
            return Ok(String::from_utf8_lossy(&output.stdout).to_string());
        }

        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

        // Check for authentication errors
        let lower = stderr.to_ascii_lowercase();
        if lower.contains("glab auth login")
            || lower.contains("not authenticated")
            || lower.contains("authentication")
            || lower.contains("unauthorized")
            || lower.contains("401")
            || lower.contains("token is invalid")
        {
            return Err(GlabCliError::AuthFailed(stderr));
        }

        Err(GlabCliError::CommandFailed(stderr))
    }

    /// Run `glab mr create` and parse the merge request URL from its output.
    ///
    /// `glab mr create` has no JSON output, so the returned detail is built
    /// from the request plus the URL/iid printed by the CLI (mirroring the
    /// GitHub implementation's handling of `gh pr create`).
    pub fn create_mr(
        &self,
        request: &CreatePrRequest,
        repo_info: &GitLabRepoInfo,
        repo_path: &Path,
    ) -> Result<PullRequestDetail, GlabCliError> {
        let body = request.body.as_deref().unwrap_or("");

        let mut args: Vec<OsString> = Vec::with_capacity(18);
        args.push(OsString::from("mr"));
        args.push(OsString::from("create"));
        args.push(OsString::from("--repo"));
        args.push(OsString::from(repo_info.repo_spec()));
        args.push(OsString::from("--source-branch"));
        args.push(OsString::from(&request.head_branch));
        args.push(OsString::from("--target-branch"));
        args.push(OsString::from(&request.base_branch));
        args.push(OsString::from("--title"));
        args.push(OsString::from(&request.title));
        args.push(OsString::from("--description"));
        args.push(OsString::from(body));
        args.push(OsString::from("--yes"));

        if request.draft.unwrap_or(false) {
            args.push(OsString::from("--draft"));
        }

        // Cross-fork merge requests: select the head repository explicitly
        if let Some(head_url) = &request.head_repo_url
            && head_url != &repo_info.repo_spec()
            && let Some(head_info) = GitLabRepoInfo::from_remote_url(head_url)
        {
            args.push(OsString::from("--head"));
            args.push(OsString::from(head_info.full_path));
        }

        let raw = self.run(args, Some(repo_path))?;
        Self::parse_mr_create_text(&raw, request)
    }

    /// Retrieve details for a merge request by URL.
    pub fn view_mr(&self, mr_url: &str) -> Result<PullRequestDetail, GlabCliError> {
        let (repo_spec, iid) = Self::parse_mr_url(mr_url).ok_or_else(|| {
            GlabCliError::UnexpectedOutput(format!("Could not parse GitLab MR URL: {mr_url}"))
        })?;

        let raw = self.run(
            [
                "mr",
                "view",
                &iid.to_string(),
                "--repo",
                &repo_spec,
                "--output",
                "json",
            ],
            None,
        )?;
        Self::parse_mr_response(&raw)
    }

    /// List merge requests for a source branch (all states).
    pub fn list_mrs_for_branch(
        &self,
        repo_info: &GitLabRepoInfo,
        branch: &str,
    ) -> Result<Vec<PullRequestDetail>, GlabCliError> {
        let raw = self.run(
            [
                "mr",
                "list",
                "--repo",
                &repo_info.repo_spec(),
                "--source-branch",
                branch,
                "--all",
                "--output",
                "json",
            ],
            None,
        )?;
        Self::parse_mr_list_response(&raw)
    }

    /// List open merge requests for the repository.
    pub fn list_open_mrs(
        &self,
        repo_info: &GitLabRepoInfo,
    ) -> Result<Vec<PullRequestDetail>, GlabCliError> {
        let raw = self.run(
            [
                "mr",
                "list",
                "--repo",
                &repo_info.repo_spec(),
                "--output",
                "json",
            ],
            None,
        )?;
        Self::parse_mr_list_response(&raw)
    }

    /// Fetch merge request discussions via the REST API and flatten them
    /// into unified comments.
    pub fn get_mr_discussions(
        &self,
        repo_info: &GitLabRepoInfo,
        mr_iid: i64,
    ) -> Result<Vec<UnifiedPrComment>, GlabCliError> {
        let endpoint = format!(
            "projects/{}/merge_requests/{}/discussions?per_page=100",
            repo_info.encoded_path(),
            mr_iid
        );
        let raw = self.run(["api", "--hostname", &repo_info.host, &endpoint], None)?;
        Self::parse_mr_discussions(&raw)
    }

    /// Parse MR URL to extract the repo spec and MR iid.
    ///
    /// Format: `https://{host}/{group...}/{project}/-/merge_requests/{iid}`
    pub fn parse_mr_url(url: &str) -> Option<(String, i64)> {
        let (repo_part, iid_part) = url.split_once("/-/merge_requests/")?;
        let iid: i64 = iid_part
            .split(|c: char| !c.is_ascii_digit())
            .next()?
            .parse()
            .ok()?;
        let info = GitLabRepoInfo::from_remote_url(repo_part)?;
        Some((info.repo_spec(), iid))
    }
}

impl GlabCli {
    /// Parse the text output of `glab mr create`, which prints the MR URL.
    fn parse_mr_create_text(
        raw: &str,
        request: &CreatePrRequest,
    ) -> Result<PullRequestDetail, GlabCliError> {
        let url = raw
            .split_whitespace()
            .find(|token| token.starts_with("http") && token.contains("/-/merge_requests/"))
            .map(|token| token.trim_end_matches(['.', ',']).to_string())
            .ok_or_else(|| {
                GlabCliError::UnexpectedOutput(format!(
                    "glab mr create did not return a merge request URL; raw output: {raw}"
                ))
            })?;

        let (_, iid) = Self::parse_mr_url(&url).ok_or_else(|| {
            GlabCliError::UnexpectedOutput(format!(
                "Failed to parse merge request number from URL '{url}'"
            ))
        })?;

        Ok(PullRequestDetail {
            number: iid,
            url,
            status: MergeStatus::Open,
            merged_at: None,
            merge_commit_sha: None,
            title: request.title.clone(),
            base_branch: request.base_branch.clone(),
            head_branch: request.head_branch.clone(),
        })
    }

    /// Parse a single MR JSON object from `glab mr view --output json`.
    fn parse_mr_response(raw: &str) -> Result<PullRequestDetail, GlabCliError> {
        let mr: GlabMrResponse = serde_json::from_str(raw.trim()).map_err(|e| {
            GlabCliError::UnexpectedOutput(format!("Failed to parse MR response: {e}; raw: {raw}"))
        })?;
        Ok(Self::mr_to_detail(mr))
    }

    fn parse_mr_list_response(raw: &str) -> Result<Vec<PullRequestDetail>, GlabCliError> {
        let mrs: Vec<GlabMrResponse> = serde_json::from_str(raw.trim()).map_err(|e| {
            GlabCliError::UnexpectedOutput(format!("Failed to parse MR list: {e}; raw: {raw}"))
        })?;
        Ok(mrs.into_iter().map(Self::mr_to_detail).collect())
    }

    fn mr_to_detail(mr: GlabMrResponse) -> PullRequestDetail {
        let status = mr.state.as_deref().unwrap_or("opened");
        let merged_at = mr
            .merged_at
            .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
            .map(|dt| dt.with_timezone(&Utc));
        let merge_commit_sha = mr.merge_commit_sha.or(mr.squash_commit_sha);

        PullRequestDetail {
            number: mr.iid,
            url: mr.web_url,
            status: Self::map_gitlab_state(status),
            merged_at,
            merge_commit_sha,
            title: mr.title.unwrap_or_default(),
            base_branch: mr.target_branch.unwrap_or_default(),
            head_branch: mr.source_branch.unwrap_or_default(),
        }
    }

    fn parse_mr_discussions(raw: &str) -> Result<Vec<UnifiedPrComment>, GlabCliError> {
        let discussions: Vec<GlabDiscussion> = serde_json::from_str(raw.trim()).map_err(|e| {
            GlabCliError::UnexpectedOutput(format!("Failed to parse discussions: {e}; raw: {raw}"))
        })?;

        let mut comments = Vec::new();

        for discussion in discussions {
            let Some(notes) = discussion.notes else {
                continue;
            };
            for note in notes {
                // Skip system-generated notes (branch pushes, status changes, ...)
                if note.system.unwrap_or(false) {
                    continue;
                }

                let id = note.id.unwrap_or(0);
                let author = note
                    .author
                    .and_then(|a| a.username)
                    .unwrap_or_else(|| "unknown".to_string());
                let body = note.body.unwrap_or_default();
                let created_at = note
                    .created_at
                    .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(Utc::now);

                let position = note.position;
                let path = position
                    .as_ref()
                    .and_then(|p| p.new_path.clone().or_else(|| p.old_path.clone()));

                if let Some(path) = path {
                    let (line, side) = match position.as_ref() {
                        Some(p) if p.new_line.is_some() => (p.new_line, Some("RIGHT".to_string())),
                        Some(p) => (p.old_line, Some("LEFT".to_string())),
                        None => (None, None),
                    };
                    comments.push(UnifiedPrComment::Review {
                        id,
                        author,
                        author_association: None,
                        body,
                        created_at,
                        url: None,
                        path,
                        line,
                        side,
                        diff_hunk: None,
                    });
                } else {
                    comments.push(UnifiedPrComment::General {
                        id: id.to_string(),
                        author,
                        author_association: None,
                        body,
                        created_at,
                        url: None,
                    });
                }
            }
        }

        comments.sort_by_key(|c| c.created_at());
        Ok(comments)
    }

    /// Map GitLab MR state to MergeStatus
    fn map_gitlab_state(state: &str) -> MergeStatus {
        match state.to_lowercase().as_str() {
            "opened" | "locked" => MergeStatus::Open,
            "merged" => MergeStatus::Merged,
            "closed" => MergeStatus::Closed,
            _ => MergeStatus::Unknown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_repo_info_from_https() {
        let info = GitLabRepoInfo::from_remote_url("https://gitlab.com/group/project.git").unwrap();
        assert_eq!(info.host, "gitlab.com");
        assert_eq!(info.full_path, "group/project");
    }

    #[test]
    fn test_repo_info_from_https_subgroup() {
        let info = GitLabRepoInfo::from_remote_url("https://gitlab.example.com/group/sub/project")
            .unwrap();
        assert_eq!(info.host, "gitlab.example.com");
        assert_eq!(info.full_path, "group/sub/project");
        assert_eq!(info.encoded_path(), "group%2Fsub%2Fproject");
    }

    #[test]
    fn test_repo_info_from_scp_style() {
        let info = GitLabRepoInfo::from_remote_url("git@gitlab.com:group/project.git").unwrap();
        assert_eq!(info.host, "gitlab.com");
        assert_eq!(info.full_path, "group/project");
    }

    #[test]
    fn test_repo_info_from_ssh_url() {
        let info =
            GitLabRepoInfo::from_remote_url("ssh://git@gitlab.example.com:2222/group/project.git")
                .unwrap();
        assert_eq!(info.host, "gitlab.example.com");
        assert_eq!(info.full_path, "group/project");
    }

    #[test]
    fn test_repo_info_invalid() {
        assert!(GitLabRepoInfo::from_remote_url("not a url").is_none());
        assert!(GitLabRepoInfo::from_remote_url("https://gitlab.com").is_none());
    }

    #[test]
    fn test_parse_mr_url() {
        let (spec, iid) =
            GlabCli::parse_mr_url("https://gitlab.com/group/sub/project/-/merge_requests/42")
                .unwrap();
        assert_eq!(spec, "https://gitlab.com/group/sub/project");
        assert_eq!(iid, 42);
    }

    #[test]
    fn test_parse_mr_url_with_suffix() {
        let (spec, iid) =
            GlabCli::parse_mr_url("https://gitlab.example.com/g/p/-/merge_requests/7/diffs")
                .unwrap();
        assert_eq!(spec, "https://gitlab.example.com/g/p");
        assert_eq!(iid, 7);
    }

    #[test]
    fn test_parse_mr_url_invalid() {
        assert!(GlabCli::parse_mr_url("https://github.com/owner/repo/pull/123").is_none());
        assert!(GlabCli::parse_mr_url("https://gitlab.com/group/project").is_none());
    }

    #[test]
    fn test_map_gitlab_state() {
        assert!(matches!(
            GlabCli::map_gitlab_state("opened"),
            MergeStatus::Open
        ));
        assert!(matches!(
            GlabCli::map_gitlab_state("merged"),
            MergeStatus::Merged
        ));
        assert!(matches!(
            GlabCli::map_gitlab_state("closed"),
            MergeStatus::Closed
        ));
        assert!(matches!(
            GlabCli::map_gitlab_state("weird"),
            MergeStatus::Unknown
        ));
    }

    #[test]
    fn test_parse_mr_create_text() {
        let request = CreatePrRequest {
            title: "My change".to_string(),
            body: None,
            head_branch: "feature".to_string(),
            base_branch: "main".to_string(),
            draft: None,
            head_repo_url: None,
        };
        let raw = "Creating merge request for feature into main in group/project\n\nhttps://gitlab.com/group/project/-/merge_requests/12\n";
        let detail = GlabCli::parse_mr_create_text(raw, &request).unwrap();
        assert_eq!(detail.number, 12);
        assert_eq!(
            detail.url,
            "https://gitlab.com/group/project/-/merge_requests/12"
        );
        assert!(matches!(detail.status, MergeStatus::Open));
        assert_eq!(detail.head_branch, "feature");
    }
}

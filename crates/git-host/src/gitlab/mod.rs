//! GitLab hosting service implementation (merge requests via `glab`).

mod cli;

use std::{path::Path, time::Duration};

use async_trait::async_trait;
use backon::{ExponentialBuilder, Retryable};
pub use cli::GlabCli;
use cli::{GitLabRepoInfo, GlabCliError};
use tokio::task;
use tracing::info;

use crate::{
    GitHostProvider,
    types::{CreatePrRequest, GitHostError, ProviderKind, PullRequestDetail, UnifiedPrComment},
};

#[derive(Debug, Clone)]
pub struct GitLabProvider {
    glab_cli: GlabCli,
}

impl GitLabProvider {
    pub fn new() -> Result<Self, GitHostError> {
        Ok(Self {
            glab_cli: GlabCli::new(),
        })
    }

    fn get_repo_info(&self, remote_url: &str) -> Result<GitLabRepoInfo, GitHostError> {
        GitLabRepoInfo::from_remote_url(remote_url).ok_or_else(|| {
            GitHostError::Repository(format!(
                "Could not parse GitLab host and project path from remote URL: {remote_url}"
            ))
        })
    }

    fn retry_policy() -> ExponentialBuilder {
        ExponentialBuilder::default()
            .with_min_delay(Duration::from_secs(1))
            .with_max_delay(Duration::from_secs(30))
            .with_max_times(3)
            .with_jitter()
    }
}

impl From<GlabCliError> for GitHostError {
    fn from(error: GlabCliError) -> Self {
        match &error {
            GlabCliError::AuthFailed(msg) => GitHostError::AuthFailed(msg.clone()),
            GlabCliError::NotAvailable => GitHostError::CliNotInstalled {
                provider: ProviderKind::GitLab,
            },
            GlabCliError::CommandFailed(msg) => {
                let lower = msg.to_ascii_lowercase();
                if lower.contains("403") || lower.contains("forbidden") {
                    GitHostError::InsufficientPermissions(msg.clone())
                } else if lower.contains("404") || lower.contains("not found") {
                    GitHostError::RepoNotFoundOrNoAccess(msg.clone())
                } else if lower.contains("not a git repository") {
                    GitHostError::NotAGitRepository(msg.clone())
                } else {
                    GitHostError::PullRequest(msg.clone())
                }
            }
            GlabCliError::UnexpectedOutput(msg) => GitHostError::UnexpectedOutput(msg.clone()),
        }
    }
}

#[async_trait]
impl GitHostProvider for GitLabProvider {
    async fn create_pr(
        &self,
        repo_path: &Path,
        remote_url: &str,
        request: &CreatePrRequest,
    ) -> Result<PullRequestDetail, GitHostError> {
        let repo_info = self.get_repo_info(remote_url)?;

        (|| async {
            let cli = self.glab_cli.clone();
            let request_clone = request.clone();
            let repo_info = repo_info.clone();
            let path = repo_path.to_path_buf();

            let cli_result =
                task::spawn_blocking(move || cli.create_mr(&request_clone, &repo_info, &path))
                    .await
                    .map_err(|err| {
                        GitHostError::PullRequest(format!(
                            "Failed to execute GitLab CLI for MR creation: {err}"
                        ))
                    })?
                    .map_err(GitHostError::from)?;

            info!(
                "Created GitLab MR !{} for branch {}",
                cli_result.number, request.head_branch
            );

            Ok(cli_result)
        })
        .retry(&Self::retry_policy())
        .when(|e: &GitHostError| e.should_retry())
        .notify(|err: &GitHostError, dur: Duration| {
            tracing::warn!(
                "GitLab API call failed, retrying after {:.2}s: {}",
                dur.as_secs_f64(),
                err
            );
        })
        .await
    }

    async fn get_pr_status(&self, pr_url: &str) -> Result<PullRequestDetail, GitHostError> {
        (|| async {
            let cli = self.glab_cli.clone();
            let url = pr_url.to_string();

            let mr = task::spawn_blocking(move || cli.view_mr(&url))
                .await
                .map_err(|err| {
                    GitHostError::PullRequest(format!(
                        "Failed to execute GitLab CLI for viewing MR: {err}"
                    ))
                })?;
            mr.map_err(GitHostError::from)
        })
        .retry(&Self::retry_policy())
        .when(|err: &GitHostError| err.should_retry())
        .notify(|err: &GitHostError, dur: Duration| {
            tracing::warn!(
                "GitLab API call failed, retrying after {:.2}s: {}",
                dur.as_secs_f64(),
                err
            );
        })
        .await
    }

    async fn list_prs_for_branch(
        &self,
        _repo_path: &Path,
        remote_url: &str,
        branch_name: &str,
    ) -> Result<Vec<PullRequestDetail>, GitHostError> {
        let repo_info = self.get_repo_info(remote_url)?;

        (|| async {
            let cli = self.glab_cli.clone();
            let repo_info = repo_info.clone();
            let branch = branch_name.to_string();

            let mrs = task::spawn_blocking(move || cli.list_mrs_for_branch(&repo_info, &branch))
                .await
                .map_err(|err| {
                    GitHostError::PullRequest(format!(
                        "Failed to execute GitLab CLI for listing MRs: {err}"
                    ))
                })?;
            mrs.map_err(GitHostError::from)
        })
        .retry(&Self::retry_policy())
        .when(|e: &GitHostError| e.should_retry())
        .notify(|err: &GitHostError, dur: Duration| {
            tracing::warn!(
                "GitLab API call failed, retrying after {:.2}s: {}",
                dur.as_secs_f64(),
                err
            );
        })
        .await
    }

    async fn get_pr_comments(
        &self,
        _repo_path: &Path,
        remote_url: &str,
        pr_number: i64,
    ) -> Result<Vec<UnifiedPrComment>, GitHostError> {
        let repo_info = self.get_repo_info(remote_url)?;

        (|| async {
            let cli = self.glab_cli.clone();
            let repo_info = repo_info.clone();

            let comments =
                task::spawn_blocking(move || cli.get_mr_discussions(&repo_info, pr_number))
                    .await
                    .map_err(|err| {
                        GitHostError::PullRequest(format!(
                            "Failed to execute GitLab CLI for fetching MR comments: {err}"
                        ))
                    })?;
            comments.map_err(GitHostError::from)
        })
        .retry(&Self::retry_policy())
        .when(|e: &GitHostError| e.should_retry())
        .notify(|err: &GitHostError, dur: Duration| {
            tracing::warn!(
                "GitLab API call failed, retrying after {:.2}s: {}",
                dur.as_secs_f64(),
                err
            );
        })
        .await
    }

    async fn list_open_prs(
        &self,
        _repo_path: &Path,
        remote_url: &str,
    ) -> Result<Vec<PullRequestDetail>, GitHostError> {
        let repo_info = self.get_repo_info(remote_url)?;

        (|| async {
            let cli = self.glab_cli.clone();
            let repo_info = repo_info.clone();

            let mrs = task::spawn_blocking(move || cli.list_open_mrs(&repo_info))
                .await
                .map_err(|err| {
                    GitHostError::PullRequest(format!(
                        "Failed to execute GitLab CLI for listing open MRs: {err}"
                    ))
                })?;
            mrs.map_err(GitHostError::from)
        })
        .retry(&Self::retry_policy())
        .when(|e: &GitHostError| e.should_retry())
        .notify(|err: &GitHostError, dur: Duration| {
            tracing::warn!(
                "GitLab API call failed, retrying after {:.2}s: {}",
                dur.as_secs_f64(),
                err
            );
        })
        .await
    }

    fn provider_kind(&self) -> ProviderKind {
        ProviderKind::GitLab
    }
}

//! Hosting services and issue trackers: sign-in, pull requests, issues, publishing.

use super::*;
use crate::{hosting, ai};

#[derive(Serialize)]
pub(crate) struct HostingStatus {
    github: Option<String>,
    gitlab: Option<String>,
    github_error: Option<String>,
    gitlab_error: Option<String>,
    ai: bool,
}

/// The hosting service of a repository's origin (or first) remote, if it's GitHub or GitLab.
pub(crate) fn remote_of(path: &str, gitlab_host: &str) -> Result<hosting::RemoteRepo, String> {
    let dir = Path::new(path);
    let mut url = git::run(dir, &["remote", "get-url", "origin"]).stdout.trim().to_string();
    if url.is_empty() {
        if let Some(first) = git::run(dir, &["remote"]).stdout.lines().next() {
            url = git::run(dir, &["remote", "get-url", first]).stdout.trim().to_string();
        }
    }
    hosting::parse_remote(&url, gitlab_host).ok_or_else(|| "This repository's remote isn't on a connected service (Preferences > Integrations).".to_string())
}

#[tauri::command]
pub(crate) async fn hosting_status(gitlab_host: String) -> Result<HostingStatus, String> {
    blocking(move || {
        let check = |p: hosting::Provider, name: &str| -> (Option<String>, Option<String>) {
            if hosting::secret_get(name).is_none() {
                return (None, None);
            }
            match hosting::whoami(&p, &gitlab_host) {
                Ok(u) => (Some(u), None),
                Err(e) => (None, Some(e)),
            }
        };
        let (github, github_error) = check(hosting::Provider::Github, "github-token");
        let (gitlab, gitlab_error) = check(hosting::Provider::Gitlab, "gitlab-token");
        HostingStatus { github, gitlab, github_error, gitlab_error, ai: ai::configured() }
    })
    .await
}

/// Store (or with an empty value, remove) a token or API key in the OS credential store.
#[tauri::command]
pub(crate) fn set_secret(name: String, value: String) -> Result<(), String> {
    let allowed = hosting::SERVICES.iter().any(|id| hosting::secret_name(id) == name) || ["anthropic-key", "openai-key", "google-key", "ollama-key", "custom-ai-key"].contains(&name.as_str());
    if !allowed {
        return Err("unknown secret".into());
    }
    hosting::secret_set(&name, value.trim())?;
    if name.ends_with("-token") { hosting::sync_git_credentials(); }
    Ok(())
}

#[tauri::command]
pub(crate) async fn repo_remote(path: String, gitlab_host: String, read_id: Option<String>) -> Result<Option<hosting::RemoteRepo>, String> {
    read_blocking(read_id, move || remote_of(&path, &gitlab_host).ok()).await
}

#[tauri::command]
pub(crate) async fn list_prs_page(path: String, gitlab_host: String, cursor: Option<String>, read_id: Option<String>) -> Result<pagination::Page<Vec<hosting::PullRequest>>, String> {
    blocking(move || read_jobs::scope(read_id, || hosting::pull_requests_page(&remote_of(&path, &gitlab_host)?, cursor.as_deref()))).await?
}

#[tauri::command]
pub(crate) async fn list_prs(path: String, gitlab_host: String, read_id: Option<String>) -> Result<Vec<hosting::PullRequest>, String> {
    read_blocking(read_id, move || hosting::pull_requests(&remote_of(&path, &gitlab_host)?)).await?
}

#[tauri::command]
pub(crate) async fn pr_details(path: String, gitlab_host: String, number: u64, read_id: Option<String>) -> Result<hosting::PullRequest, String> {
    read_blocking(read_id, move || hosting::pull_request(&remote_of(&path, &gitlab_host)?, number)).await?
}

#[tauri::command]
pub(crate) async fn pr_checkout_target(path: String, gitlab_host: String, number: u64, read_id: Option<String>) -> Result<hosting::PrCheckout, String> {
    read_blocking(read_id, move || {
        let remote = remote_of(&path, &gitlab_host)?;
        hosting::pr_checkout(&remote, &hosting::pull_request(&remote, number)?)
    }).await?
}

/// Issues for a repository: from its own issue tracker choice (Preferences > Issue Tracker), else its host.
#[tauri::command]
pub(crate) async fn list_issues_page(path: String, gitlab_host: String, tracker: Option<hosting::Tracker>, cursor: Option<String>, read_id: Option<String>) -> Result<pagination::Page<Vec<hosting::Issue>>, String> {
    blocking(move || read_jobs::scope(read_id, || pagination::one_page(cursor, || match tracker.filter(|t| !t.kind.is_empty()) {
        Some(t) => hosting::tracker_issues(&t, false),
        None => hosting::issues(&remote_of(&path, &gitlab_host)?),
    }))).await?
}

#[tauri::command]
pub(crate) async fn list_issues(path: String, gitlab_host: String, tracker: Option<hosting::Tracker>, read_id: Option<String>) -> Result<Vec<hosting::Issue>, String> {
    read_blocking(read_id, move || match tracker.filter(|t| !t.kind.is_empty()) {
        Some(t) => hosting::tracker_issues(&t, false),
        None => hosting::issues(&remote_of(&path, &gitlab_host)?),
    })
    .await?
}

#[tauri::command]
pub(crate) async fn create_issue(path: Option<String>, gitlab_host: String, tracker: Option<hosting::Tracker>, title: String, body: String) -> Result<hosting::Issue, String> {
    blocking(move || {
        let r = path.as_deref().and_then(|p| remote_of(p, &gitlab_host).ok());
        hosting::create_issue(r.as_ref(), &tracker.unwrap_or_default(), &title, &body)
    })
    .await?
}

#[tauri::command]
pub(crate) async fn integration_statuses(ids: Vec<String>) -> Result<Vec<hosting::ServiceStatus>, String> {
    blocking(move || {
        let handles: Vec<_> = ids.into_iter().map(|id| std::thread::spawn(move || hosting::status(&id))).collect();
        handles.into_iter().filter_map(|h| h.join().ok()).collect()
    })
    .await
}

/// "Connect": browser sign-in through Git Credential Manager, or the GitHub CLI's sign-in ("gh").
#[tauri::command]
pub(crate) async fn integration_sign_in(id: String, method: String) -> Result<String, String> {
    blocking(move || {
        let token = if method == "gh" { hosting::gh_cli_token()? } else { hosting::browser_sign_in(&id)? };
        hosting::secret_set(&hosting::secret_name(&id), &token)?;
        hosting::sync_git_credentials();
        hosting::whoami_id(&id)
    })
    .await?
}

#[tauri::command]
pub(crate) async fn integration_repos_page(id: String, cursor: Option<String>, read_id: Option<String>) -> Result<pagination::Page<Vec<hosting::HostedRepo>>, String> {
    blocking(move || read_jobs::scope(read_id, || pagination::one_page(cursor, || hosting::repositories(&id)))).await?
}

#[tauri::command]
pub(crate) async fn integration_repos(id: String, read_id: Option<String>) -> Result<Vec<hosting::HostedRepo>, String> {
    read_blocking(read_id, move || hosting::repositories(&id)).await?
}

#[tauri::command]
pub(crate) async fn publish_targets(service: String) -> Result<Vec<publishing::Target>, String> {
    blocking(move || publishing::targets(&service)).await?
}

#[tauri::command]
pub(crate) async fn publish_repository(path: String, request: publishing::PublishRequest, state: State<'_, AppState>) -> Result<publishing::PublishResult, String> {
    let engine=state.engine.clone();
    blocking(move || publishing::publish(Path::new(&path), &request, &engine.jobs)).await?
}

#[tauri::command]
pub(crate) async fn resume_publish(path: String, request: publishing::PublishRequest, url: String, state: State<'_, AppState>) -> Result<publishing::PublishResult, String> {
    let engine=state.engine.clone();
    blocking(move || publishing::resume(Path::new(&path), &request, &url, &engine.jobs)).await
}

#[tauri::command]
pub(crate) async fn integration_add_ssh_key(id: String, title: String, key: String) -> Result<(), String> {
    blocking(move || hosting::add_ssh_key(&id, &title, &key)).await?
}

#[tauri::command]
pub(crate) fn integration_ssh_page(id: String) -> String {
    hosting::ssh_keys_page(&id)
}

#[tauri::command]
pub(crate) async fn pr_comments_page(path: String, gitlab_host: String, number: u64, cursor: Option<String>, read_id: Option<String>) -> Result<pagination::Page<Vec<hosting::Comment>>, String> {
    blocking(move || read_jobs::scope(read_id, || pagination::one_page(cursor, || hosting::comments(&remote_of(&path, &gitlab_host)?, number)))).await?
}

#[tauri::command]
pub(crate) async fn launchpad_page(service: String, cursor: Option<String>, read_id: Option<String>) -> Result<pagination::Page<hosting::Launchpad>, String> {
    blocking(move || read_jobs::scope(read_id, || hosting::launchpad_page(&service, cursor.as_deref()))).await?
}

#[tauri::command]
pub(crate) async fn pr_comments(path: String, gitlab_host: String, number: u64, read_id: Option<String>) -> Result<Vec<hosting::Comment>, String> {
    read_blocking(read_id, move || hosting::comments(&remote_of(&path, &gitlab_host)?, number)).await?
}

#[tauri::command]
pub(crate) async fn pr_comment(path: String, gitlab_host: String, number: u64, body: String) -> Result<(), String> {
    blocking(move || hosting::add_comment(&remote_of(&path, &gitlab_host)?, number, &body)).await?
}

#[tauri::command]
pub(crate) async fn create_pr(path: String, gitlab_host: String, source: String, target: String, title: String, body: String, draft: bool) -> Result<hosting::PullRequest, String> {
    blocking(move || {
        let (remote, source) = hosting::pull_request_destination(Path::new(&path), &source, &gitlab_host)?;
        hosting::create_pr(&remote, &source, &target, &title, &body, draft)
    }).await?
}

#[tauri::command]
pub(crate) async fn cancel_pr_merge(path: String, gitlab_host: String, number: u64) -> Result<bool, String> {
    blocking(move || Ok(hosting_api::cancel_merge(&remote_of(&path, &gitlab_host)?, number))).await?
}

#[tauri::command]
pub(crate) async fn merge_pr(path: String, gitlab_host: String, number: u64, method: String) -> Result<(), String> {
    blocking(move || hosting::merge_pr(&remote_of(&path, &gitlab_host)?, number, &method)).await?
}

#[tauri::command]
pub(crate) async fn launchpad(gitlab_host: String, read_id: Option<String>) -> Result<hosting::Launchpad, String> {
    read_blocking(read_id, move || hosting::launchpad(&gitlab_host)).await
}

#[tauri::command]
pub(crate) fn issue_branch_name(number: u64, title: String) -> String {
    hosting::branch_for_issue(number, &title)
}

#[tauri::command]
pub(crate) fn open_url(url: String) -> Result<(), String> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("Only web links can be opened".into());
    }
    // Open a web link with the system: ShellExecuteExW "open".
    shell::open_external(&url)
}

/// Hand the tokens of connected services to Git (HTTPS push / pull / fetch / clone).
#[tauri::command]
pub(crate) async fn sync_git_credentials() -> Result<(), String> {
    blocking(hosting::sync_git_credentials).await
}

#[tauri::command]
pub(crate) async fn remote_auth(path: String, gitlab_host: String, read_id: Option<String>) -> Result<hosting::RemoteAuth, String> {
    read_blocking(read_id, move || hosting::remote_auth(Path::new(&path), &gitlab_host)).await
}

#[tauri::command]
pub(crate) async fn pr_template(path: String) -> Result<Option<String>, String> {
    blocking(move || extras::pr_template(Path::new(&path))).await
}

//! AI helpers and coding-agent sessions.

use super::*;
use crate::{ai, settings};

#[tauri::command]
pub(crate) async fn ai_commit_message(path: String, sha: Option<String>) -> Result<(String, String), String> {
    blocking(move || ai::commit_message(Path::new(&path), sha.as_deref())).await?
}

#[tauri::command]
pub(crate) async fn ai_stash_message(path: String) -> Result<String, String> {
    blocking(move || ai::stash_message(Path::new(&path))).await?
}

#[tauri::command]
pub(crate) async fn ai_explain_commit(path: String, sha: String) -> Result<String, String> {
    blocking(move || ai::explain_commit(Path::new(&path), &sha)).await?
}

#[tauri::command]
pub(crate) async fn ai_explain_branch(path: String, branch: String, base: Option<String>) -> Result<String, String> {
    blocking(move || {
        let base = base.or_else(|| extras::default_target(Path::new(&path))).ok_or("No branch to compare with")?;
        ai::explain_branch(Path::new(&path), &branch, &base)
    })
    .await?
}

#[tauri::command]
pub(crate) async fn ai_test_connection() -> Result<String, String> {
    blocking(ai::test_connection).await?
}

#[tauri::command]
pub(crate) async fn ai_list_models() -> Result<Vec<ai_models::Model>, String> {
    blocking(|| ai_models::list(&settings::current().ai)).await?
}

#[tauri::command]
pub(crate) async fn ai_find_program(provider: String) -> Result<ai_cli::DetectedProgram, String> {
    let agent = ai_cli::Agent::from_provider(&provider).ok_or("This provider does not use a local program.")?;
    blocking(move || ai_cli::detect_program(agent)).await?
}

#[tauri::command]
pub(crate) async fn ai_pull_request(path: String, source: String, target: String) -> Result<(String, String), String> {
    blocking(move || ai::pull_request_text(Path::new(&path), &source, &target)).await?
}

#[tauri::command]
pub(crate) async fn ai_resolve_conflict(base: String, ours: String, theirs: String, file: String) -> Result<String, String> {
    blocking(move || ai::resolve_conflict(&base, &ours, &theirs, &file)).await?
}

#[tauri::command]
pub(crate) async fn ai_compose(path: String) -> Result<Vec<ai::ComposedCommit>, String> {
    blocking(move || ai::compose(Path::new(&path))).await?
}

#[tauri::command]
pub(crate) async fn agent_sessions(path: String, paths: Option<Vec<String>>, claude_dir: Option<String>, codex_dir: Option<String>) -> Result<Vec<agents::AgentSession>, String> {
    blocking(move || {
        let mut roots = paths.unwrap_or_default();
        if !roots.contains(&path) { roots.push(path); }
        agents::sessions(&roots, claude_dir.as_deref(), codex_dir.as_deref())
    }).await
}

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod accounts;
mod actions;
mod jobs;
mod files;
mod git;
mod graph;
mod ai;
mod ai_context;
mod operation;
mod pagination;
mod process_tree;
mod read_jobs;
mod agents;
mod commands;
mod approvals;
mod ai_cli;
mod ai_models;
mod extras;
mod external;
mod prefs;
mod hosting;
mod hosting_api;
mod publishing;
mod manage;
mod rewrite;
mod metrics;
mod net;
mod sample;
mod settings;
mod shell;
mod shot;
mod terminal;
mod updates;
mod watch;
mod watch_plan;
#[cfg(test)]
mod regressions;

use actions::{Action, ActionResult, Engine, UndoInfo};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{Emitter, Manager, State};

use metrics::INITIAL_COMMITS;
use commands::app::{take_and_exit, AppState};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    // Git's credential helper mode (see git::credential_helper): answer and exit before any window opens.
    if args.get(1).map(String::as_str) == Some("--git-credential") {
        std::process::exit(hosting::git_credential_main(args.get(2).map(String::as_str).unwrap_or_default()));
    }
    let shot_view = args.iter().position(|a| a == "--shot-view").and_then(|i| args.get(i + 1)).cloned();
    let screenshot = args.iter().position(|a| a == "--screenshot").and_then(|i| args.get(i + 1)).map(|p| {
        let p = PathBuf::from(p);
        if p.is_absolute() {
            p
        } else {
            std::env::current_dir().unwrap_or_default().join(p)
        }
    });

    // The back end reads its settings from the saved file; screenshots always start from defaults.
    if screenshot.is_none() {
        settings::load();
        approvals::load(&settings::current());
        std::thread::spawn(hosting::sync_git_credentials);
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(updates::Updates::default())
        .plugin(tauri_plugin_process::init())
        .manage(AppState { engine: Arc::new(Engine::default()), screenshot: screenshot.clone(), shot_view, watchers: Arc::new(Mutex::new(HashMap::new())), terminals: terminal::Terminals::default() })
        .setup(move |app| {
            // Long jobs report progress to the window ("job-progress" events).
            let handle = app.handle().clone();
            let engine = app.state::<AppState>().engine.clone();
            engine.jobs.set_listener(std::sync::Arc::new(move |id: &str, text: &str, pct: Option<u32>| {
                let _ = handle.emit("job-progress", serde_json::json!({ "id": id, "text": text, "percent": pct }));
            }));
            // Test aid: `--shot-size 1920x1040` opens the window at that size (to check wide layouts).
            let args: Vec<String> = std::env::args().collect();
            if let Some((w, h)) = args
                .iter()
                .position(|a| a == "--shot-size")
                .and_then(|i| args.get(i + 1))
                .and_then(|s| s.split_once('x'))
                .and_then(|(w, h)| Some((w.parse::<f64>().ok()?, h.parse::<f64>().ok()?)))
            {
                if let Some(win) = app.get_webview_window("main") {
                    let _ = win.set_size(tauri::LogicalSize::new(w, h));
                    let _ = win.center();
                }
            }
            if let Some(out) = screenshot.clone().filter(|_| !args.windows(2).any(|a| a[0] == "--shot-view" && a[1] == "validation")) {
                // Safety net: take the picture anyway if the page never reports ready.
                let handle = app.handle().clone();
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_secs(40));
                    if let Some(w) = handle.get_webview_window("main") {
                        let h2 = handle.clone();
                        let _ = handle.run_on_main_thread(move || take_and_exit(&w, &out, &h2));
                    }
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app::launch_options,
            commands::repo::open_repo,
            commands::repo::history_page,
            commands::repo::commit_files,
            commands::repo::commit_details,
            commands::files::file_diff,
            commands::files::file_versions,
            commands::files::save_file,
            commands::files::file_history,
            commands::files::blame,
            commands::files::commit_stats,
            commands::files::range_files,
            commands::files::tracked_files,
            commands::files::open_in_editor,
            commands::repo::rebase_commits,
            commands::repo::rebase_plan,
            commands::settings::read_ssh_public_key,
            commands::repo::gitflow_config,
            updates::update_config,
            updates::update_check,
            updates::update_download,
            updates::update_install,
            commands::ai::ai_commit_message,
            commands::ai::ai_stash_message,
            commands::ai::ai_explain_commit,
            commands::ai::ai_explain_branch,
            commands::ai::ai_compose,
            commands::ai::ai_test_connection,
            commands::ai::ai_list_models,
            commands::ai::ai_find_program,
            commands::ai::ai_pull_request,
            commands::ai::ai_resolve_conflict,
            commands::files::save_patch,
            commands::files::pick_patch_file,
            commands::repo::conflict_check,
            commands::ai::agent_sessions,
            commands::hosting::pr_template,
            commands::repo::repo_summary,
            commands::settings::tool_versions,
            commands::settings::git_config_get,
            commands::settings::gpg_generate_key,
            commands::settings::gpg_export_key,
            commands::settings::forget_credentials,
            commands::repo::delete_orig_files,
            commands::repo::orig_cleanup_candidates,
            commands::files::pick_file,
            commands::hosting::sync_git_credentials,
            commands::hosting::remote_auth,
            commands::repo::repo_status,
            commands::hosting::hosting_status,
            commands::hosting::set_secret,
            commands::hosting::repo_remote,
            commands::hosting::list_prs_page,
            commands::hosting::list_prs,
            commands::hosting::pr_details,
            commands::hosting::pr_checkout_target,
            commands::hosting::list_issues_page,
            commands::hosting::list_issues,
            commands::hosting::pr_comments_page,
            commands::hosting::launchpad_page,
            commands::hosting::pr_comments,
            commands::hosting::pr_comment,
            commands::hosting::create_pr,
            commands::hosting::merge_pr,
            commands::hosting::cancel_pr_merge,
            commands::hosting::launchpad,
            commands::hosting::create_issue,
            commands::hosting::integration_statuses,
            commands::hosting::integration_sign_in,
            commands::hosting::integration_repos_page,
            commands::hosting::integration_repos,
            commands::hosting::publish_targets,
            commands::hosting::publish_repository,
            commands::hosting::resume_publish,
            commands::hosting::integration_add_ssh_key,
            commands::hosting::integration_ssh_page,
            commands::hosting::issue_branch_name,
            commands::hosting::open_url,
            commands::terminal::term_open,
            commands::terminal::term_write,
            commands::terminal::term_ack,
            commands::terminal::term_resize,
            commands::terminal::term_close,
            commands::settings::account_info,
            commands::settings::set_git_config,
            commands::repo::push_review,
            commands::settings::generate_ssh_key,
            commands::repo::cancel_job,
            commands::repo::lfs_info,
            commands::repo::lfs_locks,
            commands::repo::sparse_info,
            commands::repo::gitignore_templates,
            commands::repo::cleanup_list,
            commands::repo::remotes_list,
            commands::repo::submodules_list,
            commands::repo::conflict_sample,
            commands::repo::conflict_info,
            commands::repo::run_action,
            commands::repo::activity_log,
            commands::repo::activity_log_snapshot,
            commands::repo::sample_repo,
            commands::repo::last_commit_message,
            commands::files::pick_folder,
            commands::repo::init_repo,
            commands::repo::clone_repo,
            commands::terminal::open_terminal,
            commands::files::open_file_manager,
            commands::files::open_default,
            commands::files::reveal_file,
            commands::files::file_text,
            commands::settings::load_settings,
            commands::settings::save_settings,
            commands::app::desktop_validation_state,
            commands::app::desktop_validation_accounts,
            commands::repo::acknowledge_job,
            commands::repo::cancel_read,
            commands::repo::watch_repo,
            commands::repo::unwatch_repo,
            commands::app::screenshot_ready
        ])
        .run(tauri::generate_context!())
        .expect("error while running GitCraft");
}

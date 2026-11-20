//! Files: diffs, versions, history, blame, saving, and opening files with other programs.

use super::*;
use crate::files;

#[tauri::command]
pub(crate) async fn file_diff(
    path: String,
    source: git::DiffSource,
    file: String,
    untracked: bool,
    parent: Option<String>,
    ignore_ws: Option<bool>,
    read_id: Option<String>,
) -> Result<Vec<git::DiffLine>, String> {
    blocking(move || read_jobs::scope(read_id, || git::file_diff_opts(Path::new(&path), &source, &file, untracked, parent.as_deref(), ignore_ws.unwrap_or(false)))).await
}

#[tauri::command]
pub(crate) async fn file_versions(path: String, source: git::DiffSource, file: String, untracked: bool, parent: Option<String>, allow_large: Option<bool>, read_id: Option<String>) -> Result<files::FileVersions, String> {
    blocking(move || read_jobs::scope(read_id, || files::versions_with_limit(Path::new(&path), &source, &file, parent.as_deref(), untracked, allow_large.unwrap_or(false)))).await
}

#[tauri::command]
pub(crate) async fn save_file(path: String, file: String, content: String, expected: Option<String>, utf8: Option<bool>) -> Result<(), String> {
    blocking(move || {
        let operation = operation::lock_for(Path::new(&path));
        let _guard = operation.lock().unwrap_or_else(|e| e.into_inner());
        files::save_checked_mode(Path::new(&path), &file, &content, expected.as_deref(), utf8.unwrap_or(false))
    }).await?
}

#[tauri::command]
pub(crate) async fn file_history(path: String, file: String, read_id: Option<String>) -> Result<Vec<files::HistoryEntry>, String> {
    blocking(move || read_jobs::scope(read_id, || files::history(Path::new(&path), &file, metrics::FILE_HISTORY_MAX))).await?
}

#[tauri::command]
pub(crate) async fn blame(path: String, file: String, rev: Option<String>, read_id: Option<String>) -> Result<Vec<files::BlameLine>, String> {
    blocking(move || read_jobs::scope(read_id, || files::blame(Path::new(&path), &file, rev.as_deref()))).await?
}

#[tauri::command]
pub(crate) async fn commit_stats(path: String, shas: Vec<String>, read_id: Option<String>) -> Result<Vec<files::Stats>, String> {
    blocking(move || read_jobs::scope(read_id, || files::stats(Path::new(&path), &shas))).await?
}

#[tauri::command]
pub(crate) async fn range_files(path: String, from: String, to: String, read_id: Option<String>) -> Result<Vec<git::FileChange>, String> {
    blocking(move || read_jobs::scope(read_id, || files::range_files(Path::new(&path), &from, &to))).await?
}

#[tauri::command]
pub(crate) async fn tracked_files(path: String, read_id: Option<String>) -> Result<Vec<String>, String> {
    blocking(move || read_jobs::scope(read_id, || files::tracked_files(Path::new(&path)))).await?
}

/// A native file dialog owned by GitCraft's window. On Windows a dialog without an owner is not tied
/// to the window: it can open behind it and stay on screen after the window has moved on, and the
/// window stays usable while it is open. With an owner it stays on top and blocks the window until
/// it is closed.
fn file_dialog(window: &tauri::Window) -> rfd::FileDialog {
    let dialog = rfd::FileDialog::new();
    #[cfg(windows)]
    let dialog = dialog.set_parent(window);
    #[cfg(not(windows))]
    let _ = window;
    dialog
}

/// Save a patch (uncommitted changes when `shas` is empty, else those commits) to a file the person picks.
#[tauri::command]
pub(crate) async fn save_patch(path: String, shas: Vec<String>, name: String, window: tauri::Window) -> Result<Option<String>, String> {
    let patch = blocking(move || -> Result<_, String> {
        let text = extras::make_patch(Path::new(&path), &shas)?;
        let file = file_dialog(&window).set_title("Save patch").set_file_name(&name).add_filter("Patch", &["patch", "diff"]).save_file();
        Ok(file.map(|file| (file, text)))
    })
    .await??;
    let Some((file, text)) = patch else {
        return Ok(None);
    };
    tokio::fs::write(&file, text).await.map_err(err)?;
    Ok(Some(file.to_string_lossy().into_owned()))
}

#[tauri::command]
pub(crate) async fn pick_patch_file(window: tauri::Window) -> Result<Option<String>, String> {
    blocking(move || file_dialog(&window).set_title("Apply a patch").add_filter("Patch", &["patch", "diff"]).pick_file().map(|p| p.to_string_lossy().into_owned())).await
}

#[tauri::command]
pub(crate) async fn pick_file(title: String, window: tauri::Window) -> Result<Option<String>, String> {
    blocking(move || file_dialog(&window).set_title(&title).pick_file().map(|p| p.to_string_lossy().into_owned())).await
}

/// File > Open Repo in <editor> (Preferences > External Tools > Editor); with `file`, open that file.
#[tauri::command]
pub(crate) fn open_in_editor(path: String, file: Option<String>) -> Result<(), String> {
    external::open_configured_editor(&path, file.as_deref())
}

#[tauri::command]
pub(crate) async fn pick_folder(title: String, window: tauri::Window) -> Result<Option<String>, String> {
    blocking(move || file_dialog(&window).set_title(&title).pick_folder().map(|p| p.to_string_lossy().into_owned())).await
}

#[tauri::command]
pub(crate) fn open_file_manager(path: String) -> Result<(), String> {
    // A folder opens with the "explore" verb.
    shell::open_path(Path::new(&path))
}

/// File menu "Open file in default program": the system opens it with its usual app.
#[tauri::command]
pub(crate) fn open_default(path: String, file: String) -> Result<(), String> {
    let p = files::repo_file(Path::new(&path), &file)?;
    // Default action; "Failed to open path" on failure.
    if !p.exists() {
        return Err(shell::OPEN_PATH_FAILED.into());
    }
    shell::open_path(&p)
}

/// File menu "Show in File Manager": opens the folder with the file selected (its folder when the file is gone).
#[tauri::command]
pub(crate) fn reveal_file(path: String, file: String) -> Result<(), String> {
    let p = files::repo_file(Path::new(&path), &file)?;
    // Opens the folder with the file selected (SHOpenFolderAndSelectItems). For a file
    // that is gone, GitCraft opens the folder it was in instead.
    if !p.exists() {
        return shell::open_path(p.parent().unwrap_or(Path::new(&path)));
    }
    shell::show_item_in_folder(&p)
}

/// File menu "Copy file contents to clipboard": the working copy, or the file as it was in `rev`.
#[tauri::command]
pub(crate) async fn file_text(path: String, file: String, rev: Option<String>, read_id: Option<String>) -> Result<String, String> {
    read_blocking(read_id, move || files::file_text(Path::new(&path), &file, rev.as_deref())).await?
}

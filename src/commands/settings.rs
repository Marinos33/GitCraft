//! Preferences: the settings file, Git config, GPG and SSH keys.

use super::*;
use crate::{hosting, settings};

#[tauri::command]
pub(crate) async fn read_ssh_public_key(path: String) -> Result<String, String> { blocking(move || accounts::read_public_key(&path)).await? }

#[tauri::command]
pub(crate) async fn account_info(path: Option<String>, read_id: Option<String>) -> Result<accounts::AccountInfo, String> {
    read_blocking(read_id, move || accounts::info(Path::new(&path.unwrap_or_else(|| ".".into())))).await
}

#[tauri::command]
pub(crate) async fn set_git_config(path: Option<String>, scope: String, key: String, value: String, window: tauri::Window) -> Result<(), String> {
    if !matches!(scope.as_str(), "global" | "local") { return Err("Unknown Git config scope.".into()); }
    // Other keys can name programs Git runs (core.hooksPath, gpg.program, credential.helper…).
    if !approvals::plain_git_config(&key) {
        let change = if value.is_empty() { format!("git config --{scope} --unset-all {key}") } else { format!("git config --{scope} {key} \"{value}\"") };
        if !approvals::confirm(&window, "Change this Git setting?", &format!("GitCraft will change your Git configuration:\n\n{change}")).await {
            return Err("The Git setting was not changed.".into());
        }
    }
    blocking(move || accounts::set(Path::new(&path.unwrap_or_else(|| ".".into())), &scope, &key, &value)).await?
}

#[tauri::command]
pub(crate) async fn generate_ssh_key(email: String, name: String) -> Result<String, String> {
    blocking(move || accounts::generate_ssh_key(&email, &name)).await?
}

#[tauri::command]
pub(crate) async fn tool_versions() -> Result<prefs::Versions, String> {
    blocking(|| prefs::versions(&prefs::gpg_program())).await
}

#[tauri::command]
pub(crate) async fn git_config_get(path: Option<String>, scope: String, keys: Vec<String>, read_id: Option<String>) -> Result<HashMap<String, String>, String> {
    read_blocking(read_id, move || prefs::config_get(path.as_deref().map(Path::new), &scope, &keys)).await
}

#[tauri::command]
pub(crate) async fn gpg_generate_key(name: String, email: String, passphrase: String) -> Result<String, String> {
    blocking(move || prefs::gpg_generate(&prefs::gpg_program(), &name, &email, &passphrase)).await?
}

#[tauri::command]
pub(crate) async fn gpg_export_key(key: String) -> Result<String, String> {
    blocking(move || prefs::gpg_export(&prefs::gpg_program(), &key)).await?
}

#[tauri::command]
pub(crate) async fn forget_credentials(hosts: Vec<String>) -> Result<usize, String> {
    blocking(move || prefs::forget_credentials(&hosts)).await
}

#[tauri::command]
pub(crate) fn load_settings(state: State<'_, AppState>) -> serde_json::Value {
    if state.screenshot.is_some() {
        return serde_json::Value::Null; // pictures always start from the same state
    }
    settings::read_saved()
}

/// Write the front end's settings file, then apply the part the back end acts on. New commands
/// (custom editor or terminal, Git program, AI program) are confirmed first; refused ones keep
/// their previous values, which are returned for the window to apply.
#[tauri::command]
pub(crate) async fn save_settings(mut settings: serde_json::Value, window: tauri::Window, state: State<'_, AppState>) -> Result<serde_json::Map<String, serde_json::Value>, String> {
    if state.screenshot.is_some() {
        // Pictures keep the defaults; desktop validation runs apply settings in memory only.
        if state.shot_view.as_deref() == Some("validation") { apply_settings(&settings); }
        return Ok(Default::default());
    }
    let previous = settings::read_saved()["settings"].clone();
    let corrections = approvals::review(&window, &mut settings, &previous).await?;
    let f = settings::file();
    if let Some(d) = f.parent() {
        std::fs::create_dir_all(d).map_err(err)?;
    }
    // Replace the settings only after the complete JSON is written, so an interrupted save
    // cannot truncate the last usable preferences file.
    let pending = f.with_extension("json.tmp");
    std::fs::write(&pending, serde_json::to_string_pretty(&settings).map_err(err)?).map_err(err)?;
    std::fs::rename(pending, f).map_err(err)?;
    apply_settings(&settings);
    Ok(corrections)
}

/// Make `saved` (the settings file's content) the settings in effect.
pub(crate) fn apply_settings(saved: &serde_json::Value) {
    let next = settings::Settings::from_saved(saved);
    let hosts_changed = next.integrations != settings::current().integrations;
    settings::set(next);
    if hosts_changed { std::thread::spawn(hosting::sync_git_credentials); }
}

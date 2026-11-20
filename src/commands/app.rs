//! Shared command state and helpers, launch options and test aids.

use super::*;
use crate::{hosting, terminal};

pub(crate) struct WatchSlot {
    pub(crate) read_id: String,
    pub(crate) generation: Arc<()>,
    pub(crate) watcher: Option<watch::RepoWatch>,
}

pub(crate) struct AppState {
    pub(crate) engine: Arc<Engine>,
    pub(crate) screenshot: Option<PathBuf>,
    /// Test aid: which screen `--screenshot` pictures (`--shot-view diff|split|history|start`).
    pub(crate) shot_view: Option<String>,
    pub(crate) watchers: Arc<Mutex<HashMap<String, WatchSlot>>>,
    pub(crate) terminals: terminal::Terminals,
}

#[derive(Serialize)]
pub(crate) struct LaunchOptions {
    pub(crate) screenshot: bool,
    pub(crate) shot_view: Option<String>,
    pub(crate) version: String,
    pub(crate) metrics: metrics::Metrics,
}

pub(crate) fn err<E: ToString>(e: E) -> String {
    e.to_string()
}

pub(crate) async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(err)
}

pub(crate) async fn read_blocking<T: Send + 'static>(id: Option<String>, f: impl FnOnce() -> T + Send + 'static) -> Result<T, String> {
    blocking(move || read_jobs::scope(id, f)).await
}

#[tauri::command]
pub(crate) fn launch_options(state: State<'_, AppState>) -> LaunchOptions {
    LaunchOptions { screenshot: state.screenshot.is_some(), shot_view: state.shot_view.clone(), version: env!("CARGO_PKG_VERSION").to_string(), metrics: metrics::metrics() }
}

#[tauri::command]
pub(crate) fn desktop_validation_state(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    if state.screenshot.is_none() || state.shot_view.as_deref() != Some("validation") { return Err("Desktop validation mode is not active".into()); }
    let watchers = state.watchers.lock().unwrap();
    Ok(serde_json::json!({"read_slots": read_jobs::active_count(), "watchers": watchers.len(), "pending_watches": watchers.values().filter(|s| s.watcher.is_none()).count(), "terminals": state.terminals.pending_output()}))
}

#[tauri::command]
pub(crate) fn desktop_validation_accounts(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    if state.screenshot.is_none() || state.shot_view.as_deref() != Some("validation") { return Err("Desktop validation mode is not active".into()); }
    Ok(serde_json::json!({"gitlab": hosting::secret_get("gitlab-token").is_some(), "gitlab_self": hosting::secret_get("gitlab-self-token").is_some()}))
}

/// Called by the window once it has drawn about 120 frames in --screenshot mode.
#[tauri::command]
pub(crate) fn screenshot_ready(window: tauri::WebviewWindow, state: State<'_, AppState>, app: tauri::AppHandle) -> Result<(), String> {
    let Some(out) = state.screenshot.clone() else { return Ok(()) };
    take_and_exit(&window, &out, &app);
    Ok(())
}

// `window` is only needed to find the window handle on Windows.
#[cfg_attr(not(windows), allow(unused_variables))]
pub(crate) fn take_and_exit(window: &tauri::WebviewWindow, out: &Path, app: &tauri::AppHandle) {
    #[cfg(windows)]
    let res = match window.hwnd() {
        Ok(h) => shot::capture_window(h.0 as isize, out),
        Err(e) => Err(e.to_string()),
    };
    #[cfg(not(windows))]
    let res = shot::capture_window(0, out);
    match res {
        Ok(()) => println!("saved {}", out.display()),
        Err(e) => eprintln!("screenshot failed: {e}"),
    }
    app.exit(0);
}

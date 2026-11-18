//! The in-app terminal and the external terminal.

use super::*;

#[tauri::command]
pub(crate) fn term_open(cwd: String, cols: u16, rows: u16, shell: Option<String>, agent_env: Option<HashMap<String, String>>, app: tauri::AppHandle, state: State<'_, AppState>) -> Result<u32, String> {
    let a1 = app.clone();
    let a2 = app.clone();
    state.terminals.open(
        shell.as_deref().unwrap_or(""),
        &cwd,
        cols,
        rows,
        agent_env.as_ref(),
        std::sync::Arc::new(move |id, data| {
            let _ = a1.emit("term-data", serde_json::json!({ "id": id, "data": data }));
        }),
        std::sync::Arc::new(move |id| {
            let _ = a2.emit("term-exit", id);
        }),
    )
}

#[tauri::command]
pub(crate) fn term_write(id: u32, data: String, state: State<'_, AppState>) -> Result<(), String> {
    state.terminals.write(id, &data)
}

#[tauri::command]
pub(crate) fn term_ack(id: u32, bytes: usize, state: State<'_, AppState>) {
    state.terminals.acknowledge(id, bytes);
}

#[tauri::command]
pub(crate) fn term_resize(id: u32, cols: u16, rows: u16, state: State<'_, AppState>) -> Result<(), String> {
    state.terminals.resize(id, cols, rows)
}

#[tauri::command]
pub(crate) fn term_close(id: u32, state: State<'_, AppState>) {
    state.terminals.close(id)
}

/// File > Open External Terminal (Preferences > External Tools > Terminal).
#[tauri::command]
pub(crate) fn open_terminal(path: String) -> Result<(), String> {
    external::open_configured_terminal(&path)
}

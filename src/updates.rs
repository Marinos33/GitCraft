//! GitCraft updates from GitHub Releases, on the channel chosen in the settings.
//!
//! Stable reads `latest.json` from the newest full release; beta reads it from the rolling `beta`
//! release, which every build of `main` rewrites. Both are verified with the public key built into
//! the app. The window only asks to check, download and install; the address comes from here.

use serde::Serialize;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{Emitter, State};
use tauri_plugin_updater::{Update, UpdaterExt};

#[derive(Default)]
pub struct Updates {
    /// The update the last check offered.
    offered: Mutex<Option<Update>>,
    /// A verified download waiting for "Restart and Install": (version, installer bytes).
    ready: Mutex<Option<(String, Vec<u8>)>>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct UpdateInfo {
    pub version: String,
    pub notes: String,
    pub channel: String,
}

#[derive(Serialize, Clone)]
struct Progress {
    downloaded: u64,
    total: Option<u64>,
}

/// The update manifest for `channel`, from the stable address in the app's configuration.
pub fn endpoint(stable: &str, channel: &str) -> String {
    if channel == "beta" {
        stable.replace("/releases/latest/download/", "/releases/download/beta/")
    } else {
        stable.to_string()
    }
}

fn stable_endpoint(app: &tauri::AppHandle) -> Option<String> {
    let config = app.config().plugins.0.get("updater").cloned().unwrap_or_default();
    let configured = config["pubkey"].as_str().is_some_and(|k| !k.trim().is_empty());
    configured.then(|| config["endpoints"][0].as_str().map(str::to_owned)).flatten()
}

fn channel() -> String {
    if crate::settings::current().update_channel == "beta" { "beta".into() } else { "stable".into() }
}

/// Whether updates are set up in this build, and where the current channel reads from.
#[tauri::command]
pub fn update_config(app: tauri::AppHandle) -> serde_json::Value {
    let stable = stable_endpoint(&app);
    let channel = channel();
    serde_json::json!({ "configured": stable.is_some(), "channel": channel, "endpoint": stable.map(|s| endpoint(&s, &channel)) })
}

/// Look for a newer version on the chosen channel.
#[tauri::command]
pub async fn update_check(app: tauri::AppHandle, state: State<'_, Updates>) -> Result<Option<UpdateInfo>, String> {
    let stable = stable_endpoint(&app).ok_or("Automatic updates aren't set up in this build.")?;
    let channel = channel();
    let url = endpoint(&stable, &channel).parse().map_err(|_| "Invalid update address.")?;
    let updater = app.updater_builder().endpoints(vec![url]).map_err(|e| e.to_string())?.timeout(Duration::from_secs(30)).build().map_err(|e| e.to_string())?;
    let update = updater.check().await.map_err(|e| e.to_string())?;
    let info = update.as_ref().map(|u| UpdateInfo { version: u.version.clone(), notes: u.body.clone().unwrap_or_default(), channel: channel.clone() });
    let ready_version = state.ready.lock().unwrap().as_ref().map(|(v, _)| v.clone());
    if ready_version.is_some() && ready_version != info.as_ref().map(|i| i.version.clone()) {
        // A different version is now offered; the earlier download no longer applies.
        *state.ready.lock().unwrap() = None;
    }
    *state.offered.lock().unwrap() = update;
    Ok(info)
}

/// Download and verify the offered update, reporting `update-progress` events.
#[tauri::command]
pub async fn update_download(app: tauri::AppHandle, state: State<'_, Updates>) -> Result<(), String> {
    let update = state.offered.lock().unwrap().clone().ok_or("No update is available. Check for updates first.")?;
    let mut downloaded = 0u64;
    let window = app.clone();
    let bytes = tokio::time::timeout(
        Duration::from_secs(600),
        update.download(
            move |chunk, total| {
                downloaded += chunk as u64;
                let _ = window.emit("update-progress", Progress { downloaded, total });
            },
            || {},
        ),
    )
    .await
    .map_err(|_| "The update download timed out.".to_string())?
    .map_err(|e| e.to_string())?;
    *state.ready.lock().unwrap() = Some((update.version.clone(), bytes));
    Ok(())
}

/// Install the downloaded update. The window restarts GitCraft afterwards (on Windows the
/// installer closes and reopens it by itself).
#[tauri::command]
pub fn update_install(state: State<'_, Updates>) -> Result<(), String> {
    let update = state.offered.lock().unwrap().clone().ok_or("No update is available.")?;
    let ready = state.ready.lock().unwrap().take().ok_or("The update has not been downloaded yet.")?;
    if ready.0 != update.version {
        return Err("The downloaded update no longer matches the offered version. Check for updates again.".into());
    }
    update.install(&ready.1).map_err(|e| {
        *state.ready.lock().unwrap() = Some(ready.clone());
        e.to_string()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn beta_reads_the_rolling_release_and_stable_the_latest_one() {
        let stable = "https://github.com/MystifiedSky/GitCraft/releases/latest/download/latest.json";
        assert_eq!(endpoint(stable, "stable"), stable);
        assert_eq!(endpoint(stable, "beta"), "https://github.com/MystifiedSky/GitCraft/releases/download/beta/latest.json");
        assert_eq!(endpoint(stable, "nightly"), stable);
    }
}

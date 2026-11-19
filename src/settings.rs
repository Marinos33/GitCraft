//! The preferences the back end acts on, read from GitCraft's settings file.
//!
//! The front end owns the file's layout and writes it through `save_settings`; this module parses
//! the typed subset the back end needs (Git executable, SSH, logging, encodings, integrations, AI,
//! external editor and terminal)
//! when GitCraft starts and after every save. Missing keys take the same defaults as the front
//! end, unknown keys are ignored, and nothing here is pushed from the front end directly.

use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

/// Repo-Specific preferences the back end reads.
#[derive(Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct RepoPrefs {
    /// Encoding label (WHATWG names such as windows-1252); empty means UTF-8.
    pub encoding: Option<String>,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// Experimental > Git Executable: an explicit git program (empty = the one on PATH).
    pub git_path: String,
    /// SSH > Use local SSH agent (and the system's SSH config). When off, `ssh_private_key` is used.
    pub ssh_use_agent: bool,
    pub ssh_private_key: String,
    /// General > Use extended logging in activity log.
    pub extended_log: bool,
    /// Per repository path.
    pub repo_prefs: HashMap<String, RepoPrefs>,
    /// Older self-managed GitLab host, kept when `integrations.gitlab_host` is empty.
    pub gitlab_host: String,
    pub integrations: crate::hosting::Config,
    pub ai: crate::ai::AiConfig,
    /// External Tools: a known editor id, or "custom" for `editor_path` with argument templates.
    pub external_editor: String,
    pub editor_path: String,
    pub editor_file_args: String,
    pub editor_repo_args: String,
    /// External Tools: "default", a known terminal id, or "custom" for `terminal_path`.
    pub external_terminal: String,
    pub terminal_path: String,
    pub terminal_args: String,
    /// GitCraft updates: "stable" (full releases) or "beta" (every build of the main branch).
    pub update_channel: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            git_path: String::new(),
            ssh_use_agent: true,
            ssh_private_key: String::new(),
            extended_log: false,
            repo_prefs: HashMap::new(),
            gitlab_host: String::new(),
            integrations: Default::default(),
            ai: Default::default(),
            external_editor: "vscode".into(),
            editor_path: String::new(),
            editor_file_args: String::new(),
            editor_repo_args: String::new(),
            external_terminal: "default".into(),
            terminal_path: String::new(),
            terminal_args: String::new(),
            update_channel: "stable".into(),
        }
    }
}

impl Settings {
    /// Parse the `settings` object of the saved file. A value of the wrong type falls back to the
    /// defaults for the whole object only if it cannot be read at all.
    pub fn from_saved(saved: &serde_json::Value) -> Self {
        let mut settings: Settings = serde_json::from_value(saved["settings"].clone()).unwrap_or_else(|_| Self::lenient(&saved["settings"]));
        if settings.integrations.gitlab_host.trim().is_empty() {
            settings.integrations.gitlab_host = settings.gitlab_host.clone();
        }
        settings
    }

    /// Read each section on its own, so one malformed section does not reset the others.
    fn lenient(value: &serde_json::Value) -> Self {
        fn field<T: serde::de::DeserializeOwned>(value: &serde_json::Value, key: &str) -> Option<T> {
            serde_json::from_value(value.get(key)?.clone()).ok()
        }
        let d = Settings::default();
        Settings {
            git_path: field(value, "gitPath").unwrap_or(d.git_path),
            ssh_use_agent: field(value, "sshUseAgent").unwrap_or(d.ssh_use_agent),
            ssh_private_key: field(value, "sshPrivateKey").unwrap_or(d.ssh_private_key),
            extended_log: field(value, "extendedLog").unwrap_or(d.extended_log),
            repo_prefs: field(value, "repoPrefs").unwrap_or(d.repo_prefs),
            gitlab_host: field(value, "gitlabHost").unwrap_or(d.gitlab_host),
            integrations: field(value, "integrations").unwrap_or(d.integrations),
            ai: field(value, "ai").unwrap_or(d.ai),
            external_editor: field(value, "externalEditor").unwrap_or(d.external_editor),
            editor_path: field(value, "editorPath").unwrap_or(d.editor_path),
            editor_file_args: field(value, "editorFileArgs").unwrap_or(d.editor_file_args),
            editor_repo_args: field(value, "editorRepoArgs").unwrap_or(d.editor_repo_args),
            external_terminal: field(value, "externalTerminal").unwrap_or(d.external_terminal),
            terminal_path: field(value, "terminalPath").unwrap_or(d.terminal_path),
            terminal_args: field(value, "terminalArgs").unwrap_or(d.terminal_args),
            update_channel: field(value, "updateChannel").unwrap_or(d.update_channel),
        }
    }

    /// The same settings with `encoding` chosen for the repository at `path`.
    #[cfg(test)]
    pub fn with_encoding(mut self, path: &str, encoding: &str) -> Self {
        self.repo_prefs.insert(path.to_owned(), RepoPrefs { encoding: Some(encoding.to_owned()) });
        self
    }

    /// Repository path -> encoding label.
    pub fn encodings(&self) -> impl Iterator<Item = (&str, &str)> {
        self.repo_prefs.iter().filter_map(|(path, prefs)| Some((path.as_str(), prefs.encoding.as_deref().filter(|e| !e.trim().is_empty())?)))
    }
}

static CURRENT: RwLock<Option<Arc<Settings>>> = RwLock::new(None);

/// The settings in effect. Cheap to call; returns a shared snapshot.
pub fn current() -> Arc<Settings> {
    CURRENT.read().ok().and_then(|c| c.clone()).unwrap_or_default()
}

/// Replace the settings in effect.
pub fn set(settings: Settings) {
    if let Ok(mut c) = CURRENT.write() {
        *c = Some(Arc::new(settings));
    }
}

/// GitCraft's settings file.
pub fn file() -> PathBuf {
    crate::sample::data_dir().join("settings.json")
}

/// The saved file as JSON (`Null` when missing or unreadable).
pub fn read_saved() -> serde_json::Value {
    std::fs::read_to_string(file()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(serde_json::Value::Null)
}

/// Load the saved settings into the current state (at start-up and in the credential helper).
pub fn load() {
    set(Settings::from_saved(&read_saved()));
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use serde_json::json;

    /// Run `body` with `settings` in effect, restoring the previous state afterwards.
    pub(crate) fn with<T>(settings: Settings, body: impl FnOnce() -> T) -> T {
        struct Restore(Option<Arc<Settings>>);
        impl Drop for Restore {
            fn drop(&mut self) { if let Ok(mut c) = CURRENT.write() { *c = self.0.take(); } }
        }
        let _restore = Restore(CURRENT.read().ok().and_then(|c| c.clone()));
        set(settings);
        body()
    }

    #[test]
    fn missing_keys_take_front_end_defaults_and_unknown_keys_are_ignored() {
        let s = Settings::from_saved(&json!({ "settings": { "theme": "dark", "gitPath": "/usr/bin/git" }, "tabs": [] }));
        assert_eq!(s.git_path, "/usr/bin/git");
        assert!(s.ssh_use_agent);
        assert!(s.ai.enabled);
        assert_eq!(s.ai.provider, "anthropic");
        let empty = Settings::from_saved(&serde_json::Value::Null);
        assert!(empty.ssh_use_agent && empty.git_path.is_empty());
    }

    #[test]
    fn a_malformed_section_keeps_the_others() {
        let s = Settings::from_saved(&json!({ "settings": { "gitPath": 3, "sshUseAgent": false, "repoPrefs": { "/r": { "encoding": "windows-1252", "tracker": { "kind": "jira" } } } } }));
        assert!(s.git_path.is_empty());
        assert!(!s.ssh_use_agent);
        assert_eq!(s.encodings().collect::<Vec<_>>(), [("/r", "windows-1252")]);
    }

    #[test]
    fn the_older_gitlab_host_fills_an_empty_integration_host() {
        let s = Settings::from_saved(&json!({ "settings": { "gitlabHost": "git.example.test", "integrations": { "gitlabHost": "" } } }));
        assert_eq!(s.integrations.gitlab_host, "git.example.test");
        let s = Settings::from_saved(&json!({ "settings": { "gitlabHost": "old.test", "integrations": { "gitlabHost": "new.test" } } }));
        assert_eq!(s.integrations.gitlab_host, "new.test");
    }
}

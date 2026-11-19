//! Commands GitCraft may start on the person's behalf from values in its settings file: a custom
//! editor or terminal, a Git executable and an AI command-line program.
//!
//! A new value is shown in a native confirmation dialog, with the exact command, when the settings
//! are saved. Approved commands are remembered by fingerprint in a file the window cannot write,
//! so a value approved once is not asked about again, and a value that was never approved (for
//! example, written to the settings file by another program) is refused when it would run.

use crate::settings::Settings;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Mutex;

/// One command a setting can start.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Command {
    pub kind: Kind,
    /// The values that make up the command, in a fixed order; the fingerprint covers all of them.
    parts: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Editor,
    Terminal,
    Git,
    AiProgram,
}

impl Kind {
    fn tag(self) -> &'static str {
        match self {
            Kind::Editor => "editor",
            Kind::Terminal => "terminal",
            Kind::Git => "git",
            Kind::AiProgram => "ai-program",
        }
    }
    /// The top-level settings keys this command comes from (restored together when refused).
    fn fields(self) -> &'static [&'static str] {
        match self {
            Kind::Editor => &["externalEditor", "editorPath", "editorFileArgs", "editorRepoArgs"],
            Kind::Terminal => &["externalTerminal", "terminalPath", "terminalArgs"],
            Kind::Git => &["gitPath"],
            Kind::AiProgram => &["ai"],
        }
    }
}

impl Command {
    pub fn editor(program: &str, file_args: &str, repo_args: &str) -> Self {
        Command { kind: Kind::Editor, parts: vec![program.trim().into(), file_args.into(), repo_args.into()] }
    }
    pub fn terminal(program: &str, args: &str) -> Self {
        Command { kind: Kind::Terminal, parts: vec![program.trim().into(), args.into()] }
    }
    pub fn git(program: &str) -> Self {
        Command { kind: Kind::Git, parts: vec![program.trim().into()] }
    }
    pub fn ai_program(program: &str) -> Self {
        Command { kind: Kind::AiProgram, parts: vec![program.trim().into()] }
    }

    pub fn fingerprint(&self) -> String {
        let mut hash = Sha256::new();
        hash.update(self.kind.tag());
        for part in &self.parts {
            hash.update([0]);
            hash.update(part.as_bytes());
        }
        hash.finalize().iter().map(|b| format!("{b:02x}")).collect()
    }

    /// What the confirmation shows: the program and its arguments exactly as they will be used.
    pub fn describe(&self) -> String {
        let p = &self.parts;
        let or = |value: &str, default: &str| if value.trim().is_empty() { default.to_string() } else { value.to_string() };
        match self.kind {
            Kind::Editor => format!(
                "GitCraft will open files and repositories with this editor command:\n\n\"{}\" {}\n\"{}\" {}\n\n$FILE and $REPO_PATH are replaced by the file and repository paths.",
                p[0], or(&p[1], "\"$FILE\""), p[0], or(&p[2], "\"$REPO_PATH\"")
            ),
            Kind::Terminal => format!("GitCraft will open terminals with this command:\n\n\"{}\" {}\n\n$REPO_PATH is replaced by the repository path.", p[0], p[1]),
            Kind::Git => format!("GitCraft will run every Git command with this program:\n\n\"{}\"", p[0]),
            Kind::AiProgram => format!("GitCraft will run this program for AI features:\n\n\"{}\"", p[0]),
        }
    }

    pub fn title(&self) -> &'static str {
        match self.kind {
            Kind::Editor => "Allow this editor command?",
            Kind::Terminal => "Allow this terminal command?",
            Kind::Git => "Allow this Git program?",
            Kind::AiProgram => "Allow this AI program?",
        }
    }
}

/// The commands `settings` can start. Built-in tools and empty paths start nothing configurable.
pub fn commands(settings: &Settings) -> Vec<Command> {
    let mut out = Vec::new();
    if settings.external_editor == "custom" && !settings.editor_path.trim().is_empty() {
        out.push(Command::editor(&settings.editor_path, &settings.editor_file_args, &settings.editor_repo_args));
    }
    if settings.external_terminal == "custom" && !settings.terminal_path.trim().is_empty() {
        out.push(Command::terminal(&settings.terminal_path, &settings.terminal_args));
    }
    if !settings.git_path.trim().is_empty() {
        out.push(Command::git(&settings.git_path));
    }
    if !settings.ai.cli_path.trim().is_empty() {
        out.push(Command::ai_program(&settings.ai.cli_path));
    }
    out
}

static APPROVED: Mutex<Option<HashSet<String>>> = Mutex::new(None);

fn file() -> PathBuf {
    crate::sample::data_dir().join("approved-commands.json")
}

fn read(path: &std::path::Path) -> Option<HashSet<String>> {
    let saved: Value = serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
    Some(saved["approved"].as_array()?.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect())
}

fn write(path: &std::path::Path, approved: &HashSet<String>) -> Result<(), String> {
    let mut list: Vec<_> = approved.iter().collect();
    list.sort();
    if let Some(dir) = path.parent() { std::fs::create_dir_all(dir).map_err(|e| e.to_string())?; }
    let pending = path.with_extension("json.tmp");
    std::fs::write(&pending, serde_json::to_string_pretty(&json!({ "version": 1, "approved": list })).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    std::fs::rename(pending, path).map_err(|e| e.to_string())
}

/// Load the approved commands. The first time (no approvals file yet), the commands already in
/// `settings` are approved, so existing setups keep working.
pub fn load(settings: &Settings) {
    let path = file();
    let approved = read(&path).unwrap_or_else(|| {
        let migrated: HashSet<String> = commands(settings).iter().map(Command::fingerprint).collect();
        let _ = write(&path, &migrated);
        migrated
    });
    *APPROVED.lock().unwrap() = Some(approved);
}

pub fn is_approved(command: &Command) -> bool {
    APPROVED.lock().unwrap().as_ref().is_some_and(|set| set.contains(&command.fingerprint()))
}

/// Refuse a command that was never approved.
pub fn require(command: &Command) -> Result<(), String> {
    if is_approved(command) { return Ok(()); }
    Err(match command.kind {
        Kind::Editor => "This editor command was not approved. Choose it again in Preferences > External Tools.",
        Kind::Terminal => "This terminal command was not approved. Choose it again in Preferences > External Tools.",
        Kind::Git => "This Git program was not approved. Choose it again in Preferences > Experimental.",
        Kind::AiProgram => "This AI program was not approved. Choose it again in Preferences > AI.",
    }.into())
}

fn approve(command: &Command) -> Result<(), String> {
    let mut guard = APPROVED.lock().unwrap();
    let set = guard.get_or_insert_with(HashSet::new);
    if set.insert(command.fingerprint()) { write(&file(), set)?; }
    Ok(())
}

/// Ask the person, in a native dialog over `window`, whether `title`/`text` may go ahead.
pub async fn confirm(window: &tauri::Window, title: &str, text: &str) -> bool {
    let answer = rfd::AsyncMessageDialog::new()
        .set_level(rfd::MessageLevel::Warning)
        .set_title(title)
        .set_description(format!("{text}\n\nOnly allow commands you recognise."))
        .set_buttons(rfd::MessageButtons::YesNo)
        .set_parent(window)
        .show()
        .await;
    matches!(answer, rfd::MessageDialogResult::Yes)
}

/// The commands in `next` that still need a decision.
pub fn pending(next: &Settings) -> Vec<Command> {
    commands(next).into_iter().filter(|c| !is_approved(c)).collect()
}

/// Put back, in `saved` (the settings file being written), the previous values of each refused
/// command. Returns the restored top-level settings, for the window to apply.
pub fn restore(saved: &mut Value, previous: &Value, refused: &[Command]) -> Map<String, Value> {
    let mut corrections = Map::new();
    for command in refused {
        for field in command.kind.fields() {
            let old = if command.kind == Kind::AiProgram {
                // Only the program path is restored; the rest of the AI settings stay as edited.
                let mut ai = saved["settings"]["ai"].clone();
                if let Some(ai) = ai.as_object_mut() { ai.insert("cliPath".into(), previous["ai"]["cliPath"].clone()); }
                ai
            } else {
                // A key missing from the previous file had its default value.
                previous.get(*field).cloned().unwrap_or_else(|| json!(match *field { "externalEditor" => "vscode", "externalTerminal" => "default", _ => "" }))
            };
            if let Some(object) = saved["settings"].as_object_mut() {
                object.insert((*field).into(), old.clone());
            }
            corrections.insert((*field).into(), old);
        }
    }
    corrections
}

/// Review the settings about to be saved: ask about each new command, remember approvals, and
/// restore refused values. `previous` is the `settings` object currently on disk.
pub async fn review(window: &tauri::Window, saved: &mut Value, previous: &Value) -> Result<Map<String, Value>, String> {
    let mut refused = Vec::new();
    for command in pending(&Settings::from_saved(saved)) {
        if confirm(window, command.title(), &command.describe()).await {
            approve(&command)?;
        } else {
            refused.push(command);
        }
    }
    Ok(restore(saved, previous, &refused))
}

/// Git config keys the window may change without asking: identity, signing choices and line
/// endings. They cannot name a program for Git to run.
pub const PLAIN_GIT_CONFIG: &[&str] = &[
    "user.name", "user.email", "user.signingkey", "commit.gpgsign", "tag.gpgsign", "gpg.format",
    "core.autocrlf", "core.longpaths", "gpg.ssh.allowedsignersfile",
];

pub fn plain_git_config(key: &str) -> bool {
    PLAIN_GIT_CONFIG.contains(&key.to_ascii_lowercase().as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(value: Value) -> Settings {
        Settings::from_saved(&json!({ "settings": value }))
    }

    #[test]
    fn only_custom_tools_and_explicit_programs_are_commands() {
        assert!(commands(&Settings::default()).is_empty());
        assert!(commands(&settings(json!({ "externalEditor": "vscode", "editorPath": "/bin/sh" }))).is_empty());
        let all = commands(&settings(json!({
            "externalEditor": "custom", "editorPath": "/opt/ed", "editorFileArgs": "-f $FILE",
            "externalTerminal": "custom", "terminalPath": "/opt/term", "terminalArgs": "--cwd $REPO_PATH",
            "gitPath": "/opt/git", "ai": { "cliPath": "/opt/agent" },
        })));
        assert_eq!(all.iter().map(|c| c.kind).collect::<Vec<_>>(), [Kind::Editor, Kind::Terminal, Kind::Git, Kind::AiProgram]);
        assert!(all[0].describe().contains("\"/opt/ed\" -f $FILE") && all[0].describe().contains("\"/opt/ed\" \"$REPO_PATH\""));
    }

    #[test]
    fn fingerprints_cover_every_part_and_the_kind() {
        let base = Command::editor("/opt/ed", "$FILE", "");
        assert_eq!(base.fingerprint(), Command::editor(" /opt/ed ", "$FILE", "").fingerprint());
        assert_ne!(base.fingerprint(), Command::editor("/opt/ed", "$FILE --exec x", "").fingerprint());
        assert_ne!(base.fingerprint(), Command::editor("/opt/ed", "", "$FILE").fingerprint());
        assert_ne!(Command::git("/opt/x").fingerprint(), Command::ai_program("/opt/x").fingerprint());
        assert_eq!(base.fingerprint().len(), 64);
    }

    #[test]
    fn refused_commands_restore_their_previous_fields() {
        let previous = json!({ "externalEditor": "vscode", "gitPath": "", "ai": { "provider": "codex", "cliPath": "/old/codex" } });
        let mut saved = json!({ "settings": {
            "externalEditor": "custom", "editorPath": "/tmp/evil", "gitPath": "/tmp/git", "theme": "dark",
            "ai": { "provider": "claude_code", "cliPath": "/tmp/agent" },
        } });
        let refused = [Command::editor("/tmp/evil", "", ""), Command::git("/tmp/git"), Command::ai_program("/tmp/agent")];
        let corrections = restore(&mut saved, &previous, &refused);
        assert_eq!(saved["settings"]["externalEditor"], "vscode");
        assert_eq!(saved["settings"]["editorPath"], "");
        assert_eq!(saved["settings"]["gitPath"], "");
        assert_eq!(saved["settings"]["ai"], json!({ "provider": "claude_code", "cliPath": "/old/codex" }));
        assert_eq!(saved["settings"]["theme"], "dark");
        assert_eq!(corrections["externalEditor"], "vscode");
        assert_eq!(corrections["editorPath"], "");
        assert_eq!(corrections["ai"]["cliPath"], "/old/codex");
    }

    #[test]
    fn approvals_migrate_once_and_unapproved_values_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("approved.json");
        let existing = settings(json!({ "gitPath": "/usr/local/bin/git" }));
        let migrated: HashSet<String> = commands(&existing).iter().map(Command::fingerprint).collect();
        write(&path, &migrated).unwrap();
        let loaded = read(&path).unwrap();
        assert!(loaded.contains(&Command::git("/usr/local/bin/git").fingerprint()));
        assert!(!loaded.contains(&Command::git("/tmp/other-git").fingerprint()));
        let previous = APPROVED.lock().unwrap().replace(loaded);
        assert!(require(&Command::git("/usr/local/bin/git")).is_ok());
        assert!(require(&Command::git("/tmp/other-git")).is_err());
        assert_eq!(pending(&settings(json!({ "gitPath": "/tmp/other-git" }))), [Command::git("/tmp/other-git")]);
        *APPROVED.lock().unwrap() = previous;
    }

    #[test]
    fn only_plain_git_config_keys_skip_confirmation() {
        for key in ["user.name", "tag.gpgSign", "gpg.ssh.allowedSignersFile", "core.autocrlf"] { assert!(plain_git_config(key), "{key}"); }
        for key in ["core.hooksPath", "gpg.program", "credential.helper", "core.fsmonitor", "core.sshCommand", "alias.x"] { assert!(!plain_git_config(key), "{key}"); }
    }
}

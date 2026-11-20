//! Local, read-only session discovery for Claude Code and Codex.
//! Codex JSONL: session_meta + response_item messages + task_started/task_complete events.
use serde::Serialize;
use serde_json::Value;
use std::{
    fs::{self, File},
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Serialize, Clone, Debug)]
pub struct AgentSession {
    pub id: String,
    pub agent: String,
    pub path: String,
    pub title: String,
    pub modified: i64,
    pub messages: usize,
    /// Log evidence only: never claim a live process based on file modification time.
    pub status: String,
}

fn home() -> PathBuf {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .map(PathBuf::from).unwrap_or_default()
}

fn config_dir(explicit: Option<&str>, variable: &str, fallback: &str) -> PathBuf {
    let value = explicit.filter(|p| !p.trim().is_empty()).map(PathBuf::from)
        .or_else(|| std::env::var_os(variable).map(PathBuf::from))
        .unwrap_or_else(|| home().join(fallback));
    let s = value.to_string_lossy();
    if let Some(rest) = s.strip_prefix("~/").or_else(|| s.strip_prefix("~\\")) {
        home().join(rest)
    } else { value }
}

fn normalized(path: &str) -> String {
    let s = path.replace('\\', "/");
    let s = if let Some(unc) = s.strip_prefix("//?/UNC/") { format!("//{unc}") } else { s.strip_prefix("//?/").unwrap_or(&s).to_string() };
    let s = s.trim_end_matches('/');
    if cfg!(windows) { s.to_lowercase() } else { s.to_string() }
}

fn belongs(cwd: &str, root: &str) -> bool {
    let cwd = normalized(cwd);
    let root = normalized(root);
    cwd == root || cwd.strip_prefix(&root).is_some_and(|tail| tail.starts_with('/'))
}

fn project_name(path: &str) -> String {
    path.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect()
}

fn jsonl_files(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else { continue };
        if kind.is_dir() && depth > 0 { jsonl_files(&entry.path(), depth - 1, out); }
        else if kind.is_file() && entry.path().extension().is_some_and(|x| x == "jsonl") { out.push(entry.path()); }
    }
}

fn message_text(content: &Value) -> String {
    if let Some(s) = content.as_str() { return s.to_string() }
    content.as_array().map(|items| items.iter().filter_map(|v| v["text"].as_str()).collect::<Vec<_>>().join("\n")).unwrap_or_default()
}

fn title_from(text: &str) -> Option<String> {
    let text = text.trim();
    if text.is_empty() || text.starts_with('<') || text.starts_with("# AGENTS.md") { return None }
    Some(text.lines().next().unwrap_or_default().chars().take(120).collect())
}

#[cfg(test)]
static PARSES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
fn parse_session(file: &Path, agent: &str, fallback: Option<&str>) -> Option<AgentSession> {
    #[cfg(test)]
    PARSES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let meta = fs::metadata(file).ok()?;
    let modified = meta.modified().ok()?.duration_since(UNIX_EPOCH).ok()?.as_secs() as i64;
    // A corrupt or enormous log cannot allocate unbounded memory. Partial JSON lines are ignored.
    let mut reader = BufReader::new(File::open(file).ok()?.take(32 * 1024 * 1024));
    let mut line = String::new();
    let mut cwd = fallback.unwrap_or_default().to_string();
    let mut id = if agent == "claude" { file.file_stem()?.to_string_lossy().into_owned() } else { String::new() };
    let mut title = String::new();
    let mut messages = 0;
    let mut finished = false;
    let mut has_meta = agent == "claude";
    while reader.read_line(&mut line).ok()? > 0 {
        if let Ok(v) = serde_json::from_str::<Value>(&line) {
            if agent == "codex" {
                let p = &v["payload"];
                if v["type"] == "session_meta" && !has_meta {
                    cwd = p["cwd"].as_str()?.to_string();
                    id = p["id"].as_str()?.to_string();
                    has_meta = true;
                }
                if v["type"] == "response_item" && p["type"] == "message" {
                    if p["role"] == "user" || p["role"] == "assistant" { messages += 1; }
                    if title.is_empty() && p["role"] == "user" {
                        if let Some(t) = title_from(&message_text(&p["content"])) { title = t; }
                    }
                }
                if v["type"] == "event_msg" {
                    match p["type"].as_str().unwrap_or_default() {
                        "task_started" | "turn_started" => finished = false,
                        "task_complete" | "turn_complete" | "turn_aborted" => finished = true,
                        "user_message" if title.is_empty() => {
                            if let Some(t) = p["message"].as_str().and_then(title_from) { title = t; }
                        }
                        _ => {}
                    }
                }
            } else {
                if let Some(path) = v["cwd"].as_str() { cwd = path.to_string(); }
                if v["type"] == "user" || v["type"] == "assistant" { messages += 1; }
                if v["type"] == "user" {
                    finished = false;
                    if title.is_empty() {
                        if let Some(t) = title_from(&message_text(&v["message"]["content"])) { title = t; }
                    }
                }
                if v["type"] == "assistant" {
                    finished = matches!(v["message"]["stop_reason"].as_str(), Some("end_turn" | "stop_sequence"));
                }
            }
        }
        line.clear();
    }
    if !has_meta || id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') { return None }
    let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs() as i64;
    Some(AgentSession {
        id, agent: agent.into(), path: cwd,
        title: if title.is_empty() { "(untitled session)".into() } else { title },
        modified, messages,
        status: if finished && meta.len() <= 32 * 1024 * 1024 { "finished" } else if now.saturating_sub(modified) < 120 { "recent" } else { "idle" }.into(),
    })
}

// Admission stops at the byte budget instead of evicting every entry during a scan.
// This retains a stable hot subset even when a catalog is larger than the cache.
const SESSION_CACHE_BYTES: usize = 16 * 1024 * 1024;
#[derive(Default)]
struct SessionCache {
    entries: std::collections::HashMap<String, (u64, SystemTime, Option<AgentSession>, usize)>,
    bytes: usize,
}
impl SessionCache {
    fn insert(&mut self, key: String, size: u64, modified: SystemTime, result: Option<AgentSession>) {
        if let Some(old) = self.entries.remove(&key) { self.bytes -= old.3; }
        let cost = 256 + key.len() + result.as_ref().map(|s| s.id.len() + s.agent.len() + s.path.len() + s.title.len() + s.status.len()).unwrap_or(0);
        if self.bytes + cost <= SESSION_CACHE_BYTES {
            self.bytes += cost;
            self.entries.insert(key, (size, modified, result, cost));
        }
    }
}
fn cached_session(file: &Path, agent: &str, fallback: Option<&str>) -> Option<AgentSession> {
    use std::sync::{Mutex, OnceLock};
    static CACHE: OnceLock<Mutex<SessionCache>> = OnceLock::new();
    let metadata = fs::metadata(file).ok()?;
    let modified = metadata.modified().ok()?;
    let key = serde_json::to_string(&(file, agent, fallback)).ok()?;
    let cache = CACHE.get_or_init(Default::default);
    let cached = cache.lock().unwrap().entries.get(&key)
        .filter(|(size, time, _, _)| *size == metadata.len() && *time == modified).map(|(_, _, result, _)| result.clone());
    let result = match cached {
        Some(result) => result,
        None => {
            let result = parse_session(file, agent, fallback);
            cache.lock().unwrap().insert(key, metadata.len(), modified, result.clone());
            result
        }
    };
    result
}
fn scoped_session(mut session: AgentSession, roots: &[String]) -> Option<AgentSession> {
    session.path = roots.iter().filter(|r| belongs(&session.path, r)).max_by_key(|r| r.len())?.clone();
    let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs() as i64;
    if session.status == "recent" && now.saturating_sub(session.modified) >= 120 { session.status = "idle".into(); }
    Some(session)
}

fn read_session(file: &Path, agent: &str, roots: &[String], fallback: Option<&str>) -> Option<AgentSession> {
    scoped_session(cached_session(file, agent, fallback)?, roots)
}

// Each configuration keeps an event-driven index. A missed-event recovery scan
// runs once a minute, rather than once per repository/tab refresh.
const INDEX_BYTES: usize = 8 * 1024 * 1024;
struct CatalogIndex {
    root: PathBuf,
    entries: std::collections::HashMap<PathBuf, (Option<AgentSession>, usize)>,
    bytes: usize,
    complete: bool,
    events: std::sync::mpsc::Receiver<notify_debouncer_mini::notify::Event>,
    dirty: std::sync::Arc<std::sync::atomic::AtomicBool>,
    watcher: Option<notify_debouncer_mini::notify::RecommendedWatcher>,
    scanned: std::time::Instant,
    scans: usize,
    probes: usize,
}
impl CatalogIndex {
    fn new(root: PathBuf) -> Self {
        use notify_debouncer_mini::notify::{self, Watcher};
        // Native events use the resolved path (notably /private/var on macOS).
        // Discovery and event keys must share that identity, including removals
        // whose paths can no longer be canonicalized after the file disappears.
        let root = fs::canonicalize(&root).unwrap_or(root);
        let (send, events) = std::sync::mpsc::sync_channel(1024);
        let dirty = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let overflow = dirty.clone();
        let watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| match event {
            Ok(event) if !matches!(event.kind, notify::EventKind::Access(_)) => {
                if send.try_send(event).is_err() { overflow.store(true, std::sync::atomic::Ordering::Release); }
            }
            Err(_) => { overflow.store(true, std::sync::atomic::Ordering::Release); }
            _ => {}
        }).ok().and_then(|mut watcher| watcher.watch(&root, notify::RecursiveMode::Recursive).ok().map(|_| watcher));
        let mut index = Self { root, entries: Default::default(), bytes: 0, complete: true, events, dirty, watcher, scanned: std::time::Instant::now(), scans: 0, probes: 0 };
        index.scan(); index
    }
    fn update_file(&mut self, path: PathBuf) {
        if path.extension().is_none_or(|ext| ext != "jsonl") { return; }
        // Keep discovery's original three-level directory budget; refuse escapes.
        if !path.strip_prefix(&self.root).is_ok_and(|rel| rel.components().count() <= 4 && rel.components().all(|c| matches!(c, std::path::Component::Normal(_)))) { return; }
        self.probes += 1;
        let session = cached_session(&path, "codex", None);
        if let Some((_, cost)) = self.entries.remove(&path) { self.bytes -= cost; }
        if !path.is_file() { return; }
        let cost = 256 + path.as_os_str().len() * 2 + session.as_ref().map(|s| s.id.len()+s.path.len()+s.title.len()+s.agent.len()+s.status.len()).unwrap_or(0);
        if self.bytes + cost > INDEX_BYTES { self.complete = false; return; }
        self.bytes += cost; self.entries.insert(path, (session, cost));
    }
    fn scan(&mut self) {
        let mut files = Vec::new(); jsonl_files(&self.root, 3, &mut files);
        let existing: std::collections::HashSet<_> = files.iter().cloned().collect();
        self.entries.retain(|path, _| existing.contains(path));
        self.bytes = self.entries.values().map(|(_, cost)| cost).sum(); self.complete = true;
        for path in files { self.update_file(path); }
        self.scanned = std::time::Instant::now(); self.scans += 1;
    }
    fn refresh(&mut self) {
        use std::sync::atomic::Ordering;
        let events: Vec<_> = self.events.try_iter().collect();
        if self.watcher.is_none() || self.dirty.swap(false, Ordering::AcqRel) || events.iter().any(|e| e.need_rescan()) || self.scanned.elapsed() >= std::time::Duration::from_secs(60) {
            // Retry registration when a previously absent sessions folder appears.
            if self.watcher.is_none() && self.root.is_dir() { *self = Self::new(self.root.clone()); }
            else { self.scan(); }
            return;
        }
        let paths: std::collections::HashSet<_> = events.into_iter().flat_map(|e| e.paths).collect();
        self.apply_paths(paths);
    }
    fn apply_paths(&mut self, paths: std::collections::HashSet<PathBuf>) {
        for path in paths {
            if path.is_dir() && !path.is_symlink() {
                let mut files = Vec::new(); jsonl_files(&path, 3, &mut files);
                for file in files { self.update_file(file); }
            } else if path.exists() { self.update_file(path); }
            else {
                self.entries.retain(|file, _| !file.starts_with(&path));
                self.bytes = self.entries.values().map(|(_, cost)| cost).sum();
            }
        }
    }
    fn sessions(&mut self, roots: &[String]) -> Vec<AgentSession> {
        self.refresh();
        if !self.complete {
            // Oversized indexes never silently omit results. Parsed-file admission
            // stays bounded even when the index falls back to complete discovery.
            let mut files = Vec::new(); jsonl_files(&self.root, 3, &mut files);
            return files.into_iter().filter_map(|p| read_session(&p, "codex", roots, None)).collect();
        }
        self.entries.values().filter_map(|(entry, _)| scoped_session(entry.clone()?, roots)).collect()
    }
}
fn codex_sessions(root: PathBuf, roots: &[String]) -> Vec<AgentSession> {
    use std::sync::{Arc, Mutex, OnceLock};
    type Catalogs = std::collections::HashMap<PathBuf, Arc<Mutex<CatalogIndex>>>;
    static CATALOGS: OnceLock<Mutex<Catalogs>> = OnceLock::new();
    let cache = CATALOGS.get_or_init(Default::default);
    let existing = cache.lock().unwrap().get(&root).cloned();
    let index = existing.unwrap_or_else(|| {
        let index = Arc::new(Mutex::new(CatalogIndex::new(root.clone())));
        let mut catalogs = cache.lock().unwrap();
        let removed = if catalogs.len() >= 2 { let key = catalogs.keys().next().cloned().unwrap(); catalogs.remove(&key) } else { None };
        let result = catalogs.entry(root).or_insert(index).clone();
        drop(catalogs); drop(removed); result
    });
    let sessions = index.lock().unwrap().sessions(roots); sessions
}

/// Only sessions whose recorded working directory is in one of this repository's worktrees.
pub fn sessions(roots: &[String], claude_dir: Option<&str>, codex_dir: Option<&str>) -> Vec<AgentSession> {
    let claude = config_dir(claude_dir, "CLAUDE_CONFIG_DIR", ".claude");
    let codex = config_dir(codex_dir, "CODEX_HOME", ".codex");
    let mut out = Vec::new();
    for root in roots {
        let mut names = vec![project_name(root)];
        if let Ok(canonical) = fs::canonicalize(root) {
            let plain = canonical.to_string_lossy().trim_start_matches(r"\\?\").to_string();
            names.push(project_name(&plain));
        }
        names.sort();
        names.dedup();
        for name in names {
            let mut files = Vec::new();
            jsonl_files(&claude.join("projects").join(name), 0, &mut files);
            for file in files {
                if let Some(session) = read_session(&file, "claude", roots, Some(root)) { out.push(session); }
            }
        }
    }
    out.extend(codex_sessions(codex.join("sessions"), roots));
    out.sort_by_key(|s| std::cmp::Reverse(s.modified));
    let mut seen = std::collections::HashSet::new();
    out.retain(|s| seen.insert((s.agent.clone(), s.id.clone())));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn wait_for_catalog(index: &mut CatalogIndex, roots: &[String], stage: &str, matches: impl Fn(&[AgentSession]) -> bool) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            if matches(&index.sessions(roots)) { return; }
            assert!(std::time::Instant::now() < deadline, "catalog event timed out: {stage}");
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }

    #[test]
    fn catalog_events_use_the_same_canonical_identity_as_discovery() {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir(temp.path().join("alias")).unwrap();
        let directory = temp.path().join("sessions"); fs::create_dir(&directory).unwrap();
        let file = directory.join("session.jsonl");
        fs::write(&file, r#"{"type":"session_meta","payload":{"id":"original","cwd":"/index"}}"#).unwrap();
        // An unresolved alias differs lexically, just as /var and /private/var
        // do in macOS temp directories. Inject native-style events deterministically.
        let alias = temp.path().join("alias/../sessions");
        let mut index = CatalogIndex::new(alias);
        let canonical = directory.canonicalize().unwrap();
        assert_eq!(index.root, canonical);
        let canonical_file = canonical.join("session.jsonl");
        let scans = index.scans;
        fs::write(&file, r#"{"type":"session_meta","payload":{"id":"changed","cwd":"/index"}}"#).unwrap();
        index.apply_paths([canonical_file.clone()].into_iter().collect());
        assert_eq!(index.entries[&canonical_file].0.as_ref().unwrap().id, "changed");
        fs::remove_file(file).unwrap();
        index.apply_paths([canonical_file].into_iter().collect());
        assert!(index.entries.is_empty());
        assert_eq!(index.scans, scans, "canonical events must not need a recovery scan");
    }

    #[test]
    fn catalog_index_avoids_idle_probes_and_tracks_append_create_remove_and_overflow() {
        let temp = tempfile::tempdir().unwrap();
        for i in 0..4200 { fs::write(temp.path().join(format!("{i}.jsonl")), format!(r#"{{"type":"session_meta","payload":{{"id":"id-{i}","cwd":"/index/topic"}}}}"#)).unwrap(); }
        let mut index = CatalogIndex::new(temp.path().to_owned());
        assert!(index.watcher.is_some());
        let roots = ["/index".into()]; assert_eq!(index.sessions(&roots).len(), 4200);
        let probes = index.probes; let scans = index.scans;
        for _ in 0..3 { assert_eq!(index.sessions(&["/index/topic".into()]).len(), 4200); }
        assert_eq!(index.probes, probes); assert_eq!(index.scans, scans);
        let changed = temp.path().join("0.jsonl");
        fs::write(&changed, r#"{"type":"session_meta","payload":{"id":"changed","cwd":"/index"}}"#).unwrap();
        wait_for_catalog(&mut index, &roots, "modified file", |items| items.iter().any(|s| s.id == "changed"));
        assert!(index.probes - probes < 10, "changed file caused a full catalog scan");
        {
            use std::io::Write;
            let mut file = fs::OpenOptions::new().append(true).open(&changed).unwrap();
            writeln!(file, "\n{}", json!({"type":"event_msg","payload":{"type":"user_message","message":"Appended session title"}})).unwrap();
        }
        wait_for_catalog(&mut index, &roots, "appended message", |items| items.iter().any(|s| s.id == "changed" && s.title == "Appended session title"));
        fs::remove_file(changed).unwrap();
        wait_for_catalog(&mut index, &roots, "removed file", |items| items.len() == 4199);
        fs::create_dir(temp.path().join("new")).unwrap();
        fs::write(temp.path().join("new/created.jsonl"), r#"{"type":"session_meta","payload":{"id":"created","cwd":"/index"}}"#).unwrap();
        wait_for_catalog(&mut index, &roots, "created nested file", |items| items.iter().any(|s| s.id == "created"));
        index.dirty.store(true, std::sync::atomic::Ordering::Release); assert_eq!(index.sessions(&roots).len(), 4200);
        assert!(index.scans > scans); assert!(index.bytes <= INDEX_BYTES);
        eprintln!("catalog: {probes} initial probes, zero idle probes, {} retained bytes", index.bytes);
    }

    #[test]
    fn repeated_4200_file_scans_and_new_repository_scopes_do_not_reparse() {
        use std::sync::atomic::Ordering;
        let temp = tempfile::tempdir().unwrap(); let roots = vec!["/catalog".into()];
        for i in 0..4200 {
            let file = temp.path().join(format!("session-{i}.jsonl"));
            fs::write(&file, format!(r#"{{"type":"session_meta","payload":{{"id":"id-{i}","cwd":"/catalog/topic"}}}}"#)).unwrap();
            assert!(read_session(&file, "codex", &roots, None).is_some());
        }
        let before = PARSES.load(Ordering::Relaxed);
        for _ in 0..2 { for i in 0..4200 {
            let file = temp.path().join(format!("session-{i}.jsonl"));
            assert_eq!(read_session(&file, "codex", &["/catalog/topic".into()], None).unwrap().path, "/catalog/topic");
        } }
        assert_eq!(PARSES.load(Ordering::Relaxed) - before, 0);
    }

    #[test]
    fn oversized_catalog_cache_keeps_stable_entries_and_scope_independent_metadata() {
        let mut cache = SessionCache::default();
        for i in 0..70_000 { cache.insert(format!("session-{i}"), 1, UNIX_EPOCH, None); }
        assert!(cache.entries.len() > 4096); assert!(cache.bytes <= SESSION_CACHE_BYTES);
        let retained = cache.entries.len();
        for i in 0..70_000 { cache.insert(format!("session-{i}"), 1, UNIX_EPOCH, None); }
        assert_eq!(retained, cache.entries.len()); assert!(cache.entries.contains_key("session-0"));
        let temp = tempfile::tempdir().unwrap(); let file = temp.path().join("session.jsonl");
        fs::write(&file, r#"{"type":"session_meta","payload":{"id":"scope-test","cwd":"/repo/sub"}}"#).unwrap();
        assert!(read_session(&file, "codex", &["/elsewhere".into()], None).is_none());
        assert_eq!(read_session(&file, "codex", &["/repo".into()], None).unwrap().path, "/repo");
        assert_eq!(read_session(&file, "codex", &["/repo".into(), "/repo/sub".into()], None).unwrap().path, "/repo/sub");
        fs::write(&file, r#"{"type":"session_meta","payload":{"id":"changed-session","cwd":"/repo"}}"#).unwrap();
        assert_eq!(read_session(&file, "codex", &["/repo".into()], None).unwrap().id, "changed-session");
    }

    #[test]
    fn discovers_both_agents_and_filters_worktrees_and_unrelated_folders() {
        let temp = std::env::temp_dir().join(format!("gitcraft-agent-discovery-{}", std::process::id()));
        fs::create_dir_all(&temp).unwrap();
        let repo = temp.join("repo").to_string_lossy().into_owned();
        let worktree = temp.join("repo-agent").to_string_lossy().into_owned();
        let roots = vec![repo.clone(), worktree.clone()];
        let claude = temp.join("claude");
        let codex = temp.join("codex");
        let cdir = claude.join("projects").join(project_name(&repo));
        let xdir = codex.join("sessions/2026/11/12");
        fs::create_dir_all(&cdir).unwrap();
        fs::create_dir_all(&xdir).unwrap();
        fs::write(cdir.join("claude-id.jsonl"), format!("{}\n{}\ninvalid tail",
            json!({"type":"user","cwd":repo,"message":{"content":[{"type":"text","text":"Fix the menu"}]}}),
            json!({"type":"assistant","message":{"content":"Done","stop_reason":"end_turn"}}))).unwrap();
        let rollout = |id: &str, cwd: &str| format!("{}\n{}\n{}\n",
            json!({"type":"session_meta","payload":{"id":id,"cwd":cwd}}),
            json!({"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"Add the banner"}]}}),
            json!({"type":"event_msg","payload":{"type":"task_complete"}}));
        fs::write(xdir.join("rollout-one.jsonl"), rollout("codex-id", &format!("{worktree}/src"))).unwrap();
        fs::write(xdir.join("rollout-other.jsonl"), rollout("other-id", &format!("{repo}-other"))).unwrap();
        let found = sessions(&roots, claude.to_str(), codex.to_str());
        assert_eq!(found.len(), 2, "{found:?}");
        let c = found.iter().find(|s| s.agent == "claude").unwrap();
        assert_eq!((&*c.title, &*c.status, c.messages), ("Fix the menu", "finished", 2));
        let x = found.iter().find(|s| s.agent == "codex").unwrap();
        assert_eq!((&*x.path, &*x.title, &*x.status), (&*worktree, "Add the banner", "finished"));
        assert!(!belongs(&format!("{repo}-other"), &repo));
        // Unchanged results can be reused, but appended messages must invalidate the cached parse.
        assert_eq!(sessions(&roots, claude.to_str(), codex.to_str()).len(), 2);
        let file = cdir.join("claude-id.jsonl");
        let mut text = fs::read_to_string(&file).unwrap();
        text.push_str(&format!("\n{}\n", json!({"type":"user", "cwd":repo, "message":{"content":"another request"}})));
        fs::write(file, text).unwrap();
        let updated = sessions(&roots, claude.to_str(), codex.to_str());
        assert_eq!(updated.iter().find(|s| s.agent == "claude").unwrap().messages, 3);
        fs::remove_dir_all(temp).unwrap();
    }
}

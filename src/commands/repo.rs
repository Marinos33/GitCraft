//! Opening and reading repositories, running actions, cloning, initialising and watching.

use super::*;

#[derive(Serialize)]
#[derive(ts_rs::TS)]
#[ts(export)]
pub(crate) struct RepoView {
    path: String,
    name: String,
    #[serde(flatten)]
    data: git::RepoData,
    graph: graph::GraphLayout,
    history_cursor: Option<HistoryCursor>,
    undo: UndoInfo,
}

#[derive(Clone, Deserialize, Serialize)]
#[derive(ts_rs::TS)]
#[ts(export)]
pub(crate) struct HistoryCursor {
    revs: Vec<String>,
    offset: usize,
    layout: graph::LayoutCursor,
}

#[derive(Serialize)]
#[derive(ts_rs::TS)]
#[ts(export)]
pub(crate) struct HistoryPage {
    commits: Vec<git::Commit>,
    graph: graph::GraphLayout,
    has_more: bool,
    history_cursor: Option<HistoryCursor>,
}

#[tauri::command]
pub(crate) async fn history_page(path: String, cursor: HistoryCursor, max: Option<usize>, read_id: Option<String>) -> Result<HistoryPage, String> {
    blocking(move || read_jobs::scope(read_id, || {
        let mut cursor = cursor;
        let (commits, has_more) = git::history_page(Path::new(&path), &cursor.revs, cursor.offset, max.unwrap_or(2000).clamp(100, 5000))?;
        let graph = cursor.layout.append(&commits);
        cursor.offset += commits.len();
        Ok(HistoryPage { commits, graph, has_more, history_cursor: has_more.then_some(cursor) })
    })).await?
}

#[tauri::command]
pub(crate) async fn open_repo(path: String, max: Option<usize>, view: Option<git::RefView>, state: State<'_, AppState>, read_id: Option<String>) -> Result<RepoView, String> {
    let engine = state.engine.clone();
    blocking(move || read_jobs::scope(read_id, || {
        // Tabs saved by versions that turned /Users/x into \Users\x on macOS/Linux: restore the slashes.
        let path = if cfg!(not(windows)) && path.starts_with('\\') { path.replace('\\', "/") } else { path };
        let p = PathBuf::from(&path);
        if !p.exists() {
            return Err(format!("The folder {path} doesn't exist"));
        }
        let root = git::repo_root(&p).ok_or_else(|| format!("{path} is not a Git repository"))?;
        let view = view.unwrap_or_default();
        let data = git::load_view_checked(&root, max.unwrap_or(INITIAL_COMMITS).clamp(prefs::MIN_INITIAL_COMMITS, 5000), &view)?;
        let pinned_sha = view.pinned.as_ref().and_then(|p| data.refs.iter().find(|r| &r.full == p)).map(|r| r.sha.clone());
        let mut layout = graph::LayoutCursor::new(pinned_sha.as_deref());
        let graph = layout.append(&data.commits);
        let history_cursor = data.has_more.then(|| HistoryCursor { revs: data.history_revs.clone(), offset: data.commits.len(), layout });
        // Windows paths use backslashes; on macOS/Linux the path must stay as-is (a blanket replace turns /Users/x into \Users\x).
        let root_s = if cfg!(windows) { root.to_string_lossy().replace('/', "\\") } else { root.to_string_lossy().into_owned() };
        Ok(RepoView {
            name: git::repo_name(&root),
            undo: engine.undo_info(&root_s),
            path: root_s,
            data,
            graph,
            history_cursor,
        })
    }))
    .await?
}

#[tauri::command]
pub(crate) async fn commit_details(path: String, sha: String, read_id: Option<String>) -> Result<git::Commit, String> {
    blocking(move || read_jobs::scope(read_id, || {
        let output = git::run(Path::new(&path), &["log", "--no-walk", "-1", "--format=%H%x00%P%x00%an%x00%ae%x00%at%x00%s%x00%b%x00", &sha, "--"]);
        if !output.ok { return Err(output.message()); }
        git::parse_commits(&output.stdout).into_iter().next().ok_or_else(|| "The selected commit is no longer available.".into())
    })).await?
}

#[tauri::command]
pub(crate) async fn commit_files(path: String, sha: String, read_id: Option<String>) -> Result<Vec<git::CommitFile>, String> {
    blocking(move || read_jobs::scope(read_id, || {
        let dir = PathBuf::from(&path);
        let o = git::run(&dir, &["rev-list", "--parents", "-n1", &sha]);
        let parents: Vec<String> = o.stdout.split_whitespace().skip(1).map(|s| s.to_string()).collect();
        let c = git::Commit {
            sha: sha.clone(),
            parents,
            author: String::new(),
            email: String::new(),
            time: 0,
            summary: String::new(),
            body: String::new(),
        };
        git::commit_file_entries(&dir, &c)
    }))
    .await
}

#[tauri::command]
pub(crate) async fn rebase_plan(path: String, base: String, read_id: Option<String>) -> Result<serde_json::Value, String> {
    read_blocking(read_id, move || {
        let dir = Path::new(&path);
        let lock = operation::lock_for(dir); let _guard = read_jobs::lock(&lock)?;
        let expected = rewrite::plan_state(dir, &base)?;
        let commits = rewrite::rebase_commits(dir, &base)?;
        Ok(serde_json::json!({ "expected": expected, "commits": commits }))
    }).await?
}

#[tauri::command]
pub(crate) async fn rebase_commits(path: String, base: String, read_id: Option<String>) -> Result<Vec<git::Commit>, String> {
    read_blocking(read_id, move || rewrite::rebase_commits(Path::new(&path), &base)).await?
}

#[tauri::command]
pub(crate) async fn conflict_info(path: String, file: String, read_id: Option<String>) -> Result<rewrite::ConflictInfo, String> {
    read_blocking(read_id, move || rewrite::conflict_info(Path::new(&path), &file)).await
}

#[tauri::command]
pub(crate) async fn gitflow_config(path: String, read_id: Option<String>) -> Result<manage::GitflowConfig, String> {
    read_blocking(read_id, move || manage::gitflow_config(Path::new(&path))).await
}

#[tauri::command]
pub(crate) async fn cleanup_list(path: String, read_id: Option<String>) -> Result<Vec<manage::CleanupBranch>, String> {
    read_blocking(read_id, move || manage::cleanup_list(Path::new(&path))).await
}

#[tauri::command]
pub(crate) async fn remotes_list(path: String, read_id: Option<String>) -> Result<Vec<manage::Remote>, String> {
    read_blocking(read_id, move || manage::remotes(Path::new(&path))).await
}

#[tauri::command]
pub(crate) async fn submodules_list(path: String, read_id: Option<String>) -> Result<Vec<manage::Submodule>, String> {
    read_blocking(read_id, move || manage::submodules(Path::new(&path))).await
}

#[tauri::command]
pub(crate) async fn lfs_locks(path: String, read_id: Option<String>) -> Result<Vec<manage::LfsLock>, String> { read_blocking(read_id, move || manage::lfs_locks(Path::new(&path))).await? }

#[tauri::command]
pub(crate) async fn lfs_info(path: String, read_id: Option<String>) -> Result<manage::LfsInfo, String> {
    read_blocking(read_id, move || manage::lfs_info(Path::new(&path))).await
}

#[tauri::command]
pub(crate) async fn sparse_info(path: String, read_id: Option<String>) -> Result<manage::SparseInfo, String> {
    read_blocking(read_id, move || manage::sparse_info(Path::new(&path))).await
}

#[tauri::command]
pub(crate) fn gitignore_templates() -> Vec<String> {
    manage::GITIGNORE_TEMPLATES.iter().map(|(n, _)| n.to_string()).collect()
}

#[tauri::command]
pub(crate) async fn push_review(path: String, read_id: Option<String>) -> Result<actions::PushReview, String> {
    read_blocking(read_id, move || {
        let dir=Path::new(&path);let lock=operation::lock_for(dir);let _guard=read_jobs::lock(&lock)?;
        actions::push_review(dir)
    }).await?
}

#[tauri::command]
pub(crate) fn cancel_job(id: String, state: State<'_, AppState>) -> bool {
    state.engine.jobs.cancel(&id)
}

#[tauri::command]
pub(crate) async fn conflict_check(path: String, target: Option<String>, read_id: Option<String>) -> Result<extras::ConflictCheck, String> {
    read_blocking(read_id, move || extras::conflict_check(Path::new(&path), target.as_deref())).await
}

/// Just the file lists (one `git status`), for staging and working-folder edits.
#[tauri::command]
pub(crate) async fn repo_status(path: String, read_id: Option<String>) -> Result<git::WorkStatus, String> {
    blocking(move || read_jobs::scope(read_id, || git::try_status(Path::new(&path)))).await?
}

#[tauri::command]
pub(crate) async fn orig_cleanup_candidates(path: String) -> Result<Vec<prefs::OrigCandidate>, String> { blocking(move || prefs::orig_cleanup_candidates(Path::new(&path))).await }

#[tauri::command]
pub(crate) async fn delete_orig_files(path: String, candidates: Vec<prefs::OrigCandidate>) -> Result<Vec<String>, String> {
    blocking(move || {
        let dir = Path::new(&path); let lock = operation::lock_for(dir); let _guard = lock.lock().unwrap_or_else(|error| error.into_inner());
        prefs::delete_orig_selected(dir, &candidates)
    }).await?
}

#[tauri::command]
pub(crate) async fn repo_summary(path: String) -> Result<extras::RepoSummary, String> {
    blocking(move || extras::repo_summary(Path::new(&path))).await
}

#[tauri::command]
pub(crate) async fn run_action(path: String, action: Action, state: State<'_, AppState>) -> Result<ActionResult, String> {
    let engine = state.engine.clone();
    blocking(move || engine.perform(&path, action)).await
}

#[tauri::command]
pub(crate) fn activity_log_snapshot(version: Option<u64>, state: State<'_, AppState>) -> Option<(u64, Vec<git::LogEntry>)> { state.engine.log_snapshot(version) }

#[tauri::command]
pub(crate) fn activity_log(state: State<'_, AppState>) -> Vec<git::LogEntry> {
    state.engine.log_entries()
}

#[tauri::command]
pub(crate) async fn sample_repo() -> Result<String, String> {
    blocking(|| sample::ensure_sample().map(|p| p.to_string_lossy().into_owned())).await?
}

#[tauri::command]
pub(crate) async fn conflict_sample() -> Result<String, String> {
    blocking(|| sample::ensure_conflict_sample().map(|p| p.to_string_lossy().into_owned())).await?
}

#[tauri::command]
pub(crate) async fn last_commit_message(path: String, read_id: Option<String>) -> Result<(String, String), String> {
    read_blocking(read_id, move || git::last_commit_message(Path::new(&path))).await
}

#[tauri::command]
pub(crate) async fn init_repo(parent: String, name: String, options: Option<manage::InitOptions>, state: State<'_, AppState>) -> Result<manage::InitOutcome, String> {
    let engine = state.engine.clone();
    let name = name.trim();
    if name.is_empty() || Path::new(name).components().count() != 1 || !matches!(Path::new(name).components().next(), Some(std::path::Component::Normal(_))) {
        return Err("Enter a folder name, without parent directories.".into());
    }
    let dir = PathBuf::from(parent).join(name);
    let nonempty = match tokio::fs::read_dir(&dir).await {
        Ok(mut entries) => entries.next_entry().await.map_err(err)?.is_some(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(error) => return Err(err(error)),
    };
    if nonempty {
        return Err("Choose an empty folder for a new repository. Existing files were not changed.".into());
    }
    let name = name.to_owned();
    tokio::fs::create_dir_all(&dir).await.map_err(err)?;
    blocking(move || {
        let opts = options.unwrap_or_default();
        let branch = if opts.branch.trim().is_empty() { "main".to_string() } else { opts.branch.trim().to_string() };
        let o = engine.git(&dir, &["init", "-b", &branch]);
        if o.ok {
            let f = manage::init_files(&dir, name.trim(), &opts, &|a: &[&str]| engine.git(&dir, a));
            if !f.ok {
                return Err(f.message());
            }
            Ok(manage::initialize_lfs(&dir, opts.lfs, &|args| engine.git(&dir, args)))
        } else {
            Err(o.message())
        }
    })
    .await?
}

/// What the Clone dialog can ask for besides the address and folder.
#[derive(serde::Deserialize, Default)]
#[derive(ts_rs::TS)]
#[ts(export, optional_fields = nullable)]
pub(crate) struct CloneOptions {
    /// Only the newest this-many commits (--depth).
    depth: Option<u32>,
    /// Only commits newer than this date, YYYY-MM-DD (--shallow-since).
    since: Option<String>,
    /// Clone just this branch (--single-branch --branch).
    branch: Option<String>,
    /// Extra flags parsed as quoted argument fields without shell expansion.
    custom_flags: Option<String>,
    /// Start with a sparse checkout (--sparse) and check out only `sparse_paths`.
    sparse: Option<bool>,
    sparse_paths: Option<Vec<String>>,
    /// Leave file contents out of the first download (--filter blob:none).
    blobless: Option<bool>,
}

/// Clone options that set configuration or name programs and folders Git uses: -c/--config,
/// -u/--upload-pack, --template and --separate-git-dir, including Git's abbreviations of the long
/// forms and short options bundled together (-qc).
pub(crate) fn refused_clone_flag(flag: &str) -> bool {
    if let Some(long) = flag.strip_prefix("--") {
        let name = long.split('=').next().unwrap_or_default();
        return !name.is_empty() && ["config", "upload-pack", "template", "separate-git-dir"].iter().any(|refused| refused.starts_with(name));
    }
    let Some(short) = flag.strip_prefix('-') else { return false };
    for c in short.chars() {
        match c {
            'c' | 'u' => return true,
            // These take a value; the rest of the argument is that value.
            'o' | 'b' | 'j' => return false,
            _ => {}
        }
    }
    false
}

/// The `git clone` arguments in a fixed order: branch, depth, progress, since, custom flags, sparse, filter.
pub(crate) fn clone_args(o: &CloneOptions) -> Result<Vec<String>, String> {
    let mut a: Vec<String> = Vec::new();
    if let Some(b) = o.branch.as_deref().map(str::trim).filter(|b| !b.is_empty()) {
        a.extend(["--single-branch".to_string(), "--branch".to_string(), b.to_string()]);
    }
    if let Some(d) = o.depth.filter(|d| *d > 0) {
        a.extend(["--depth".to_string(), d.to_string()]);
    }
    a.push("--progress".into());
    if let Some(since) = o.since.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
        a.extend(["--shallow-since".to_string(), since.to_string()]);
    }
    if let Some(flags) = o.custom_flags.as_deref() {
        let flags = external::argument_fields(flags)?;
        if let Some(flag) = flags.iter().find(|f| refused_clone_flag(f)) {
            return Err(format!("The clone option {flag} is not allowed here: it can change which programs Git runs."));
        }
        a.extend(flags);
    }
    if o.sparse == Some(true) {
        a.push("--sparse".into());
    }
    if o.blobless == Some(true) {
        a.extend(["--filter".to_string(), "blob:none".to_string()]);
    }
    Ok(a)
}

/// The folder a clone goes into: the URL's last segment without `.git`, refused when it is not a
/// plain folder name (empty, `.`, `..`, characters that are invalid in a file name, a trailing dot,
/// or a Windows device name such as CON or COM1).
pub(crate) fn clone_folder_name(url: &str) -> Result<String, String> {
    const DEVICES: &[&str] = &["CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9"];
    let name = url.trim_end_matches(['/', '\\']).rsplit(['/', '\\', ':']).next().unwrap_or("").trim_end_matches(".git").trim();
    let stem = name.split('.').next().unwrap_or("").trim_end().to_ascii_uppercase();
    let invalid = name.is_empty() || name.ends_with('.') || DEVICES.contains(&stem.as_str())
        || name.chars().any(|c| c.is_control() || "<>:\"|?*".contains(c));
    if invalid {
        return Err("GitCraft could not choose a folder name from this URL.".into());
    }
    Ok(name.to_string())
}

#[tauri::command]
pub(crate) async fn clone_repo(url: String, parent: String, options: Option<CloneOptions>, state: State<'_, AppState>) -> Result<String, String> {
    let engine = state.engine.clone();
    let parent = PathBuf::from(parent);
    tokio::fs::create_dir_all(&parent).await.map_err(err)?;
    blocking(move || {
        let opts = options.unwrap_or_default();
        let url = url.trim().to_string();
        if url.is_empty() {
            return Err("Enter a repository URL or path.".into());
        }
        let name = clone_folder_name(&url)?;
        let dest = parent.join(&name);
        let dest_s = dest.to_string_lossy().into_owned();
        let mut args: Vec<String> = vec!["clone".into()];
        args.extend(clone_args(&opts)?);
        // `--` keeps the URL and folder from ever being read as options.
        args.push("--".into());
        args.push(url);
        args.push(dest_s);
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let o = engine.job("clone", "clone", &parent, &refs);
        if !o.ok {
            return Err(o.message());
        }
        if opts.sparse == Some(true) {
            // Only the chosen paths are checked out; with none, only the files in the top folder.
            let mut set: Vec<String> = vec!["sparse-checkout".into(), "set".into(), "--no-cone".into()];
            let paths: Vec<String> = opts.sparse_paths.unwrap_or_default().into_iter().map(|p| p.trim().to_string()).filter(|p| !p.is_empty()).collect();
            if !paths.is_empty() {
                set.push("--".into());
                set.extend(paths);
            } else { set.extend(["/*".into(), "!/*/".into()]); }
            let refs: Vec<&str> = set.iter().map(String::as_str).collect();
            let s = engine.git(&dest, &refs);
            if !s.ok {
                return Err(format!("The repository was cloned, but the sparse checkout could not be set: {}", s.message()));
            }
        }
        Ok(dest.to_string_lossy().into_owned())
    })
    .await?
}

#[tauri::command]
pub(crate) fn acknowledge_job(id: String, state: State<'_, AppState>) { state.engine.jobs.acknowledge(&id); }

#[tauri::command]
pub(crate) fn cancel_read(id: String) { read_jobs::cancel(&id); }

/// Watch a repository and tell the window when something changed (rules in src/watch.rs).
#[tauri::command]
pub(crate) async fn watch_repo(path: String, app: tauri::AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let generation = Arc::new(());
    static NEXT_WATCH: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let read_id = format!("watch-{}", NEXT_WATCH.fetch_add(1, std::sync::atomic::Ordering::Relaxed));
    let watchers = state.watchers.clone();
    {
        let mut map = watchers.lock().unwrap();
        if map.contains_key(&path) { return Ok(()); }
        map.insert(path.clone(), WatchSlot { read_id: read_id.clone(), generation: generation.clone(), watcher: None });
    }
    blocking(move || {
        let key = path.clone();
        let result = read_jobs::scope(Some(read_id), || watch::start(Path::new(&path), move |full| {
            let _ = app.emit("repo-changed", serde_json::json!({ "path": key, "full": full }));
        }));
        let mut map = watchers.lock().unwrap();
        if !map.get(&path).is_some_and(|slot| Arc::ptr_eq(&slot.generation, &generation)) {
            drop(map);
            drop(result); // stop a superseded watcher outside the map lock
            return Ok(());
        }
        match result {
            Ok(watcher) => { map.get_mut(&path).unwrap().watcher = Some(watcher); Ok(()) }
            Err(error) => { map.remove(&path); Err(error) }
        }
    }).await?
}

#[tauri::command]
pub(crate) async fn unwatch_repo(path: String, state: State<'_, AppState>) -> Result<(), String> {
    let removed = state.watchers.lock().unwrap().remove(&path);
    if let Some(slot) = &removed { if slot.watcher.is_none() { read_jobs::cancel(&slot.read_id); } }
    blocking(move || drop(removed)).await
}

#[cfg(test)]
mod clone_tests {
    use super::*;

    #[test]
    fn clone_arguments_keep_a_fixed_order() {
        let all = CloneOptions {
            depth: Some(5),
            since: Some("2025-01-31".into()),
            branch: Some("dev".into()),
            custom_flags: Some("--no-tags   --quiet".into()),
            sparse: Some(true),
            sparse_paths: None,
            blobless: Some(true),
        };
        assert_eq!(
            clone_args(&all).unwrap(),
            ["--single-branch", "--branch", "dev", "--depth", "5", "--progress", "--shallow-since", "2025-01-31", "--no-tags", "--quiet", "--sparse", "--filter", "blob:none"]
        );
        assert_eq!(clone_args(&CloneOptions::default()).unwrap(), ["--progress"]);
        let blank = CloneOptions { branch: Some("  ".into()), depth: Some(0), ..Default::default() };
        assert_eq!(clone_args(&blank).unwrap(), ["--progress"]);
    }

    #[test]
    fn clone_flags_cannot_set_config_or_programs() {
        for flags in ["-c core.fsmonitor=evil", "--config core.hooksPath=/tmp", "--config=a=b", "-ccore.x=y", "-qc a=b", "-u /tmp/evil",
            "--upload-pack=/tmp/evil", "--upload-p /tmp/evil", "--template=/tmp/t", "--temp /tmp/t", "--separate-git-dir /tmp/g", "--conf x=y", "-qu evil"] {
            let options = CloneOptions { custom_flags: Some(flags.into()), ..Default::default() };
            assert!(clone_args(&options).is_err(), "{flags}");
        }
        for flags in ["--no-tags", "--recurse-submodules", "-q", "--origin upstream", "-ocustom", "-bcache", "--jobs 4", "--reference-if-able /x"] {
            let options = CloneOptions { custom_flags: Some(flags.into()), ..Default::default() };
            assert!(clone_args(&options).is_ok(), "{flags}");
        }
    }

    #[test]
    fn clone_folder_names_are_plain_names() {
        assert_eq!(clone_folder_name("https://github.com/a/repo.git").unwrap(), "repo");
        assert_eq!(clone_folder_name("git@github.com:a/repo.git/").unwrap(), "repo");
        assert_eq!(clone_folder_name(r"C:\src\local").unwrap(), "local");
        for url in ["https://example.com/a/..", "https://example.com/.", ".git", "https://example.com/a|b", "https://host/team/con.git", "https://host/team/Com1.txt", "https://host/team/repo."] {
            assert!(clone_folder_name(url).is_err(), "{url}");
        }
    }
}

//! Git actions the user can take, the activity log and undo / redo.

use crate::git::{self, LogEntry};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

/// Exact local and remote state reviewed before a destructive push.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(ts_rs::TS)]
#[ts(export)]
pub struct PushReview {
    pub branch: String,
    pub head: String,
    pub remote: String,
    pub remote_ref: String,
    pub url: String,
    pub remote_oid: String,
    pub tracking_oid: String,
    pub config: String,
}

pub fn push_review(dir: &Path) -> Result<PushReview, String> {
    let read = |args: &[&str]| { let out=git::run(dir,args); if out.ok {Ok(out.stdout.trim().to_string())} else {Err(out.message())} };
    let branch=read(&["symbolic-ref","-q","HEAD"])?;
    let head=read(&["rev-parse","--verify","HEAD"])?;
    let refs=read(&["for-each-ref","--format=%(refname)%00%(push:remotename)%00%(push:remoteref)%00%(push)","refs/heads"])?;
    let fields=refs.lines().map(|line|line.split('\0').collect::<Vec<_>>()).find(|fields|fields.first()==Some(&branch.as_str())).ok_or("The current branch is no longer available.")?;
    if fields.len()!=4 || fields[1].is_empty() {return Err("Set a single push destination for this branch before forcing a push.".into());}
    let remote=fields[1].to_string();
    let remote_ref=if !fields[2].is_empty() {fields[2].to_string()} else {
        // Default push modes can omit push:remoteref. Resolve the advertised tracking
        // ref only through Git's standard fetch mapping; refuse unfamiliar mappings.
        let mapping=read(&["config","--get-all",&format!("remote.{remote}.fetch")])?;
        let expected=format!("refs/heads/*:refs/remotes/{remote}/*");
        if !mapping.lines().any(|line|line.trim_start_matches('+')==expected) {return Err("This push mapping requires an explicit destination before force push.".into());}
        let suffix=fields[3].strip_prefix(&format!("refs/remotes/{remote}/")).ok_or("The push destination could not be resolved.")?;
        format!("refs/heads/{suffix}")
    };
    if !remote_ref.starts_with("refs/heads/") {return Err("Force push requires a branch destination.".into());}
    let urls=read(&["remote","get-url","--push","--all",&remote])?;
    if urls.lines().count()!=1 {return Err("Force push requires one explicitly reviewed push URL.".into());}
    let url=urls;
    let remote_state=read(&["ls-remote","--refs","--",&url,&remote_ref])?;
    let remote_oid=remote_state.lines().find_map(|line|line.split_once('\t').filter(|(_,reference)|*reference==remote_ref).map(|(oid,_)|oid.to_string())).unwrap_or_default();
    let tracking_oid=if fields[3].is_empty() {String::new()} else {read(&["rev-parse","--verify",fields[3]]).unwrap_or_default()};
    let config=read(&["config","--null","--list","--includes"])?;
    let config=crate::files::content_token(Some(config.as_bytes()));
    Ok(PushReview {branch,head,remote,remote_ref,url,remote_oid,tracking_oid,config})
}

fn branch_config(dir: &Path, name: &str) -> Result<Vec<(String,String)>, git::GitOut> {
    let out=git::run(dir,&["config","--local","--null","--list"]);if !out.ok {return Err(out);}
    let mut config:Vec<_>=out.stdout.split('\0').filter_map(|record| {
        let (key,value)=record.split_once('\n')?;
        let (branch,_)=key.strip_prefix("branch.")?.rsplit_once('.')?;
        (branch==name).then(||(key.to_string(),value.to_string()))
    }).collect();
    config.sort_by(|a,b|a.0.cmp(&b.0));Ok(config)
}

/// Git's answer when there is nothing to stash; the toolbar Stash treats it as an error
/// (a stash with no local changes is refused instead of reported as done).
pub const STASH_NOTHING: &str = "No local changes to save";

/// A stash's name: the typed message trimmed, else "WIP on <short branch>", or "WIP on (no branch)" when
/// HEAD is detached.
pub fn stash_message(message: &str, branch: &str) -> String {
    let typed = message.trim();
    if !typed.is_empty() {
        return typed.to_string();
    }
    let branch = branch.trim();
    format!("WIP on {}", if branch.is_empty() || branch == "HEAD" { "(no branch)" } else { branch })
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
#[derive(ts_rs::TS)]
#[ts(export, optional_fields = nullable)]
pub enum Action {
    Stage { paths: Vec<String> },
    Unstage { paths: Vec<String> },
    StageAll,
    UnstageAll,
    Discard { paths: Vec<String> },
    DiscardAll,
    Commit {
        summary: String,
        description: String,
        amend: bool,
        stage_all: bool,
        #[serde(default)]
        #[ts(as = "Option<bool>", optional)]
        skip_hooks: bool,
        /// Commit > Remove comments from commit messages (git commit --cleanup=strip).
        #[serde(default)]
        #[ts(as = "Option<bool>", optional)]
        strip_comments: bool,
    },
    Checkout { target: String },
    CheckoutRemote { remote_branch: String },
    CheckoutPullRequest { url: String, reference: String, expected: String },
    CreateBranch { name: String, at: Option<String>, checkout: bool },
    DeleteBranch { name: String },
    DeleteRemoteBranch { remote_branch: String },
    RenameBranch { old: String, new: String },
    CreateTag { name: String, at: Option<String>, message: Option<String> },
    DeleteTag { name: String },
    PushTag { name: String, remote: String },
    DeleteRemoteTag { name: String, remote: String },
    Fetch,
    Pull { mode: String },
    /// force: push even though the remote has other commits. lease (default true) keeps the
    /// safety check that the remote branch is still where we last saw it.
    Push { force: bool, #[serde(default)] lease: Option<bool>, #[serde(default)] review: Option<PushReview> },
    /// Push the selected local branch without changing the checked-out branch.
    PushBranch { branch: String },
    Stash { message: String },
    StashPop { index: usize, expected: Option<String> },
    StashApply { index: usize, expected: Option<String> },
    StashDrop { index: usize, expected: Option<String> },
    CherryPick { sha: String, #[serde(default)] mainline: Option<usize> },
    Revert { sha: String, #[serde(default)] mainline: Option<usize> },
    Reset { sha: String, mode: String },
    Merge { branch: String },
    Rebase { onto: String },
    AbortMerge,
    Sequencer { operation: String, command: String },
    AbortRebase,
    ContinueRebase,
    SkipRebase,
    /// Interactive rebase onto `base` ("--root" for the whole history) with our own todo list.
    InteractiveRebase { base: String, items: Vec<crate::rewrite::TodoItem>, expected: crate::rewrite::PlanState },
    /// Interactive cherry pick of the listed commits onto HEAD.
    InteractiveCherryPick { items: Vec<crate::rewrite::TodoItem> },
    /// Conflicts: take "ours" (current) or "theirs" (incoming) for whole files.
    TakeSide { paths: Vec<String>, side: String },
    MarkResolved { paths: Vec<String> },
    MarkConflicted { paths: Vec<String> },
    /// Write the merge tool's output and mark the file resolved.
    SaveResolved { file: String, content: String, expected: Option<String>, utf8: Option<bool> },
    FastForward { branch: String, to: String },
    SetUpstream { branch: String, upstream: String },
    StashRename { index: usize, expected: Option<String>, message: String },
    /// Background fetch of every remote (auto-fetch), optionally pruning.
    AutoFetch { prune: bool },
    FetchRemote { remote: String },
    RemoteEdit { name: String, new_name: String, url: String, push_url: String },
    RemoteRemove { name: String },
    WorktreeAdd { path: String, branch: String, new_branch: bool, start: Option<String> },
    WorktreeRemove { path: String, force: bool },
    WorktreeLock { path: String, message: String },
    WorktreeUnlock { path: String },
    SubmoduleAdd { url: String, path: String },
    SubmoduleUpdate { paths: Vec<String> },
    SubmoduleRemove { path: String },
    /// "Clean Up Local Branches": delete local branches and, optionally, their remote branches.
    DeleteBranches { names: Vec<String>, remotes: Vec<String> },
    GitflowInit { config: crate::manage::GitflowConfig },
    LfsInstall,
    /// "Compose commits with AI": commit the given groups of files one after another.
    ComposeCommits { commits: Vec<crate::ai::ComposedCommit> },
    /// Apply a .patch file (mailbox patches become commits).
    ApplyPatchFile { file: String },
    /// "Stash this file" / "Stash N files": stash only the chosen files.
    StashFiles { paths: Vec<String>, message: String },
    /// "Cherry pick file from stash": copy one file's stashed version into the working folder.
    StashTakeFile { index: usize, expected: Option<String>, path: String },
    /// "Revert this hunk": undo one hunk (or chosen lines) of a commit in the working folder.
    RevertHunk { sha: String, parent: Option<String>, file: String, hunk: usize, lines: Option<Vec<usize>> },
    /// Squash merge: bring a branch's changes in as staged changes, to commit as one commit.
    MergeSquash { branch: String },
    /// Open a file in the external diff tool (git difftool), or a conflict in the merge tool.
    ExternalDiff { file: String, staged: bool, tool: String },
    ExternalMerge { file: String, tool: String },
    LfsTrack { pattern: String },
    LfsUntrack { pattern: String },
    LfsPull,
    LfsLock { path: String },
    LfsUnlock { path: String, force: bool },
    /// Add a pattern to .gitignore; `untrack` also removes matching tracked files from the index.
    Ignore { pattern: String, untrack: Vec<String> },
    SparseSet { patterns: Vec<String> },
    SparseDisable,
    GitflowStart { kind: String, name: String },
    GitflowFinish { kind: String, name: String, keep: bool, tag_message: String },
    AddRemote { name: String, url: String },
    /// Stage, unstage or discard one hunk, or some lines of it.
    Patch { file: String, untracked: bool, op: crate::files::PatchOp, hunk: usize, lines: Option<Vec<usize>>, expected: Vec<git::DiffLine> },
    CherryPickMany { shas: Vec<String> },
    Undo,
    Redo,
}

/// Start git without waiting (external tools stay open as long as the person needs them).
fn spawn_git(dir: &Path, args: &[String]) {
    let mut c = git::git_cmd(dir);
    c.args(args);
    let _ = c.spawn();
}

#[derive(Debug, Clone, Serialize)]
#[derive(ts_rs::TS)]
#[ts(export)]
pub struct ActionResult {
    pub ok: bool,
    pub message: String,
}

mod log;
mod perform;
mod undo;
#[cfg(test)]
mod tests;

use undo::{Stacks, UndoRec};

#[derive(Default)]
pub struct Engine {
    log: Mutex<Vec<LogEntry>>,
    log_version: std::sync::atomic::AtomicU64,
    stacks: Mutex<HashMap<String, Stacks>>,
    /// Long network jobs with progress and Cancel (clone, fetch, pull, push).
    pub jobs: crate::jobs::Jobs,
}

#[derive(Debug, Clone, Serialize, Default)]
#[derive(ts_rs::TS)]
#[ts(export)]
pub struct UndoInfo {
    pub undo: Option<String>,
    pub redo: Option<String>,
}

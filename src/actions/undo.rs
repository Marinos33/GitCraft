//! Undo and redo: what each action recorded, and how to reverse or replay it.

use super::*;
use super::perform::discard_state;

#[derive(Debug, Clone)]
pub(super) enum UndoRec {
    Commit { before: String, after: String, label: String, branch: String },
    InitialCommit { after: String, branch: String, label: String },
    Checkout { from: String, to: String },
    CreateBranch { name: String, sha: String },
    DeleteBranch { name: String, sha: String, config: Vec<(String,String)> },
    Reset { before: String, after: String, mode: String, branch: String },
    CreateTag { name: String, sha: String },
    DeleteTag { name: String, sha: String },
    Discard { stash: String, paths: Option<Vec<String>>, redo_state: Option<String> },
    Patch { patch: String, cached: bool, reverse: bool, label: String },
    /// History rewritten (rebase, cherry pick, fast-forward): move the branch back with reset --keep.
    Rewrite { before: String, after: String, label: String, branch: String },
}

impl UndoRec {
    fn label(&self) -> String {
        match self {
            UndoRec::Commit { label, .. } | UndoRec::InitialCommit { label, .. } => label.clone(),
            UndoRec::Checkout { to, .. } => format!("checkout {to}"),
            UndoRec::CreateBranch { name, .. } => format!("create branch {name}"),
            UndoRec::DeleteBranch { name, .. } => format!("delete branch {name}"),
            UndoRec::Reset { .. } => "reset".into(),
            UndoRec::CreateTag { name, .. } => format!("create tag {name}"),
            UndoRec::DeleteTag { name, .. } => format!("delete tag {name}"),
            UndoRec::Discard { .. } => "discard changes".into(),
            UndoRec::Patch { label, .. } => label.clone(),
            UndoRec::Rewrite { label, .. } => label.clone(),
        }
    }
}

#[derive(Default)]
pub(super) struct Stacks {
    undo: Vec<UndoRec>,
    redo: Vec<UndoRec>,
}

impl Engine {
    pub fn undo_info(&self, repo: &str) -> UndoInfo {
        let s = self.stacks.lock().unwrap();
        match s.get(repo) {
            Some(st) => UndoInfo {
                undo: st.undo.last().map(|r| r.label()),
                redo: st.redo.last().map(|r| r.label()),
            },
            None => UndoInfo::default(),
        }
    }

    pub(super) fn push_undo(&self, repo: &str, rec: UndoRec) {
        let mut s = self.stacks.lock().unwrap();
        let st = s.entry(repo.to_string()).or_default();
        st.undo.push(rec);
        st.redo.clear();
        if st.undo.len() > 50 {
            st.undo.remove(0);
        }
    }

    pub(super) fn apply(&self, dir: &Path, rec: &UndoRec, undo: bool) -> git::GitOut {
        if let UndoRec::Commit { before, after, branch, .. } | UndoRec::Rewrite { before, after, branch, .. } = rec {
            let expected = if undo { after } else { before };
            if self.branch_ref(dir) != *branch || self.rev(dir, "HEAD") != *expected {
                return git::GitOut::failed("Return to the original branch and commit before undoing or redoing this action; the repository has changed.");
            }
        }
        if let UndoRec::InitialCommit { after, branch, .. } = rec {
            let expected = if undo { after.as_str() } else { "" };
            if self.branch_ref(dir) != *branch || self.rev(dir, "HEAD") != expected {
                return git::GitOut::failed("Return to the original branch and commit before undoing or redoing this action; the repository has changed.");
            }
        }
        match (rec, undo) {
            (UndoRec::InitialCommit { after, branch, .. }, true) => self.git(dir, &["update-ref", "-d", branch, after]),
            (UndoRec::InitialCommit { after, branch, .. }, false) => self.git(dir, &["update-ref", branch, after, &"0".repeat(after.len())]),
            (UndoRec::Commit { before, .. }, true) => self.git(dir, &["reset", "-q", "--soft", before]),
            (UndoRec::Commit { after, .. }, false) => self.git(dir, &["reset", "-q", "--soft", after]),
            (UndoRec::Checkout { from, .. }, true) => self.git(dir, &["checkout", "-q", from]),
            (UndoRec::Checkout { to, .. }, false) => self.git(dir, &["checkout", "-q", to]),
            (UndoRec::DeleteBranch { name, sha, config }, false) => {
                if self.rev(dir,&format!("refs/heads/{name}"))!=*sha || branch_config(dir,name).ok().as_ref()!=Some(config) {
                    return git::GitOut::failed("This branch or its settings changed after Undo; refusing to delete it.");
                }
                self.git(dir,&["branch","-D","--",name])
            }
            (UndoRec::DeleteBranch { name, sha, config }, true) => {
                let existing=match branch_config(dir,name) {Ok(config)=>config,Err(out)=>return out};
                if !existing.is_empty() || !self.rev(dir,&format!("refs/heads/{name}")).is_empty() {return git::GitOut::failed("The deleted branch or its settings were recreated externally. Refusing to overwrite them.");}
                let created=self.git(dir,&["branch","--",name,sha]);if !created.ok {return created;}
                for (key,value) in config {
                    let out=self.git(dir,&["config","--local","--add","--",key,value]);
                    if !out.ok {let _=self.git(dir,&["branch","-D","--",name]);return out;}
                }
                created
            }
            (UndoRec::CreateBranch { name, sha }, true) => {
                if self.rev(dir, &format!("refs/heads/{name}")) != *sha {
                    return git::GitOut::failed("This branch has changed since the recorded action; refusing to delete its new commits.");
                }
                self.git(dir, &["branch", "-D", name])
            }
            (UndoRec::CreateBranch { name, sha }, false) => {
                self.git(dir, &["branch", name, sha])
            }
            (UndoRec::Reset { before, after, mode, branch }, undo) => {
                let current_branch = git::run(dir, &["symbolic-ref", "-q", "HEAD"]).stdout.trim().to_string();
                let expected = if undo { after } else { before };
                if &current_branch != branch || self.rev(dir, "HEAD") != *expected {
                    return git::GitOut { ok: false, stdout: String::new(), stderr: "Return to the branch and commit where this reset was performed before undoing or redoing it.".into() };
                }
                // Replaying a hard reset must never destroy edits made since the original reset.
                if mode == "--hard" {
                    let status = git::run(dir, &["status", "--porcelain", "-uall"]);
                    if !status.ok { return status; }
                    if !status.stdout.is_empty() {
                        return git::GitOut { ok: false, stdout: String::new(), stderr: "Stash or commit your changes before undoing or redoing a hard reset.".into() };
                    }
                }
                self.git(dir, &["reset", "-q", mode, if undo { before } else { after }])
            }
            (UndoRec::CreateTag { name, .. }, true) | (UndoRec::DeleteTag { name, .. }, false) => self.git(dir, &["tag", "-d", name]),
            (UndoRec::CreateTag { name, sha }, false) | (UndoRec::DeleteTag { name, sha }, true) => self.git(dir, &["tag", name, sha]),
            (UndoRec::Discard { stash, .. }, true) => self.git(dir, &["stash", "apply", "-q", stash]),
            (UndoRec::Discard { paths, redo_state, .. }, false) => {
                if !matches!((redo_state.as_deref(), discard_state(dir)), (Some(expected), Ok(current)) if expected == current) {
                    return git::GitOut::failed("The working tree changed after Undo. Discard the intended files again instead of redoing this action.");
                }
                let previous = self.rev(dir, "stash@{0}");
                let mut args = vec!["stash", "push", "--include-untracked", "-m", "Auto stash before redo discard"];
                if let Some(paths) = paths { args.push("--"); args.extend(paths.iter().map(String::as_str)); }
                let out = self.git(dir, &args);
                if out.ok && self.rev(dir, "stash@{0}") == previous { return git::GitOut::failed("There are no changes to discard."); }
                if out.ok { let _ = self.git(dir, &["stash", "drop", "-q"]); }
                out
            },
            (UndoRec::Rewrite { before, .. }, true) => self.git(dir, &["reset", "-q", "--keep", before]),
            (UndoRec::Rewrite { after, .. }, false) => self.git(dir, &["reset", "-q", "--keep", after]),
            (UndoRec::Patch { patch, cached, reverse, .. }, undo) => {
                // Undo applies the patch the other way round; redo applies it again.
                let rev = if undo { !*reverse } else { *reverse };
                match crate::files::apply_patch(dir, patch, *cached, rev) {
                    Ok(()) => git::GitOut { ok: true, stdout: String::new(), stderr: String::new() },
                    Err(e) => git::GitOut { ok: false, stdout: String::new(), stderr: e },
                }
            }
        }
    }

    pub(super) fn undo(&self, repo: &str, dir: &Path) -> ActionResult {
        let rec = {
            let mut s = self.stacks.lock().unwrap();
            s.get_mut(repo).and_then(|st| st.undo.pop())
        };
        let Some(mut rec) = rec else {
            return ActionResult { ok: false, message: "Nothing to undo".into() };
        };
        let o = self.apply(dir, &rec, true);
        if o.ok {
            if let UndoRec::Discard { redo_state, .. } = &mut rec { *redo_state = discard_state(dir).ok(); }
            let label = rec.label();
            self.stacks.lock().unwrap().entry(repo.to_string()).or_default().redo.push(rec);
            ActionResult { ok: true, message: format!("Undo Successful: {label}") }
        } else {
            self.stacks.lock().unwrap().entry(repo.to_string()).or_default().undo.push(rec);
            ActionResult { ok: false, message: format!("Undo Failed: {}", o.message()) }
        }
    }

    pub(super) fn redo(&self, repo: &str, dir: &Path) -> ActionResult {
        let rec = {
            let mut s = self.stacks.lock().unwrap();
            s.get_mut(repo).and_then(|st| st.redo.pop())
        };
        let Some(rec) = rec else {
            return ActionResult { ok: false, message: "Nothing to redo".into() };
        };
        let o = self.apply(dir, &rec, false);
        if o.ok {
            let label = rec.label();
            self.stacks.lock().unwrap().entry(repo.to_string()).or_default().undo.push(rec);
            ActionResult { ok: true, message: format!("Redo Successful: {label}") }
        } else {
            self.stacks.lock().unwrap().entry(repo.to_string()).or_default().redo.push(rec);
            ActionResult { ok: false, message: format!("Redo Failed: {}", o.message()) }
        }
    }
}

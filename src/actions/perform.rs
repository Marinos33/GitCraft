//! Running one action: the Git commands behind each `Action` and the undo record it leaves.

use super::*;

impl Engine {
    pub(super) fn in_progress(&self, dir: &Path, what: &str) -> bool {
        let gd = git::run(dir, &["rev-parse", "--absolute-git-dir"]).stdout.trim().to_string();
        Path::new(&gd).join(what).exists()
    }

    pub(super) fn rev(&self, dir: &Path, r: &str) -> String {
        git::run(dir, &["rev-parse", "--verify", "-q", r]).stdout.trim().to_string()
    }

    pub(super) fn branch_ref(&self, dir: &Path) -> String {
        git::run(dir, &["symbolic-ref", "-q", "HEAD"]).stdout.trim().to_owned()
    }

    pub(super) fn head_name(&self, dir: &Path) -> String {
        let o = git::run(dir, &["symbolic-ref", "--short", "-q", "HEAD"]);
        if o.ok {
            o.stdout.trim().to_string()
        } else {
            self.rev(dir, "HEAD")
        }
    }

    /// Merge commits require an explicit mainline; reject stale or invalid choices before changing HEAD.
    pub(super) fn apply_commit(&self, dir: &Path, sha: &str, mainline: Option<usize>, revert: bool) -> ActionResult {
        let resolved = git::run(dir, &["rev-parse", "--verify", "--end-of-options", &format!("{sha}^{{commit}}")]);
        if !resolved.ok {
            return ActionResult { ok: false, message: resolved.message() };
        }
        let sha = resolved.stdout.trim();
        let parents = git::run(dir, &["rev-list", "--parents", "-n1", sha]);
        if !parents.ok {
            return ActionResult { ok: false, message: parents.message() };
        }
        let count = parents.stdout.split_whitespace().count().saturating_sub(1);
        if (count > 1 && mainline.is_none()) || mainline.is_some_and(|p| count <= 1 || p == 0 || p > count) {
            return ActionResult { ok: false, message: format!("Choose a parent of this merge commit (1–{count}).") };
        }
        let mut args = vec![if revert { "revert".to_string() } else { "cherry-pick".to_string() }];
        if revert { args.push("--no-edit".into()); }
        if let Some(parent) = mainline { args.extend(["-m".into(), parent.to_string()]); }
        args.push(sha.into());
        let out = self.git(dir, &args.iter().map(String::as_str).collect::<Vec<_>>());
        ActionResult { ok: out.ok, message: if out.ok { if revert { "Reverted commit" } else { "Cherry picked commit" }.into() } else { out.message() } }
    }

    pub fn perform(&self, repo: &str, mut action: Action) -> ActionResult {
        let dir = Path::new(repo);
        let operation = crate::operation::lock_for(dir);
        let _guard = operation.lock().unwrap_or_else(|e| e.into_inner());
        let done = |o: git::GitOut, msg: &str| -> ActionResult {
            if o.ok {
                ActionResult { ok: true, message: msg.to_string() }
            } else {
                ActionResult { ok: false, message: o.message() }
            }
        };
        match &mut action {
            Action::StashPop { index, expected } | Action::StashApply { index, expected } | Action::StashDrop { index, expected }
                | Action::StashRename { index, expected, .. } | Action::StashTakeFile { index, expected, .. } => {
                if let Some(expected) = expected {
                    let list = self.git(dir, &["stash", "list", "--format=%H"]);
                    let Some(current) = list.stdout.lines().position(|sha| sha == expected) else {
                        return done(git::GitOut::failed("The selected stash no longer exists. Refresh the stash list and try again."), "");
                    };
                    *index = current;
                }
            },
            _ => {},
        }
        if matches!(&action, Action::Merge { .. } | Action::MergeSquash { .. } | Action::Rebase { .. } | Action::CherryPick { .. } | Action::CherryPickMany { .. } | Action::InteractiveRebase { .. }) { crate::prefs::begin_orig_cleanup(dir); }
        if matches!(&action, Action::AbortMerge | Action::AbortRebase) { crate::prefs::cancel_orig_cleanup(dir); }
        let deleting_tag = matches!(&action, Action::DeleteRemoteTag { .. });
        match action {
            Action::Stage { paths } => {
                let mut a = vec!["add", "--"];
                a.extend(paths.iter().map(|s| s.as_str()));
                done(self.git(dir, &a), "Staged")
            }
            Action::Unstage { paths } => {
                let has_head = !self.rev(dir, "HEAD").is_empty();
                let mut a: Vec<&str> = if has_head { vec!["reset", "-q", "HEAD", "--"] } else { vec!["rm", "--cached", "-f", "-q", "--"] };
                a.extend(paths.iter().map(|s| s.as_str()));
                done(self.git(dir, &a), "Unstaged")
            }
            Action::StageAll => done(self.git(dir, &["add", "-A"]), "Staged all changes"),
            Action::UnstageAll => {
                if self.rev(dir, "HEAD").is_empty() {
                    done(self.git(dir, &["rm", "-r", "--cached", "-f", "-q", "."]), "Unstaged all changes")
                } else {
                    done(self.git(dir, &["reset", "-q"]), "Unstaged all changes")
                }
            }
            Action::Discard { paths } => self.discard(repo, dir, Some(paths)),
            Action::DiscardAll => self.discard(repo, dir, None),
            Action::Commit { summary, description, amend, stage_all, skip_hooks, strip_comments } => {
                let unmerged=self.git(dir, &["ls-files", "--unmerged", "-z"]);
                if !unmerged.ok { return done(unmerged, ""); }
                if !unmerged.stdout.is_empty() { return done(git::GitOut::failed("Resolve and stage every conflicted file before committing. Commit All cannot resolve conflicts automatically."), ""); }
                if summary.trim().is_empty() {
                    return ActionResult { ok: false, message: "Type a commit summary first".into() };
                }
                if stage_all {
                    let o = self.git(dir, &["add", "-A"]);
                    if !o.ok {
                        return done(o, "");
                    }
                }
                let before = self.rev(dir, "HEAD");
                let mut msg = summary.trim().to_string();
                if !description.trim().is_empty() {
                    msg.push_str("\n\n");
                    msg.push_str(description.trim());
                }
                let mut a = vec!["commit", "-q", "-m", msg.as_str()];
                if amend {
                    a.push("--amend");
                }
                if skip_hooks {
                    a.push("--no-verify"); // "Skip git hooks"
                }
                if strip_comments {
                    a.push("--cleanup=strip"); // "Remove comments from commit messages"
                }
                let o = self.git(dir, &a);
                if o.ok {
                    let after = self.rev(dir, "HEAD");
                    let branch = self.branch_ref(dir);
                    let label = if amend { "amend commit".to_string() } else { "commit".to_string() };
                    self.push_undo(repo, if before.is_empty() { UndoRec::InitialCommit { after, branch, label } }
                        else { UndoRec::Commit { before, after, branch, label } });
                }
                done(o, if amend { "Amended previous commit" } else { "Committed" })
            }
            Action::Checkout { target } => {
                let from = self.head_name(dir);
                let o = self.git(dir, &["checkout", "-q", &target]);
                if o.ok {
                    self.push_undo(repo, UndoRec::Checkout { from, to: target.clone() });
                }
                done(o, &format!("Checked out {target}"))
            }
            Action::CheckoutPullRequest { url, reference, expected } => {
                if !matches!(expected.len(), 40 | 64) || !expected.bytes().all(|c| c.is_ascii_hexdigit()) || reference.starts_with('-') {
                    return done(git::GitOut::failed("The pull request has an invalid source commit."), "");
                }
                let from = self.head_name(dir);
                let fetched = self.job(repo, "fetch", dir, &["fetch", "--no-tags", "--", &url, &reference]);
                if !fetched.ok { return done(fetched, ""); }
                if self.rev(dir, "FETCH_HEAD^{commit}") != expected {
                    return done(git::GitOut::failed("The pull request changed during checkout. Refresh it and try again."), "");
                }
                let result = self.git(dir, &["checkout", "--detach", "-q", &expected]);
                if result.ok { self.push_undo(repo, UndoRec::Checkout { from, to: expected }); }
                done(result, "Checked out the pull request's source commit")
            }
            Action::CheckoutRemote { remote_branch } => {
                let local = remote_branch.split_once('/').map_or(remote_branch.as_str(), |(_, branch)| branch).to_string();
                let selected = format!("refs/remotes/{remote_branch}");
                if !self.git(dir, &["check-ref-format", "--branch", &local]).ok || self.rev(dir, &selected).is_empty() {
                    return done(git::GitOut::failed("That remote branch is no longer available. Refresh the repository and try again."), "");
                }
                let exists = !self.rev(dir, &format!("refs/heads/{local}")).is_empty();
                if exists {
                    let upstream = self.git(dir, &["rev-parse", "--symbolic-full-name", "--verify", &format!("{local}@{{upstream}}")]);
                    if !upstream.ok || upstream.stdout.trim() != selected {
                        return done(git::GitOut::failed(&format!("Local branch '{local}' does not track '{remote_branch}'. Create a differently named local branch from the selected remote commit, or explicitly change its upstream before checking it out.")), "");
                    }
                }
                let from = self.head_name(dir);
                let o = if exists {
                    self.git(dir, &["checkout", "-q", &local])
                } else {
                    self.git(dir, &["checkout", "-q", "-b", &local, "--track", &remote_branch])
                };
                if o.ok {
                    self.push_undo(repo, UndoRec::Checkout { from, to: local.clone() });
                }
                done(o, &format!("Checked out {local}"))
            }
            Action::CreateBranch { name, at, checkout } => {
                let name = name.trim().replace(' ', "-");
                if name.is_empty() {
                    return ActionResult { ok: false, message: "Branch name is empty".into() };
                }
                let at = at.unwrap_or_else(|| "HEAD".into());
                let o = self.git(dir, &["branch", &name, &at]);
                if !o.ok {
                    return done(o, "");
                }
                let sha = self.rev(dir, &format!("refs/heads/{name}"));
                self.push_undo(repo, UndoRec::CreateBranch { name: name.clone(), sha });
                if checkout {
                    let o = self.git(dir, &["checkout", "-q", &name]);
                    return done(o, &format!("Created and checked out {name}"));
                }
                ActionResult { ok: true, message: format!("Created branch {name}") }
            }
            Action::DeleteBranch { name } => {
                let sha = self.rev(dir, &format!("refs/heads/{name}"));
                let config=match branch_config(dir,&name) {Ok(config)=>config,Err(out)=>return done(out, "")};
                let o = self.git(dir, &["branch", "-D", "--", &name]);
                if o.ok {
                    self.push_undo(repo, UndoRec::DeleteBranch { name: name.clone(), sha, config });
                }
                done(o, &format!("Deleted {name}"))
            }
            Action::DeleteRemoteBranch { remote_branch } => {
                let (remote, b) = remote_branch.split_once('/').unwrap_or(("origin", &remote_branch));
                done(self.git(dir, &["push", remote, "--delete", b]), &format!("Deleted {remote_branch}"))
            }
            Action::RenameBranch { old, new } => {
                let new = new.trim().replace(' ', "-");
                done(self.git(dir, &["branch", "-m", &old, &new]), &format!("Renamed {old} to {new}"))
            }
            Action::CreateTag { name, at, message } => {
                let at = at.unwrap_or_else(|| "HEAD".into());
                let o = match message.as_deref().filter(|m| !m.trim().is_empty()) {
                    Some(m) => self.git(dir, &["tag", "-a", &name, &at, "-m", m]),
                    None => self.git(dir, &["tag", &name, &at]),
                };
                if o.ok {
                    let sha = self.rev(dir, &format!("refs/tags/{name}"));
                    self.push_undo(repo, UndoRec::CreateTag { name: name.clone(), sha });
                }
                done(o, &format!("Created tag {name}"))
            }
            Action::DeleteTag { name } => {
                let sha = self.rev(dir, &format!("refs/tags/{name}"));
                let o = self.git(dir, &["tag", "-d", &name]);
                if o.ok {
                    self.push_undo(repo, UndoRec::DeleteTag { name: name.clone(), sha });
                }
                done(o, &format!("Deleted tag {name}"))
            }
            Action::PushTag { name, remote } | Action::DeleteRemoteTag { name, remote } => {
                let deleting = deleting_tag;
                let full = format!("refs/tags/{name}");
                if !git::run(dir, &["check-ref-format", &full]).ok {
                    return ActionResult { ok: false, message: "Choose a valid tag name.".into() };
                }
                if !git::run(dir, &["remote"]).stdout.lines().any(|r| r == remote) {
                    return ActionResult { ok: false, message: "The selected remote no longer exists.".into() };
                }
                if !deleting && !git::run(dir, &["show-ref", "--verify", "--quiet", &full]).ok {
                    return ActionResult { ok: false, message: format!("Local tag {name} no longer exists.") };
                }
                // Full refspecs avoid ambiguity when a branch and tag share a name; never force a tag.
                let spec = if deleting { format!(":{full}") } else { format!("{full}:{full}") };
                done(self.job(repo, "push", dir, &["push", "--progress", "--", &remote, &spec]),
                    &format!("{} tag {name} {} {remote}", if deleting { "Deleted" } else { "Pushed" }, if deleting { "from" } else { "to" }))
            }
            Action::Fetch => done(self.job(repo, "fetch", dir, &["fetch", "--all", "--prune", "--progress"]), "Fetched all remotes"),
            Action::Pull { mode } => match mode.as_str() {
                "fetch" => done(self.job(repo, "fetch", dir, &["fetch", "--all", "--prune", "--progress"]), "Fetched all remotes"),
                "ffonly" => done(self.job(repo, "pull", dir, &["pull", "--ff-only", "--progress"]), "Pulled (fast-forward only)"),
                "rebase" => done(self.job(repo, "pull", dir, &["pull", "--rebase", "--progress"]), "Pulled (rebase)"),
                _ => {
                    let args = crate::extras::merge_args(dir, "pull", &["--no-rebase", "--progress"]);
                    done(self.job(repo, "pull", dir, &args.iter().map(String::as_str).collect::<Vec<_>>()), "Pulled")
                }
            },
            Action::Push { force, lease, review } => {
                if force {
                    let Some(review)=review else {return done(git::GitOut::failed("Review the branch and destination again before forcing a push."), "");};
                    let current=match push_review(dir) {Ok(current)=>current,Err(error)=>return done(git::GitOut::failed(&error), "")};
                    if current != review {return done(git::GitOut::failed("The branch, commit, push destination or remote tip changed after confirmation. Review the push again."), "");}
                    let expected=if lease==Some(false) {&review.remote_oid} else {&review.tracking_oid};
                    let guard=format!("--force-with-lease={}:{}",review.remote_ref,expected);
                    let refspec=format!("{}:{}",review.head,review.remote_ref);
                    // Push the reviewed object and URL even if an external process changes HEAD/config
                    // after validation. An explicit remote lease also protects the final network race.
                    let out=self.jobs.run(&format!("{repo}|push"),dir,&["push","--progress",&guard,"--",&review.url,&refspec]);
                    self.record(&format!("git push --force-with-lease {} {}",review.remote,review.remote_ref),&out);
                    return done(out,"Pushed the reviewed commit");
                }
                let branch = git::run(dir, &["symbolic-ref", "-q", "HEAD"]).stdout.trim().strip_prefix("refs/heads/").unwrap_or_default().to_string();
                if branch.is_empty() {
                    return ActionResult { ok: false, message: "Push is not available when in a detached HEAD state".into() };
                }
                let upstream = git::run(dir, &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"]);
                let o = if upstream.ok {
                    self.job(repo, "push", dir, &["push", "--progress"])
                } else {
                    let remotes = git::run(dir, &["remote"]).stdout;
                    let remote = if remotes.lines().any(|r| r == "origin") {
                        "origin".to_string()
                    } else if let Some(r) = remotes.lines().next() {
                        r.to_string()
                    } else {
                        return ActionResult { ok: false, message: "This repository has no remote to push to. Add one first.".into() };
                    };
                    self.job(repo, "push", dir, &["push", "-u", "--progress", &remote, &format!("refs/heads/{branch}:refs/heads/{branch}")])
                };
                done(o, &format!("Pushed {branch}"))
            }
            Action::PushBranch { branch } => {
                let source = format!("refs/heads/{branch}");
                let resolved = git::run(dir, &["rev-parse", "--verify", "--end-of-options", &source]);
                if !resolved.ok {
                    return ActionResult { ok: false, message: format!("Local branch {branch} no longer exists.") };
                }
                let remotes = git::run(dir, &["remote"]).stdout;
                let configured = git::run(dir, &["config", "--get", &format!("branch.{branch}.remote")]).stdout.trim().to_string();
                let remote = if !configured.is_empty() {
                    configured
                } else if remotes.lines().any(|r| r == "origin") {
                    "origin".into()
                } else {
                    remotes.lines().next().unwrap_or("").to_string()
                };
                if remote.is_empty() || !remotes.lines().any(|r| r == remote) {
                    return ActionResult { ok: false, message: "Choose a server remote for this branch before starting a pull request.".into() };
                }
                let merge = git::run(dir, &["config", "--get", &format!("branch.{branch}.merge")]).stdout.trim().to_string();
                let destination = if merge.is_empty() { source.clone() } else { merge };
                if !destination.starts_with("refs/heads/") {
                    return ActionResult { ok: false, message: "The branch upstream must point to a remote branch.".into() };
                }
                let refspec = format!("{source}:{destination}");
                done(self.job(repo, "push", dir, &["push", "--set-upstream", "--progress", &remote, &refspec]), &format!("Pushed {branch}"))
            }
            Action::Stash { message } => {
                let unmerged=self.git(dir, &["ls-files", "--unmerged", "-z"]);
                if !unmerged.ok { return done(unmerged, ""); }
                if !unmerged.stdout.is_empty() || ["MERGE_HEAD","CHERRY_PICK_HEAD","REVERT_HEAD","rebase-merge","rebase-apply","sequencer"].iter().any(|state|self.in_progress(dir,state)) {
                    return done(git::GitOut::failed("Finish or abort the current merge, rebase or replay before stashing. Its conflict state was kept."), "");
                }
                // Stage everything (new files too), then `stash push` without --include-untracked, named by stash_message.
                let add = self.git(dir, &["add", "-A"]);
                if !add.ok {
                    return done(add, "");
                }
                let branch = git::run(dir, &["symbolic-ref", "--short", "-q", "HEAD"]).stdout.trim().to_string();
                let msg = stash_message(&message, &branch);
                let o = self.git(dir, &["stash", "push", "-m", &msg]);
                // A stash with no local changes is reported as an error, not as success.
                if o.ok && o.stdout.trim() == STASH_NOTHING {
                    return ActionResult { ok: false, message: STASH_NOTHING.into() };
                }
                done(o, "Stashed changes")
            }
            Action::StashPop { index, .. } => done(self.git(dir, &["stash", "pop", "-q", &format!("stash@{{{index}}}")]), "Popped stash"),
            Action::StashApply { index, .. } => done(self.git(dir, &["stash", "apply", "-q", &format!("stash@{{{index}}}")]), "Applied stash"),
            Action::StashDrop { index, .. } => done(self.git(dir, &["stash", "drop", "-q", &format!("stash@{{{index}}}")]), "Deleted stash"),
            Action::CherryPick { sha, mainline } => self.apply_commit(dir, &sha, mainline, false),
            Action::Revert { sha, mainline } => self.apply_commit(dir, &sha, mainline, true),
            Action::Reset { sha, mode } => {
                let before = self.rev(dir, "HEAD");
                let flag = match mode.as_str() {
                    "soft" => "--soft",
                    "hard" => "--hard",
                    "mixed" => "--mixed",
                    _ => return ActionResult { ok: false, message: "Choose a soft, mixed, or hard reset.".into() },
                };
                let resolved = git::run(dir, &["rev-parse", "--verify", "--end-of-options", &format!("{sha}^{{commit}}")]);
                if !resolved.ok { return done(resolved, ""); }
                let sha = resolved.stdout.trim().to_string();
                let o = self.git(dir, &["reset", "-q", flag, &sha]);
                if o.ok {
                    self.push_undo(repo, UndoRec::Reset { before, after: sha.clone(), mode: flag.to_string(), branch: git::run(dir, &["symbolic-ref", "-q", "HEAD"]).stdout.trim().to_string() });
                }
                done(o, "Reset branch")
            }
            Action::Merge { branch } => {
                let args = crate::extras::merge_args(dir, "merge", &["--no-edit", &branch]);
                done(self.git(dir, &args.iter().map(String::as_str).collect::<Vec<_>>()), &format!("Merged {branch}"))
            }
            Action::Rebase { onto } => {
                let before = self.rev(dir, "HEAD");
                let o = self.git(dir, &["rebase", "--autostash", &onto]);
                if o.ok {
                    let after = self.rev(dir, "HEAD");
                    self.push_undo(repo, UndoRec::Rewrite { before, after, branch: self.branch_ref(dir), label: format!("rebase onto {onto}") });
                }
                done(o, &format!("Rebased onto {onto}"))
            }
            Action::AbortMerge => done(self.git(dir, &["merge", "--abort"]), "Merge aborted"),
            Action::Sequencer { operation, command } => {
                if !["cherry-pick", "revert", "am"].contains(&operation.as_str()) || !["continue", "skip", "abort"].contains(&command.as_str()) {
                    return done(git::GitOut::failed("Unsupported recovery command."), "");
                }
                let flag = format!("--{command}");
                done(self.git(dir, &["-c", "core.editor=true", &operation, &flag]), "Git operation updated")
            },
            Action::AbortRebase => done(self.git(dir, &["rebase", "--abort"]), "Rebase aborted"),
            Action::ContinueRebase => done(self.git(dir, &["-c", "core.editor=true", "rebase", "--continue"]), "Rebase continued"),
            Action::SkipRebase => done(self.git(dir, &["rebase", "--skip"]), "Skipped commit"),
            Action::AutoFetch { prune } => {
                let o = if prune { self.git(dir, &["fetch", "--all", "--prune", "--quiet"]) } else { self.git(dir, &["fetch", "--all", "--quiet"]) };
                // A failed background fetch only asks to fetch by hand.
                if o.ok {
                    ActionResult { ok: true, message: String::new() }
                } else {
                    ActionResult { ok: false, message: "Autofetch failed. Fetch manually for more information.".into() }
                }
            }
            Action::FetchRemote { remote } => done(self.job(repo, "fetch", dir, &["fetch", "--prune", "--progress", &remote]), &format!("Fetched {remote}")),
            Action::RemoteEdit { name, new_name, url, push_url } => {
                let mut name = name;
                if !new_name.trim().is_empty() && new_name.trim() != name {
                    let o = self.git(dir, &["remote", "rename", &name, new_name.trim()]);
                    if !o.ok {
                        return done(o, "");
                    }
                    name = new_name.trim().to_string();
                }
                let o = self.git(dir, &["remote", "set-url", &name, url.trim()]);
                if !o.ok {
                    return done(o, "");
                }
                if push_url.trim().is_empty() || push_url.trim() == url.trim() {
                    let clear = self.git(dir, &["config", "--local", "--unset-all", &format!("remote.{name}.pushurl")]);
                    // git config exits 5 when the optional key does not exist.
                    if !clear.ok && git::run(dir, &["config", "--get-all", &format!("remote.{name}.pushurl")]).ok {
                        return done(clear, "");
                    }
                }
                if !push_url.trim().is_empty() && push_url.trim() != url.trim() {
                    return done(self.git(dir, &["remote", "set-url", "--push", &name, push_url.trim()]), "Remote updated");
                }
                ActionResult { ok: true, message: "Remote updated".into() }
            }
            Action::RemoteRemove { name } => done(self.git(dir, &["remote", "remove", &name]), &format!("Removed remote {name}")),
            Action::WorktreeAdd { path, branch, new_branch, start } => {
                let o = if new_branch {
                    let start = start.unwrap_or_else(|| "HEAD".into());
                    self.git(dir, &["worktree", "add", "-b", branch.trim(), path.trim(), &start])
                } else {
                    self.git(dir, &["worktree", "add", path.trim(), branch.trim()])
                };
                done(o, &format!("Worktree created at {}", path.trim()))
            }
            Action::WorktreeRemove { path, force } => {
                let o = if force { self.git(dir, &["worktree", "remove", "--force", &path]) } else { self.git(dir, &["worktree", "remove", &path]) };
                done(o, "Worktree removed")
            }
            Action::WorktreeLock { path, message } => {
                let o = if message.trim().is_empty() { self.git(dir, &["worktree", "lock", &path]) } else { self.git(dir, &["worktree", "lock", "--reason", message.trim(), &path]) };
                done(o, "Worktree locked")
            }
            Action::WorktreeUnlock { path } => done(self.git(dir, &["worktree", "unlock", &path]), "Worktree unlocked"),
            Action::SubmoduleAdd { url, path } => {
                let mut a = vec!["submodule", "add", url.trim()];
                if !path.trim().is_empty() {
                    a.push(path.trim());
                }
                done(self.git(dir, &a), "Submodule added")
            }
            Action::SubmoduleUpdate { paths } => {
                let mut a = vec!["submodule", "update", "--init", "--recursive", "--"];
                a.extend(paths.iter().map(|s| s.as_str()));
                done(self.git(dir, &a), "Submodules updated")
            }
            Action::SubmoduleRemove { path } => {
                let module = dir.join(&path);
                if module.join(".git").exists() {
                    let status = git::run(&module, &["status", "--porcelain", "--untracked-files=all", "--ignored=matching", "--ignore-submodules=none"]);
                    if !status.ok || !status.stdout.trim().is_empty() {
                        return done(git::GitOut::failed("The submodule has local work. Commit, stash, or back it up before removing it."), "");
                    }
                } else if module.exists() && std::fs::read_dir(&module).map(|mut items| items.next().is_some()).unwrap_or(true) {
                    return done(git::GitOut::failed("The submodule folder contains local files. Back them up before removing it."), "");
                }
                let o = self.git(dir, &["submodule", "deinit", "--", &path]);
                if !o.ok {
                    return done(o, "");
                }
                done(self.git(dir, &["rm", "--", &path]), &format!("Removed submodule {path}"))
            }
            Action::DeleteBranches { names, remotes } => {
                let mut failed = Vec::new();
                for n in &names {
                    let sha = self.rev(dir, &format!("refs/heads/{n}"));
                    let config=match branch_config(dir,n) {Ok(config)=>config,Err(_)=>{failed.push(n.clone());continue;}};
                    if self.git(dir, &["branch", "-D", "--", n]).ok {
                        self.push_undo(repo, UndoRec::DeleteBranch { name: n.clone(), sha, config });
                    } else {
                        failed.push(n.clone());
                    }
                }
                for r in &remotes {
                    let (remote, b) = r.split_once('/').unwrap_or(("origin", r));
                    if !self.git(dir, &["push", remote, "--delete", b]).ok {
                        failed.push(r.clone());
                    }
                }
                if failed.is_empty() {
                    ActionResult { ok: true, message: format!("Deleted {} branch{}", names.len() + remotes.len(), if names.len() + remotes.len() == 1 { "" } else { "es" }) }
                } else {
                    ActionResult { ok: false, message: format!("Could not delete: {}", failed.join(", ")) }
                }
            }
            Action::ExternalDiff { file, staged, tool } => {
                let mut a = vec!["--literal-pathspecs".to_string(), "difftool".into(), "-y".into()];
                if staged {
                    a.push("--cached".into());
                }
                if !tool.trim().is_empty() {
                    a.push(format!("--tool={}", tool.trim()));
                }
                a.extend(["--".into(), file.clone()]);
                spawn_git(dir, &a);
                ActionResult { ok: true, message: format!("Opening {file} in the diff tool") }
            }
            Action::ExternalMerge { file, tool } => {
                let mut a = vec!["--literal-pathspecs".to_string(), "mergetool".into(), "-y".into()];
                if !tool.trim().is_empty() {
                    a.push(format!("--tool={}", tool.trim()));
                }
                a.extend(["--".into(), file.clone()]);
                spawn_git(dir, &a);
                ActionResult { ok: true, message: format!("Opening {file} in the merge tool") }
            }
            Action::ComposeCommits { commits } => {
                let before = self.rev(dir, "HEAD");
                let o = if before.is_empty() { self.git(dir, &["rm", "-r", "--cached", "--ignore-unmatch", "--", "."]) }
                    else { self.git(dir, &["reset", "-q"]) };
                if !o.ok { return done(o, ""); }
                let failed = |i: usize, o: git::GitOut| {
                    let after = self.rev(dir, "HEAD");
                    if before != after && !after.is_empty() {
                        let branch = self.branch_ref(dir);
                        let label = "partially composed commits".into();
                        self.push_undo(repo, if before.is_empty() { UndoRec::InitialCommit { after, branch, label } }
                            else { UndoRec::Commit { before: before.clone(), after, branch, label } });
                    }
                    ActionResult { ok: false, message: format!("Commit {} of {}: {}. Completed commits remain and can be undone.", i + 1, commits.len(), o.message()) }
                };
                for (i, c) in commits.iter().enumerate() {
                    let mut add = vec!["add", "-A", "--"];
                    add.extend(c.files.iter().map(|s| s.as_str()));
                    let o = self.git(dir, &add);
                    if !o.ok {
                        return failed(i, o);
                    }
                    let msg = if c.description.trim().is_empty() { c.summary.trim().to_string() } else { format!("{}\n\n{}", c.summary.trim(), c.description.trim()) };
                    let o = self.git(dir, &["commit", "-q", "-m", &msg]);
                    if !o.ok {
                        return failed(i, o);
                    }
                }
                let after = self.rev(dir, "HEAD");
                if before != after && !after.is_empty() {
                    let branch = self.branch_ref(dir);
                    let label = format!("{} composed commits", commits.len());
                    self.push_undo(repo, if before.is_empty() { UndoRec::InitialCommit { after, branch, label } }
                        else { UndoRec::Commit { before, after, branch, label } });
                }
                ActionResult { ok: true, message: format!("Created {} commits", commits.len()) }
            }
            Action::StashFiles { paths, message } => {
                // A stash is refused as a whole if any chosen file is unchanged.
                for p in &paths {
                    if git::run(dir, &["status", "--porcelain", "--untracked-files=all", "--", p]).stdout.trim().is_empty() {
                        return ActionResult { ok: false, message: "cannot stash changes - one of the files does not have any changes to stash.".into() };
                    }
                }
                let mut args = vec!["stash", "push", "--include-untracked"];
                let msg = message.trim().to_string();
                if !msg.is_empty() {
                    args.extend(["-m", msg.as_str()]);
                }
                args.push("--");
                args.extend(paths.iter().map(|p| p.as_str()));
                let n = paths.len();
                done(self.git(dir, &args), &format!("Stashed {n} file{}", if n == 1 { "" } else { "s" }))
            }
            Action::StashTakeFile { index, path, .. } => {
                // Tracked files are in the stash commit; new files are in its third parent.
                let status = git::run(dir, &["status", "--porcelain=v1", "--untracked-files=all", "--ignored=matching", "--", &path]);
                if !status.ok { return done(status, ""); }
                if !status.stdout.is_empty() {
                    return ActionResult { ok: false, message: format!("Stash or commit the current changes to {path} before applying its stashed version.") };
                }
                let stash = format!("stash@{{{index}}}");
                let mut o = self.git(dir, &["checkout", &stash, "--", &path]);
                if !o.ok {
                    let untracked = format!("{stash}^3");
                    let o2 = self.git(dir, &["checkout", &untracked, "--", &path]);
                    if o2.ok {
                        o = o2;
                    }
                }
                done(o, &format!("Took {path} from the stash"))
            }
            Action::RevertHunk { sha, parent, file, hunk, lines } => {
                let diff = match git::diff_text_checked(dir, &git::DiffSource::Commit(sha.clone()), &file, parent.as_deref(), false) { Ok(diff) => diff, Err(error) => return done(git::GitOut::failed(&error), "") };
                let Some(patch) = crate::files::partial_patch(&diff, hunk, lines.as_deref(), true) else {
                    return ActionResult { ok: false, message: "That change could not be found in the commit.".into() };
                };
                let current = std::fs::read_to_string(dir.join(&file)).unwrap_or_default();
                if let Err(e) = crate::files::check_hunks_exact(&current, &patch, true) {
                    return ActionResult { ok: false, message: format!("Could not revert the hunk: {e} ({file} has changed since that commit)") };
                }
                self.log_note(&format!("git apply --reverse (revert hunk of {file} from {})", &sha[..sha.len().min(7)]));
                match crate::files::apply_patch(dir, &patch, false, true) {
                    Ok(()) => {
                        self.push_undo(repo, UndoRec::Patch { patch, cached: false, reverse: true, label: format!("revert hunk in {file}") });
                        ActionResult { ok: true, message: format!("Reverted the change in {file}; it is now an uncommitted change") }
                    }
                    Err(e) => ActionResult { ok: false, message: format!("The hunk could not be reverted because {file} has changed since: {e}") },
                }
            }
            Action::MergeSquash { branch } => {
                let args = crate::extras::merge_args(dir, "merge", &["--squash", &branch]);
                done(self.git(dir, &args.iter().map(String::as_str).collect::<Vec<_>>()), &format!("Squashed {branch}; review and commit the staged changes"))
            }
            Action::ApplyPatchFile { file } => {
                let o = crate::extras::apply_patch_file(dir, Path::new(&file), &|a: &[&str]| self.git(dir, a));
                done(o, "Patch applied")
            }
            Action::LfsInstall => done(self.git(dir, &["lfs", "install", "--local"]), "Git LFS enabled for this repository"),
            Action::LfsTrack { pattern } => done(self.git(dir, &["lfs", "track", pattern.trim()]), &format!("LFS now tracks {}", pattern.trim())),
            Action::LfsUntrack { pattern } => done(self.git(dir, &["lfs", "untrack", pattern.trim()]), &format!("LFS no longer tracks {}", pattern.trim())),
            Action::LfsPull => done(self.git(dir, &["lfs", "pull"]), "LFS files downloaded"),
            Action::LfsLock { path } => done(self.git(dir, &["lfs", "lock", &path]), &format!("Locked {path}")),
            Action::LfsUnlock { path, force } => {
                let o = if force { self.git(dir, &["lfs", "unlock", "--force", &path]) } else { self.git(dir, &["lfs", "unlock", &path]) };
                done(o, &format!("Unlocked {path}"))
            }
            Action::Ignore { pattern, untrack } => {
                if let Err(e) = crate::manage::ignore(dir, pattern.trim()) {
                    return ActionResult { ok: false, message: e };
                }
                if !untrack.is_empty() {
                    let mut a = vec!["rm", "-r", "-q", "--cached", "--ignore-unmatch", "--"];
                    a.extend(untrack.iter().map(|s| s.as_str()));
                    let o = self.git(dir, &a);
                    if !o.ok {
                        return done(o, "");
                    }
                }
                ActionResult { ok: true, message: format!("Added {} to .gitignore", pattern.trim()) }
            }
            Action::SparseSet { patterns } => {
                let mut a = vec!["sparse-checkout", "set", "--no-cone"];
                a.extend(patterns.iter().map(|s| s.as_str()));
                done(self.git(dir, &a), "Sparse checkout updated")
            }
            Action::SparseDisable => done(self.git(dir, &["sparse-checkout", "disable"]), "Sparse checkout turned off"),
            Action::GitflowInit { config } => done(crate::manage::gitflow_init(dir, &config, &|a: &[&str]| self.git(dir, a)), "Gitflow has been initialized"),
            Action::GitflowStart { kind, name } => {
                done(crate::manage::gitflow_start(dir, &kind, &name, &|a: &[&str]| self.git(dir, a)), &format!("Started {kind}: '{}'", name.trim()))
            }
            Action::GitflowFinish { kind, name, keep, tag_message } => {
                done(crate::manage::gitflow_finish(dir, &kind, &name, keep, &tag_message, &|a: &[&str]| self.git(dir, a)), &format!("Finished {kind}: '{name}'"))
            }
            Action::InteractiveRebase { base, items, expected } => {
                if crate::rewrite::plan_state(dir, &base).ok().as_ref() != Some(&expected) {
                    return done(git::GitOut::failed("The branch, tip, or base changed after this plan was prepared. Reopen the planner and review the current history."), "");
                }
                let before = self.rev(dir, "HEAD");
                let o = crate::rewrite::interactive_rebase(dir, &base, &items);
                self.record(&format!("git rebase -i {base} ({} steps)", items.len()), &o);
                if o.ok {
                    let after = self.rev(dir, "HEAD");
                    self.push_undo(repo, UndoRec::Rewrite { before, after, branch: self.branch_ref(dir), label: "interactive rebase".into() });
                    ActionResult { ok: true, message: "Rebase complete".into() }
                } else if self.in_progress(dir, "rebase-merge") {
                    ActionResult { ok: false, message: "The rebase stopped on a conflict. Resolve it, then Continue Rebase (or Abort Rebase).".into() }
                } else {
                    ActionResult { ok: false, message: o.message() }
                }
            }
            Action::InteractiveCherryPick { items } => {
                let before = self.rev(dir, "HEAD");
                let o = crate::rewrite::interactive_cherry_pick(dir, &items, &|a: &[&str]| self.git(dir, a));
                let after = self.rev(dir, "HEAD");
                if after != before {
                    self.push_undo(repo, UndoRec::Rewrite { before, after, branch: self.branch_ref(dir), label: "interactive cherry pick".into() });
                }
                done(o, "Cherry pick complete")
            }
            Action::TakeSide { paths, side } => {
                let (flag,stage)=match side.as_str() {"ours"=>("--ours","2"),"theirs"=>("--theirs","3"),_=>return done(git::GitOut::failed("Choose current or incoming conflict contents."), "")};
                let mut selected=Vec::new();
                for path in &paths {
                    let entries=self.git(dir,&["ls-files","--unmerged","-z","--",path]);if !entries.ok {return done(entries, "");}
                    let stages:Vec<_>=entries.stdout.split('\0').filter_map(|record|record.split_once('\t').and_then(|(header,_)|header.split_whitespace().nth(2))).collect();
                    if stages.is_empty() {return done(git::GitOut::failed(&format!("{path} is no longer an unresolved file. Refresh its changes first.")), "");}
                    selected.push((path,stages.contains(&stage)));
                }
                for (path,present) in selected {
                    if !present {
                        let removed=self.git(dir,&["rm","-q","--",path]);if !removed.ok {return done(removed, "");}
                    } else {
                        let full=dir.join(path);
                        let backup=match std::fs::symlink_metadata(&full) {
                            Ok(metadata)=>{
                                let saved=match tempfile::NamedTempFile::new() {Ok(file)=>file,Err(error)=>return done(git::GitOut::failed(&error.to_string()), "")};
                                let moved=metadata.file_type().is_symlink();
                                let copied=if moved {std::fs::remove_file(saved.path()).and_then(|_|std::fs::rename(&full,saved.path()))} else {std::fs::copy(&full,saved.path()).map(|_|())};
                                if let Err(error)=copied {return done(git::GitOut::failed(&format!("Could not back up {path}: {error}")), "");}
                                Some((saved,moved))
                            }
                            Err(error) if error.kind()==std::io::ErrorKind::NotFound=>None,
                            Err(error)=>return done(git::GitOut::failed(&error.to_string()), ""),
                        };
                        let mut result=self.git(dir,&["checkout",flag,"--",path]);
                        if result.ok {result=self.git(dir,&["add","--",path]);}
                        if !result.ok {
                            if let Some((saved,moved))=backup {
                                let restored=(|| {
                                    if std::fs::symlink_metadata(&full).is_ok_and(|metadata|metadata.file_type().is_symlink()) {std::fs::remove_file(&full)?;}
                                    if moved {if full.exists() {std::fs::remove_file(&full)?;}std::fs::rename(saved.path(),&full)} else {std::fs::copy(saved.path(),&full).map(|_|())}
                                })();
                                if let Err(error)=restored {
                                    let recovery=saved.path().display().to_string();
                                    let (file,recovery_path)=saved.into_parts();drop(file);std::mem::forget(recovery_path);
                                    return done(git::GitOut::failed(&format!("{}\nCould not restore {path}: {error}. Original contents are retained at {recovery}",result.message())), "");
                                }
                            }
                            return done(result, "");
                        }
                    }
                }
                ActionResult {ok:true,message:if side=="theirs" {"Took incoming".into()} else {"Took current".into()}}
            }
            Action::MarkResolved { paths } => {
                let mut a = vec!["add", "--"];
                a.extend(paths.iter().map(|s| s.as_str()));
                done(self.git(dir, &a), "Marked resolved")
            }
            Action::MarkConflicted { paths } => {
                let mut a = vec!["checkout", "-m", "--"];
                a.extend(paths.iter().map(|s| s.as_str()));
                done(self.git(dir, &a), "Marked conflicted")
            }
            Action::SaveResolved { file, content, expected, utf8 } => {
                if !utf8.unwrap_or(false) && crate::rewrite::conflict_info(dir,&file).decoding_loss {
                    return ActionResult {ok:false,message:"ENCODING_DECODE_LOSS: Conflict content contains undecodable bytes. Choose its repository encoding or explicitly convert the reviewed output to UTF-8.".into()};
                }
                if let Err(e) = crate::files::save_checked_mode(dir, &file, &content, expected.as_deref(), utf8.unwrap_or(false)) {
                    return ActionResult { ok: false, message: e };
                }
                done(self.git(dir, &["add", "--", &file]), &format!("Saved and resolved {file}"))
            }
            Action::FastForward { branch, to } => {
                let head = git::run(dir, &["symbolic-ref", "--short", "-q", "HEAD"]).stdout.trim().to_string();
                let before = self.rev(dir, &format!("refs/heads/{branch}"));
                let o = if head == branch {
                    self.git(dir, &["merge", "--ff-only", &to])
                } else {
                    // Moves a branch that isn't checked out; git refuses anything but a fast-forward.
                    self.git(dir, &["fetch", ".", &format!("{to}:{branch}")])
                };
                if o.ok && head == branch {
                    let after = self.rev(dir, "HEAD");
                    self.push_undo(repo, UndoRec::Rewrite { before, after, branch: self.branch_ref(dir), label: format!("fast-forward {branch}") });
                }
                done(o, &format!("Fast-forwarded {branch} to {to}"))
            }
            Action::SetUpstream { branch, upstream } => {
                if upstream.trim().is_empty() {
                    done(self.git(dir, &["branch", "--unset-upstream", &branch]), "Upstream removed")
                } else {
                    let up = upstream.trim().to_string();
                    done(self.git(dir, &["branch", &format!("--set-upstream-to={up}"), &branch]), &format!("{branch} now tracks {up}"))
                }
            }
            Action::StashRename { index, message, .. } => {
                // git has no rename: drop the entry and store the same stash commit with the new message.
                let sha = self.rev(dir, &format!("stash@{{{index}}}"));
                if sha.is_empty() {
                    return ActionResult { ok: false, message: "That stash no longer exists".into() };
                }
                let o = self.git(dir, &["stash", "drop", "-q", &format!("stash@{{{index}}}")]);
                if !o.ok {
                    return done(o, "");
                }
                done(self.git(dir, &["stash", "store", "-m", message.trim(), &sha]), "Stash message updated")
            }
            Action::AddRemote { name, url } => done(self.git(dir, &["remote", "add", name.trim(), url.trim()]), "Remote added"),
            Action::Patch { file, untracked, op, hunk, lines, expected } => {
                let what = if lines.is_some() { "lines" } else { "hunk" };
                let verb = match op {
                    crate::files::PatchOp::Stage => "stage",
                    crate::files::PatchOp::Unstage => "unstage",
                    crate::files::PatchOp::Discard => "discard",
                };
                self.log_note(&format!("git apply ({verb} {what} of {file})"));
                match crate::files::apply_part_checked(dir, &file, untracked, op, hunk, lines.as_deref(), &expected) {
                    Ok((patch, cached, reverse)) => {
                        // Staging is easy to reverse by hand; discards are recorded for Undo.
                        if op == crate::files::PatchOp::Discard {
                            self.push_undo(repo, UndoRec::Patch { patch, cached, reverse, label: format!("discard {what} in {file}") });
                        }
                        let past = match op {
                            crate::files::PatchOp::Stage => "Staged",
                            crate::files::PatchOp::Unstage => "Unstaged",
                            crate::files::PatchOp::Discard => "Discarded",
                        };
                        ActionResult { ok: true, message: format!("{past} {what}") }
                    }
                    Err(e) => ActionResult { ok: false, message: e },
                }
            }
            Action::CherryPickMany { shas } => {
                let mut a = vec!["cherry-pick"];
                a.extend(shas.iter().map(|s| s.as_str()));
                done(self.git(dir, &a), &format!("Cherry picked {} commits", shas.len()))
            }
            Action::Undo => self.undo(repo, dir),
            Action::Redo => self.redo(repo, dir),
        }
    }

    pub(super) fn discard(&self, repo: &str, dir: &Path, paths: Option<Vec<String>>) -> ActionResult {
        // Keep an automatic stash so the discard can be undone.
        let mut a = vec!["stash", "push", "--include-untracked", "-m", "Auto stash before discard"];
        if let Some(p) = &paths {
            a.push("--");
            a.extend(p.iter().map(|s| s.as_str()));
        }
        let previous = self.rev(dir, "stash@{0}");
        let o = self.git(dir, &a);
        if !o.ok {
            return ActionResult { ok: false, message: o.message() };
        }
        let sha = self.rev(dir, "stash@{0}");
        if sha.is_empty() || sha == previous {
            return ActionResult { ok: false, message: "There are no changes to discard.".into() };
        }
        let _ = self.git(dir, &["stash", "drop", "-q"]);
        self.push_undo(repo, UndoRec::Discard { stash: sha, paths, redo_state: None });
        ActionResult { ok: true, message: "Discarded changes".into() }
    }
}

// Conservatively refuse replay after any intervening tracked or untracked edit.
pub(super) fn discard_state(dir: &Path) -> Result<String, String> {
    use std::hash::{Hash, Hasher};
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    for args in [vec!["diff", "--binary", "--no-ext-diff", "--no-textconv", "--no-color", "HEAD"], vec!["diff", "--cached", "--binary", "--no-ext-diff", "--no-textconv", "--no-color"], vec!["ls-files", "--others", "--exclude-standard", "-z"]] {
        let output = git::git_cmd(dir).args(&args).output().map_err(|e| e.to_string())?;
        if !output.status.success() { return Err(String::from_utf8_lossy(&output.stderr).into_owned()); }
        output.stdout.hash(&mut hash);
        if args[0] == "ls-files" {
            let names = std::str::from_utf8(&output.stdout).map_err(|_| "Cannot safely verify a non-UTF-8 file name for Redo.")?;
            for name in names.split('\0').filter(|s| !s.is_empty()) { std::fs::read(dir.join(name)).map_err(|e| e.to_string())?.hash(&mut hash); }
        }
    }
    Ok(format!("{:x}", hash.finish()))
}

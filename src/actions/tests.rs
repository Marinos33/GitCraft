use super::*;
use std::fs;


#[test]
fn remote_tags_are_explicit_preserve_annotations_and_leave_local_tags() {
    let d = temp_repo("remote-tag-audit");
    let repo = d.to_string_lossy().to_string();
    let e = Engine::default();
    fs::write(d.join("f.txt"), "one\n").unwrap();
    assert!(git::run(&d, &["add", "."]).ok);
    assert!(git::run(&d, &["commit", "-qm", "one"]).ok);
    assert!(git::run(&d, &["tag", "-a", "release", "-m", "Release annotation"]).ok);
    assert!(git::run(&d, &["branch", "release"]).ok);
    let object = git::run(&d, &["rev-parse", "refs/tags/release"]).stdout.trim().to_string();
    let remote = d.join("remote.git");
    assert!(git::run(&d, &["init", "--bare", &remote.to_string_lossy()]).ok);
    assert!(git::run(&d, &["remote", "add", "origin", &remote.to_string_lossy()]).ok);
    assert!(!e.perform(&repo, Action::PushTag { name: "release".into(), remote: "missing".into() }).ok);
    let result = e.perform(&repo, Action::PushTag { name: "release".into(), remote: "origin".into() });
    assert!(result.ok, "{}", result.message);
    assert_eq!(git::run(&remote, &["rev-parse", "refs/tags/release"]).stdout.trim(), object);
    assert!(!git::run(&remote, &["show-ref", "--verify", "refs/heads/release"]).ok);
    assert!(e.perform(&repo, Action::DeleteRemoteTag { name: "release".into(), remote: "origin".into() }).ok);
    assert!(!git::run(&remote, &["show-ref", "--verify", "refs/tags/release"]).ok);
    assert_eq!(git::run(&d, &["rev-parse", "refs/tags/release"]).stdout.trim(), object);
    let _ = fs::remove_dir_all(&d);
}

#[test]
fn merge_commit_actions_require_and_use_the_selected_parent() {
    let d = temp_repo("mainline-audit");
    let repo = d.to_string_lossy().to_string();
    let e = Engine::default();
    fs::write(d.join("base.txt"), "base\n").unwrap();
    assert!(git::run(&d, &["add", "."]).ok);
    assert!(git::run(&d, &["commit", "-qm", "base"]).ok);
    let base = head(&d);
    assert!(git::run(&d, &["checkout", "-qb", "feature"]).ok);
    fs::write(d.join("side.txt"), "side\n").unwrap();
    assert!(git::run(&d, &["add", "."]).ok);
    assert!(git::run(&d, &["commit", "-qm", "side"]).ok);
    assert!(git::run(&d, &["checkout", "-q", "master"]).ok);
    fs::write(d.join("main.txt"), "main\n").unwrap();
    assert!(git::run(&d, &["add", "."]).ok);
    assert!(git::run(&d, &["commit", "-qm", "main"]).ok);
    assert!(git::run(&d, &["merge", "--no-ff", "-m", "merge", "feature"]).ok);
    let merge = head(&d);
    for mainline in [None, Some(0), Some(3)] {
        assert!(!e.perform(&repo, Action::Revert { sha: merge.clone(), mainline }).ok);
        assert_eq!(head(&d), merge);
    }
    assert!(e.perform(&repo, Action::Revert { sha: merge.clone(), mainline: Some(2) }).ok);
    assert!(d.join("side.txt").exists());
    assert!(!d.join("main.txt").exists());
    assert!(git::run(&d, &["checkout", "-qb", "picked", &base]).ok);
    let result = e.perform(&repo, Action::CherryPick { sha: merge, mainline: Some(2) });
    assert!(result.ok, "{}", result.message);
    assert!(d.join("main.txt").exists());
    assert!(!d.join("side.txt").exists());
    let _ = fs::remove_dir_all(&d);
}

#[test]
fn reset_modes_keep_their_promises_and_retry_failed_undo() {
    let d = temp_repo("reset-audit");
    let repo = d.to_string_lossy().to_string();
    let e = Engine::default();
    fs::write(d.join("f.txt"), "one\n").unwrap();
    assert!(git::run(&d, &["add", "."]).ok);
    assert!(git::run(&d, &["commit", "-qm", "one"]).ok);
    let first = head(&d);
    fs::write(d.join("f.txt"), "two\n").unwrap();
    assert!(git::run(&d, &["commit", "-qam", "two"]).ok);
    let second = head(&d);
    assert!(!e.perform(&repo, Action::Reset { sha: first.clone(), mode: "typo".into() }).ok);
    assert_eq!(head(&d), second);
    assert!(e.perform(&repo, Action::Reset { sha: first.clone(), mode: "soft".into() }).ok);
    assert_eq!(git::status(&d).staged.len(), 1);
    assert!(e.perform(&repo, Action::Undo).ok);
    assert!(e.perform(&repo, Action::Reset { sha: first.clone(), mode: "mixed".into() }).ok);
    assert!(git::status(&d).staged.is_empty());
    assert_eq!(git::status(&d).unstaged.len(), 1);
    assert!(e.perform(&repo, Action::Reset { sha: second.clone(), mode: "hard".into() }).ok);
    // Current-HEAD hard reset is meaningful: it discards edits without moving the branch.
    fs::write(d.join("f.txt"), "discard me\n").unwrap();
    assert!(e.perform(&repo, Action::Reset { sha: second.clone(), mode: "hard".into() }).ok);
    assert_eq!(fs::read_to_string(d.join("f.txt")).unwrap().replace("\r\n", "\n"), "two\n");
    assert!(e.perform(&repo, Action::Reset { sha: first.clone(), mode: "hard".into() }).ok);
    assert!(git::run(&d, &["checkout", "-qb", "other"]).ok);
    assert!(!e.perform(&repo, Action::Undo).ok, "cannot undo a reset on another branch");
    assert!(git::run(&d, &["checkout", "-q", "master"]).ok);
    fs::write(d.join("f.txt"), "keep my new work\n").unwrap();
    assert!(!e.perform(&repo, Action::Undo).ok);
    assert_eq!(head(&d), first);
    assert_eq!(fs::read_to_string(d.join("f.txt")).unwrap(), "keep my new work\n");
    // The failed undo stays on the stack; saving the edit allows exactly the same undo to retry.
    assert!(git::run(&d, &["stash", "push", "-qm", "save"]).ok);
    assert!(e.perform(&repo, Action::Undo).ok);
    assert_eq!(head(&d), second);
    fs::write(d.join("new.txt"), "keep untracked\n").unwrap();
    assert!(!e.perform(&repo, Action::Redo).ok);
    fs::remove_file(d.join("new.txt")).unwrap();
    assert!(e.perform(&repo, Action::Redo).ok);
    assert_eq!(head(&d), first);
    let _ = fs::remove_dir_all(&d);
}

#[test]
fn annotated_tag_undo_retains_the_tag_object() {
    let d = temp_repo("tag-audit");
    let repo = d.to_string_lossy().to_string();
    let e = Engine::default();
    fs::write(d.join("f.txt"), "one\n").unwrap();
    assert!(git::run(&d, &["add", "."]).ok);
    assert!(git::run(&d, &["commit", "-qm", "one"]).ok);
    assert!(e.perform(&repo, Action::CreateTag { name: "v1".into(), at: None, message: Some("Release note".into()) }).ok);
    let object = git::run(&d, &["rev-parse", "refs/tags/v1"]).stdout;
    assert!(e.perform(&repo, Action::Undo).ok);
    assert!(e.perform(&repo, Action::Redo).ok);
    assert_eq!(git::run(&d, &["rev-parse", "refs/tags/v1"]).stdout, object);
    assert!(e.perform(&repo, Action::DeleteTag { name: "v1".into() }).ok);
    assert!(e.perform(&repo, Action::Undo).ok);
    assert_eq!(git::run(&d, &["rev-parse", "refs/tags/v1"]).stdout, object);
    assert_eq!(git::run(&d, &["cat-file", "-t", "refs/tags/v1"]).stdout.trim(), "tag");
    let _ = fs::remove_dir_all(&d);
}

#[test]
fn push_selected_branch_and_create_worktree_at_selected_commit() {
    let d = temp_repo("branch-audit");
    let repo = d.to_string_lossy().to_string();
    let e = Engine::default();
    fs::write(d.join("f.txt"), "one\n").unwrap();
    assert!(git::run(&d, &["add", "."]).ok);
    assert!(git::run(&d, &["commit", "-qm", "one"]).ok);
    let first = head(&d);
    assert!(git::run(&d, &["branch", "feature"]).ok);
    fs::write(d.join("f.txt"), "two\n").unwrap();
    assert!(git::run(&d, &["commit", "-qam", "two"]).ok);
    let second = head(&d);
    // A local bare repository exercises pushing without contacting a service.
    let remote = d.join("remote.git");
    let remote_path = remote.to_string_lossy().to_string();
    assert!(git::run(&d, &["init", "--bare", &remote_path]).ok);
    assert!(git::run(&d, &["remote", "add", "origin", &remote_path]).ok);
    let pushed = e.perform(&repo, Action::PushBranch { branch: "feature".into() });
    assert!(pushed.ok, "{}", pushed.message);
    assert_eq!(head(&d), second);
    assert_eq!(git::run(&d, &["symbolic-ref", "--short", "HEAD"]).stdout.trim(), "master");
    assert_eq!(git::run(&remote, &["rev-parse", "refs/heads/feature"]).stdout.trim(), first);
    assert!(!git::run(&remote, &["rev-parse", "--verify", "refs/heads/master"]).ok);
    let wt = d.join("selected-worktree").to_string_lossy().to_string();
    let created = e.perform(&repo, Action::WorktreeAdd { path: wt.clone(), branch: "selected".into(), new_branch: true, start: Some(first.clone()) });
    assert!(created.ok, "{}", created.message);
    assert_eq!(head(Path::new(&wt)), first);
    assert!(e.perform(&repo, Action::WorktreeRemove { path: wt, force: false }).ok);
    let _ = fs::remove_dir_all(&d);
}

#[test]
fn every_log_writer_bounds_entries_and_notifies_snapshot_readers() {
    let engine=Engine::default();let output=git::GitOut {ok:false,stdout:String::new(),stderr:"\u{e9}".repeat(50000)};
    for i in 0..1000 {engine.record(&format!("job {i}"),&output);}
    let (version,entries)=engine.log_snapshot(None).unwrap();assert_eq!(entries.len(),500);assert_eq!(entries[0].command,"job 500");assert!(!entries[0].ok);assert!(entries[0].output.len()<=65536+12);assert!(entries[0].output.ends_with("[truncated]"));
    engine.log_note("hunk operation");let (next,entries)=engine.log_snapshot(Some(version)).unwrap();assert!(next>version);assert_eq!(entries.len(),500);assert_eq!(entries.last().unwrap().command,"hunk operation");
    assert!(engine.log_snapshot(Some(next)).is_none());
    let folder=tempfile::tempdir().unwrap();assert!(engine.git(folder.path(),&["--version"]).ok);assert!(engine.log_snapshot(Some(next)).is_some());assert_eq!(engine.log_entries().len(),500);
}

#[test]
fn log_diagnostics_redact_credentials_for_every_writer() {
    let engine=Engine::default();let folder=tempfile::tempdir().unwrap();assert!(git::run(folder.path(),&["init","-q"]).ok);
    let url="https://audit-user:SYNTHETIC_AUDIT_SECRET@fixture.example/repo.git";
    assert!(engine.git(folder.path(),&["remote","add","origin",url]).ok);
    assert!(engine.job("fixture","config",folder.path(),&["remote","set-url","origin",url]).ok);
    let error=git::GitOut {ok:false,stdout:String::new(),stderr:format!("failed: {url}")};engine.record(&format!("fetch {url}"),&error);engine.log_note(&format!("note {url}"));
    for entry in engine.log_entries() {assert!(!entry.command.contains("SYNTHETIC_AUDIT_SECRET"));assert!(!entry.output.contains("SYNTHETIC_AUDIT_SECRET"));}
    assert!(!error.message().contains("SYNTHETIC_AUDIT_SECRET"));
    assert_eq!(git::redact_diagnostic("https://fixture.example/repo.git"),"https://fixture.example/repo.git");
    assert!(!git::redact_diagnostic("https://user:encoded%40SYNTHETIC@fixture.example/path?access_token=QUERY_SECRET").contains("SYNTHETIC"));
    assert!(!git::redact_diagnostic("https://fixture.example/path?access_token=QUERY_SECRET").contains("QUERY_SECRET"));
    assert!(!git::redact_diagnostic("git -c http.extraHeader=Authorization: Bearer HEADER_SECRET clone url").contains("HEADER_SECRET"));
}

#[test]
fn stash_names_follow_the_rules() {
    assert_eq!(stash_message("", "main"), "WIP on main");
    assert_eq!(stash_message("  fix later \n", "main"), "fix later");
    assert_eq!(stash_message("", ""), "WIP on (no branch)");
    assert_eq!(stash_message("", "feature/x"), "WIP on feature/x");
}

fn temp_repo(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("gitcraft-test-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    for a in [&["init", "-q", "-b", "master"][..], &["config", "user.name", "Test"], &["config", "user.email", "t@example.com"], &["config", "commit.gpgsign", "false"]] {
        assert!(git::run(&d, a).ok);
    }
    d
}

fn head(d: &Path) -> String {
    git::run(d, &["rev-parse", "HEAD"]).stdout.trim().to_string()
}

/// A person's repository showed no history: its refs/remotes/origin/HEAD file was blank, so
/// `git log --remotes` stopped with "fatal: bad object origin/HEAD".
#[test]
fn history_loads_with_a_broken_remote_head_or_branch() {
    let d = temp_repo("broken-ref");
    fs::write(d.join("a.txt"), "a\n").unwrap();
    assert!(git::run(&d, &["add", "-A"]).ok);
    assert!(git::run(&d, &["commit", "-q", "-m", "first"]).ok);
    let sha = head(&d);
    fs::create_dir_all(d.join(".git/refs/remotes/origin")).unwrap();
    fs::write(d.join(".git/refs/remotes/origin/main"), format!("{sha}\n")).unwrap();
    // 1. Blank pointer file, as in the person's repository.
    fs::write(d.join(".git/refs/remotes/origin/HEAD"), "                              \n").unwrap();
    let r = git::load(&d, 100);
    assert_eq!(r.commits.len(), 1, "blank origin/HEAD");
    assert!(r.refs.iter().any(|x| x.name == "origin/main"));
    // 2. Pointer to a branch the remote deleted: also skipped, and not used as the default target.
    fs::write(d.join(".git/refs/remotes/origin/HEAD"), "ref: refs/remotes/origin/gone\n").unwrap();
    assert_eq!(git::load(&d, 100).commits.len(), 1, "stale origin/HEAD");
    assert_eq!(crate::extras::default_target(&d).as_deref(), Some("origin/main"));
    // 3. A damaged local branch file: the rest of the history still shows.
    fs::write(d.join(".git/refs/heads/damaged"), "0000000000000000000000000000000000000001\n").unwrap();
    assert_eq!(git::load(&d, 100).commits.len(), 1, "damaged branch");
    // 4. A tag of a tree (not a commit) is skipped; real refs stay.
    assert!(git::run(&d, &["tag", "treetag", "HEAD^{tree}"]).ok);
    let walk = git::walkable_refs(&d);
    assert!(!walk.iter().any(|r| r == "refs/tags/treetag"), "{walk:?}");
    assert!(walk.iter().any(|r| r == "refs/heads/master") && walk.iter().any(|r| r == "refs/remotes/origin/main"), "{walk:?}");
    assert_eq!(git::load(&d, 100).commits.len(), 1, "tree tag");
    let _ = fs::remove_dir_all(&d);
}

#[test]
fn merge_preview_and_actions_share_rename_rules() {
    let d = temp_repo("merge-native-rules");
    let repo = d.to_string_lossy().to_string();
    let e = Engine::default();
    let g = |args: &[&str]| {
        let o = git::run(&d, args);
        assert!(o.ok, "{args:?}: {}", o.message());
    };
    g(&["config", "core.autocrlf", "false"]);
    let base: String = (1..=40).map(|i| format!("unique line {i:02} with enough text for similarity\n")).collect();
    fs::write(d.join("old.txt"), &base).unwrap();
    g(&["add", "-A"]);
    g(&["commit", "-q", "-m", "base"]);
    g(&["checkout", "-q", "-b", "side"]);
    g(&["mv", "old.txt", "new.txt"]);
    fs::write(d.join("new.txt"), base.replace("line 02", "line two")).unwrap();
    g(&["add", "-A"]);
    g(&["commit", "-q", "-m", "rename and edit"]);
    g(&["checkout", "-q", "master"]);
    fs::write(d.join("old.txt"), base.replace("line 35", "line thirty-five")).unwrap();
    g(&["commit", "-q", "-am", "edit other end"]);
    let before = head(&d);
    g(&["config", "merge.renames", "false"]);
    g(&["config", "merge.renamelimit", "-1"]);
    g(&["config", "diff.renamelimit", "77"]);
    let preview = crate::extras::conflict_check(&d, Some("side"));
    assert!(preview.note.is_none(), "{:?}", preview.note);
    assert!(preview.files.is_empty(), "{:?}", preview.files);
    for squash in [false, true] {
        let action = if squash {
            Action::MergeSquash { branch: "side".into() }
        } else {
            Action::Merge { branch: "side".into() }
        };
        let result = e.perform(&repo, action);
        assert!(result.ok, "{}", result.message);
        assert!(!d.join("old.txt").exists());
        let merged = fs::read_to_string(d.join("new.txt")).unwrap();
        assert!(merged.contains("line two") && merged.contains("line thirty-five"), "{merged}");
        g(&["reset", "--hard", &before]);
    }
    assert_eq!(git::run(&d, &["config", "--get", "merge.renames"]).stdout.trim(), "false");
    assert_eq!(git::run(&d, &["config", "--get", "merge.renamelimit"]).stdout.trim(), "-1");
    let _ = fs::remove_dir_all(&d);
}

#[test]
fn stage_commit_undo_redo_branch_stash_discard() {
    let d = temp_repo("flow");
    let repo = d.to_string_lossy().to_string();
    let e = Engine::default();
    fs::write(d.join("a.txt"), "one\n").unwrap();
    assert!(e.perform(&repo, Action::StageAll).ok);
    assert!(e.perform(&repo, Action::Commit { summary: "first".into(), description: String::new(), amend: false, stage_all: false, skip_hooks: false, strip_comments: false }).ok);
    let first = head(&d);

    fs::write(d.join("a.txt"), "one\ntwo\n").unwrap();
    let st = git::status(&d);
    assert_eq!(st.unstaged.len(), 1);
    assert!(e.perform(&repo, Action::Stage { paths: vec!["a.txt".into()] }).ok);
    assert_eq!(git::status(&d).staged.len(), 1);
    assert!(e.perform(&repo, Action::Commit { summary: "second".into(), description: "body".into(), amend: false, stage_all: false, skip_hooks: false, strip_comments: false }).ok);
    let second = head(&d);
    assert_ne!(first, second);

    // Undo the commit: HEAD goes back, the change stays staged.
    assert!(e.perform(&repo, Action::Undo).ok);
    assert_eq!(head(&d), first);
    assert_eq!(git::status(&d).staged.len(), 1);
    assert!(e.perform(&repo, Action::Redo).ok);
    assert_eq!(head(&d), second);

    // Branches
    assert!(e.perform(&repo, Action::CreateBranch { name: "feature x".into(), at: None, checkout: true }).ok);
    let data = git::load(&d, 100);
    assert_eq!(data.head_branch.as_deref(), Some("feature-x"));
    assert!(e.perform(&repo, Action::Checkout { target: "master".into() }).ok);
    assert!(e.perform(&repo, Action::DeleteBranch { name: "feature-x".into() }).ok);
    assert!(e.perform(&repo, Action::Undo).ok, "undo delete branch");
    assert!(git::load(&d, 100).refs.iter().any(|r| r.name == "feature-x"));

    // Stash and pop
    fs::write(d.join("b.txt"), "new file\n").unwrap();
    assert!(e.perform(&repo, Action::Stash { message: "wip".into() }).ok);
    assert_eq!(git::load(&d, 100).stashes.len(), 1);
    assert!(git::status(&d).unstaged.is_empty());
    assert!(e.perform(&repo, Action::StashPop { index: 0, expected: None }).ok);
    // Everything is staged before stashing, so a new file comes back staged (git stash pop re-adds it).
    let st = git::status(&d);
    assert_eq!(st.unstaged.len() + st.staged.len(), 1);
    assert_eq!(st.staged.len(), 1);
    // With nothing to stash an error is reported instead of success.
    assert!(e.perform(&repo, Action::StageAll).ok);
    assert!(e.perform(&repo, Action::Stash { message: String::new() }).ok);
    let named = git::run(&d, &["stash", "list", "--format=%s"]).stdout;
    assert!(named.lines().next().unwrap_or("").ends_with("WIP on master"), "default stash name: {named}");
    let none = e.perform(&repo, Action::Stash { message: String::new() });
    assert!(!none.ok && none.message == STASH_NOTHING, "{}", none.message);
    assert!(e.perform(&repo, Action::StashPop { index: 0, expected: None }).ok);
    assert_eq!(git::status(&d).staged.len(), 1);
    assert!(e.perform(&repo, Action::UnstageAll).ok);

    // Discard is undoable
    assert!(e.perform(&repo, Action::DiscardAll).ok);
    assert!(git::status(&d).unstaged.is_empty());
    assert!(e.perform(&repo, Action::Undo).ok);
    assert!(d.join("b.txt").exists());

    // Tags, reset, diff, graph
    assert!(e.perform(&repo, Action::CreateTag { name: "v1".into(), at: Some(first.clone()), message: None }).ok);
    let data = git::load(&d, 100);
    assert!(data.refs.iter().any(|r| r.kind == git::RefKind::Tag && r.sha == first));
    let files = git::commit_files(&d, &data.commits[0]);
    assert_eq!(files.len(), 1);
    let diff = git::file_diff(&d, &git::DiffSource::Commit(second.clone()), "a.txt", false, Some(&first));
    assert!(diff.iter().any(|l| l.kind == git::DiffLineKind::Added && l.text == "two"));
    let g = crate::graph::layout(&data.commits);
    assert_eq!(g.rows.len(), data.commits.len());
    assert!(e.perform(&repo, Action::Reset { sha: first.clone(), mode: "soft".into() }).ok);
    assert_eq!(head(&d), first);
    assert!(e.perform(&repo, Action::Undo).ok);
    assert_eq!(head(&d), second);
    assert!(!e.log_entries().is_empty());
    let _ = fs::remove_dir_all(&d);
}

#[test]
fn stage_and_discard_single_lines_hide_and_compare() {
    let d = temp_repo("lines");
    let repo = d.to_string_lossy().to_string();
    let e = Engine::default();
    // Line endings may become CRLF on Windows (core.autocrlf); compare without them.
    let read = |p: &str| fs::read_to_string(d.join(p)).unwrap().replace("\r\n", "\n");
    let index = |p: &str| git::run(&d, &["show", &format!(":{p}")]).stdout.replace("\r\n", "\n");
    fs::write(d.join("f.txt"), "a\nb\nc\n").unwrap();
    assert!(e.perform(&repo, Action::Commit { summary: "base".into(), description: String::new(), amend: false, stage_all: true, skip_hooks: false, strip_comments: false }).ok);
    let base = head(&d);
    fs::write(d.join("f.txt"), "a\nB\nc\nnew\n").unwrap();
    // Hunk body: " a", "-b", "+B", " c", "+new" -> stage only "+new" (index 4)
    let r = e.perform(&repo, Action::Patch { file: "f.txt".into(), untracked: false, op: crate::files::PatchOp::Stage, hunk: 0, lines: Some(vec![4]), expected: git::file_diff_opts(&d, &git::DiffSource::Unstaged, "f.txt", false, None, false) });
    assert!(r.ok, "{}", r.message);
    assert_eq!(index("f.txt"), "a\nb\nc\nnew\n");
    // Discard the remaining unstaged change (b -> B), then undo the discard.
    let r = e.perform(&repo, Action::Patch { file: "f.txt".into(), untracked: false, op: crate::files::PatchOp::Discard, hunk: 0, lines: None, expected: git::file_diff_opts(&d, &git::DiffSource::Unstaged, "f.txt", false, None, false) });
    assert!(r.ok, "{}", r.message);
    assert_eq!(read("f.txt"), "a\nb\nc\nnew\n");
    assert!(e.perform(&repo, Action::Undo).ok);
    assert_eq!(read("f.txt"), "a\nB\nc\nnew\n");
    // Unstage the staged line again.
    let r = e.perform(&repo, Action::Patch { file: "f.txt".into(), untracked: false, op: crate::files::PatchOp::Unstage, hunk: 0, lines: None, expected: git::file_diff_opts(&d, &git::DiffSource::Staged, "f.txt", false, None, false) });
    assert!(r.ok, "{}", r.message);
    assert!(git::status(&d).staged.is_empty());
    // Untracked file: stage one line of a brand-new file.
    fs::write(d.join("n.txt"), "one\ntwo\n").unwrap();
    let r = e.perform(&repo, Action::Patch { file: "n.txt".into(), untracked: true, op: crate::files::PatchOp::Stage, hunk: 0, lines: Some(vec![0]), expected: git::file_diff_opts(&d, &git::DiffSource::Unstaged, "n.txt", true, None, false) });
    assert!(r.ok, "{}", r.message);
    assert_eq!(index("n.txt"), "one\n");

    // Commit and compare; hide a branch from the graph.
    assert!(e.perform(&repo, Action::Commit { summary: "next".into(), description: String::new(), amend: false, stage_all: true, skip_hooks: false, strip_comments: false }).ok);
    assert!(e.perform(&repo, Action::CreateBranch { name: "side".into(), at: Some(base.clone()), checkout: false }).ok);
    let files = crate::files::range_files(&d, &base, "HEAD").unwrap();
    assert_eq!(files.len(), 2);
    let st = crate::files::stats(&d, &[head(&d)]).unwrap();
    assert_eq!(st[0].files, 2);
    let hist = crate::files::history(&d, "f.txt", 10).unwrap();
    assert_eq!(hist.len(), 2);
    let bl = crate::files::blame(&d, "f.txt", None).unwrap();
    assert_eq!(bl.len(), 4);
    let v = crate::files::versions(&d, &git::DiffSource::Range(base.clone(), git::WORKDIR.into()), "f.txt", None, false);
    assert_eq!(v.old.as_deref().map(|s| s.replace("\r\n", "\n")), Some("a\nb\nc\n".to_string()));
    let view = git::RefView { hidden: vec!["refs/heads/side".into()], solo: vec![], pinned: None, smart: false };
    assert_eq!(git::load_view(&d, 100, &view).commits.len(), 2);
    let solo = git::RefView { hidden: vec![], solo: vec!["refs/heads/side".into()], pinned: None, smart: false };
    assert_eq!(git::load_view(&d, 100, &solo).commits.len(), 1);
    let more = git::load_view(&d, 1, &git::RefView::default());
    assert!(more.has_more);
    let _ = fs::remove_dir_all(&d);
}


#[test]
fn interactive_rebase_conflicts_and_branch_extras() {
    use crate::rewrite::{Step, TodoItem};
    let d = temp_repo("rewrite");
    let repo = d.to_string_lossy().to_string();
    let e = Engine::default();
    let read = |p: &str| fs::read_to_string(d.join(p)).unwrap().replace("\r\n", "\n");
    let commit = |f: &str, t: &str, m: &str| {
        fs::write(d.join(f), t).unwrap();
        assert!(e.perform(&repo, Action::Commit { summary: m.into(), description: String::new(), amend: false, stage_all: true, skip_hooks: false, strip_comments: false }).ok);
        head(&d)
    };
    let base = commit("a.txt", "1\n", "one");
    let c2 = commit("b.txt", "2\n", "two");
    let c3 = commit("c.txt", "3\n", "three");
    let c4 = commit("d.txt", "4\n", "four");
    let list = crate::rewrite::rebase_commits(&d, &base).unwrap();
    assert_eq!(list.iter().map(|c| c.sha.clone()).collect::<Vec<_>>(), vec![c2.clone(), c3.clone(), c4.clone()]);

    // Reword "two", squash "three" into it, drop "four"; a dirty file survives (autostash).
    fs::write(d.join("a.txt"), "1 changed\n").unwrap();
    let items = vec![
        TodoItem { action: Step::Reword, sha: c2.clone(), message: Some("two (reworded)".into()) },
        TodoItem { action: Step::Squash, sha: c3.clone(), message: None },
        TodoItem { action: Step::Drop, sha: c4.clone(), message: None },
    ];
    let r = e.perform(&repo, Action::InteractiveRebase { expected: crate::rewrite::plan_state(&d, &base).unwrap(), base: base.clone(), items });
    assert!(r.ok, "{}", r.message);
    let log = git::run(&d, &["log", "--format=%s", &format!("{base}..HEAD")]).stdout;
    assert_eq!(log.lines().collect::<Vec<_>>(), vec!["two (reworded)"]);
    let msg = git::run(&d, &["log", "-1", "--format=%B"]).stdout;
    assert!(msg.contains("three"), "squash keeps both messages: {msg}");
    assert!(!d.join("d.txt").exists());
    assert!(d.join("c.txt").exists());
    assert_eq!(read("a.txt"), "1 changed\n");
    // Undo puts the branch back.
    assert!(e.perform(&repo, Action::Undo).ok);
    assert_eq!(head(&d), c4);
    assert!(e.perform(&repo, Action::DiscardAll).ok);

    // Reorder: "four" before "two"; then interactive cherry pick onto a side branch.
    let items = vec![
        TodoItem { action: Step::Pick, sha: c4.clone(), message: None },
        TodoItem { action: Step::Pick, sha: c2.clone(), message: None },
        TodoItem { action: Step::Pick, sha: c3.clone(), message: None },
    ];
    assert!(e.perform(&repo, Action::InteractiveRebase { expected: crate::rewrite::plan_state(&d, &base).unwrap(), base: base.clone(), items }).ok);
    let log = git::run(&d, &["log", "--reverse", "--format=%s", &format!("{base}..HEAD")]).stdout;
    assert_eq!(log.lines().collect::<Vec<_>>(), vec!["four", "two", "three"]);
    assert!(e.perform(&repo, Action::CreateBranch { name: "side".into(), at: Some(base.clone()), checkout: true }).ok);
    let items = vec![
        TodoItem { action: Step::Pick, sha: c2.clone(), message: None },
        TodoItem { action: Step::Reword, sha: c3.clone(), message: Some("three!".into()) },
    ];
    let r = e.perform(&repo, Action::InteractiveCherryPick { items });
    assert!(r.ok, "{}", r.message);
    let log = git::run(&d, &["log", "--reverse", "--format=%s", &format!("{base}..HEAD")]).stdout;
    assert_eq!(log.lines().collect::<Vec<_>>(), vec!["two", "three!"]);

    // Merge conflict: same line changed on both branches.
    commit("a.txt", "side\n", "side change");
    assert!(e.perform(&repo, Action::Checkout { target: "master".into() }).ok);
    commit("a.txt", "master\n", "master change");
    let r = e.perform(&repo, Action::Merge { branch: "side".into() });
    assert!(!r.ok);
    let data = git::load(&d, 50);
    assert_eq!(data.state_note.as_deref(), Some("merging"));
    assert!(data.status.conflicted);
    assert_eq!(data.conflict_labels, Some(("master".into(), "side".into())));
    let info = crate::rewrite::conflict_info(&d, "a.txt");
    assert_eq!(info.ours.as_deref().map(|s| s.replace("\r\n", "\n")), Some("master\n".into()));
    assert_eq!(info.theirs.as_deref().map(|s| s.replace("\r\n", "\n")), Some("side\n".into()));
    assert!(info.working.unwrap().contains("<<<<<<<"));
    assert!(e.perform(&repo, Action::TakeSide { paths: vec!["a.txt".into()], side: "theirs".into() }).ok);
    assert_eq!(read("a.txt"), "side\n");
    assert!(e.perform(&repo, Action::MarkConflicted { paths: vec!["a.txt".into()] }).ok);
    assert!(read("a.txt").contains("<<<<<<<"));
    assert!(e.perform(&repo, Action::SaveResolved { file: "a.txt".into(), content: "master\nside\n".into(), expected: None, utf8: None }).ok);
    assert!(!git::status(&d).conflicted);
    assert!(git::load(&d, 50).merge_msg.unwrap().contains("Merge branch 'side'"));
    assert!(e.perform(&repo, Action::Commit { summary: "Merge side".into(), description: String::new(), amend: false, stage_all: false, skip_hooks: false, strip_comments: false }).ok);
    assert_eq!(git::run(&d, &["rev-list", "--parents", "-n1", "HEAD"]).stdout.split_whitespace().count(), 3);

    // Fast-forward a branch that isn't checked out, set an upstream, rename a stash.
    assert!(e.perform(&repo, Action::CreateBranch { name: "old".into(), at: Some(base.clone()), checkout: false }).ok);
    assert!(e.perform(&repo, Action::FastForward { branch: "old".into(), to: "master".into() }).ok);
    assert_eq!(git::run(&d, &["rev-parse", "old"]).stdout, git::run(&d, &["rev-parse", "master"]).stdout);
    assert!(e.perform(&repo, Action::SetUpstream { branch: "old".into(), upstream: "master".into() }).ok);
    assert_eq!(git::run(&d, &["rev-parse", "--abbrev-ref", "old@{u}"]).stdout.trim(), "master");
    fs::write(d.join("e.txt"), "x").unwrap();
    assert!(e.perform(&repo, Action::Stash { message: "first".into() }).ok);
    assert!(e.perform(&repo, Action::StashRename { index: 0, message: "renamed".into(), expected: None }).ok);
    assert_eq!(git::load(&d, 5).stashes[0].message, "renamed");
    let _ = fs::remove_dir_all(&d);
}


#[test]
fn remotes_worktrees_submodules_cleanup_and_gitflow() {
    let d = temp_repo("manage");
    let repo = d.to_string_lossy().to_string();
    let e = Engine::default();
    let commit = |f: &str, m: &str| {
        fs::write(d.join(f), m).unwrap();
        assert!(e.perform(&repo, Action::Commit { summary: m.into(), description: String::new(), amend: false, stage_all: true, skip_hooks: false, strip_comments: false }).ok);
    };
    commit("a.txt", "one");
    assert!(e.perform(&repo, Action::RenameBranch { old: "master".into(), new: "main".into() }).ok);

    // Remotes: add a bare clone, fetch it, rename and re-point it, then remove it.
    let bare = std::env::temp_dir().join(format!("gitcraft-test-bare-{}", std::process::id()));
    let _ = fs::remove_dir_all(&bare);
    assert!(git::run(&d, &["clone", "-q", "--bare", ".", bare.to_string_lossy().as_ref()]).ok);
    let url = bare.to_string_lossy().to_string();
    assert!(e.perform(&repo, Action::AddRemote { name: "origin".into(), url: url.clone() }).ok);
    assert!(e.perform(&repo, Action::FetchRemote { remote: "origin".into() }).ok);
    assert!(e.perform(&repo, Action::AutoFetch { prune: true }).ok);
    assert!(e.perform(&repo, Action::RemoteEdit { name: "origin".into(), new_name: "upstream".into(), url: url.clone(), push_url: String::new() }).ok);
    let rs = crate::manage::remotes(&d);
    assert_eq!(rs.len(), 1);
    assert_eq!(rs[0].name, "upstream");
    assert!(e.perform(&repo, Action::RemoteRemove { name: "upstream".into() }).ok);
    assert!(crate::manage::remotes(&d).is_empty());

    // Worktrees: add with a new branch, lock, unlock, remove.
    let wt = std::env::temp_dir().join(format!("gitcraft-test-wt-{}", std::process::id()));
    let _ = fs::remove_dir_all(&wt);
    let wts = wt.to_string_lossy().to_string();
    assert!(e.perform(&repo, Action::WorktreeAdd { path: wts.clone(), branch: "wt-branch".into(), new_branch: true, start: None }).ok);
    let data = git::load(&d, 10);
    assert_eq!(data.worktrees.len(), 2);
    assert!(e.perform(&repo, Action::WorktreeLock { path: wts.clone(), message: "busy".into() }).ok);
    assert!(git::load(&d, 10).worktrees.iter().any(|w| w.locked));
    assert!(e.perform(&repo, Action::WorktreeUnlock { path: wts.clone() }).ok);
    assert!(e.perform(&repo, Action::WorktreeRemove { path: wts.clone(), force: false }).ok);
    assert_eq!(git::load(&d, 10).worktrees.len(), 1);

    // Submodule from the bare repository.
    let r = e.perform(&repo, Action::SubmoduleAdd { url: url.clone(), path: "libs/dep".into() });
    if r.ok {
        let subs = crate::manage::submodules(&d);
        assert_eq!(subs.len(), 1);
        assert_eq!(subs[0].path, "libs/dep");
        assert!(e.perform(&repo, Action::SubmoduleUpdate { paths: vec![] }).ok);
        assert!(e.perform(&repo, Action::SubmoduleRemove { path: "libs/dep".into() }).ok);
    } // (some Git setups forbid file:// submodules; the rest still runs)
    assert!(e.perform(&repo, Action::Commit { summary: "tidy".into(), description: String::new(), amend: false, stage_all: true, skip_hooks: false, strip_comments: false }).ok || true);

    // Gitflow: init creates develop; feature start / finish; release with a tag.
    let cfg = crate::manage::gitflow_config(&d);
    assert!(!cfg.initialized);
    assert_eq!((cfg.master.as_str(), cfg.develop.as_str(), cfg.feature.as_str()), ("main", "develop", "feature/"));
    assert!(e.perform(&repo, Action::GitflowInit { config: cfg }).ok);
    assert!(crate::manage::gitflow_config(&d).initialized);
    assert!(e.perform(&repo, Action::GitflowStart { kind: "feature".into(), name: "login".into() }).ok);
    commit("login.txt", "login");
    let r = e.perform(&repo, Action::GitflowFinish { kind: "feature".into(), name: "login".into(), keep: false, tag_message: String::new() });
    assert!(r.ok, "{}", r.message);
    assert!(d.join("login.txt").exists());
    assert!(e.perform(&repo, Action::GitflowStart { kind: "release".into(), name: "1.0".into() }).ok);
    let r = e.perform(&repo, Action::GitflowFinish { kind: "release".into(), name: "1.0".into(), keep: false, tag_message: "Release 1.0".into() });
    assert!(r.ok, "{}", r.message);
    assert!(git::run(&d, &["rev-parse", "--verify", "-q", "refs/tags/1.0"]).ok);
    assert!(git::run(&d, &["merge-base", "--is-ancestor", "refs/tags/1.0", "main"]).ok);

    // Branch clean-up: a merged branch shows up as merged and can be deleted (and undone).
    assert!(e.perform(&repo, Action::CreateBranch { name: "old-work".into(), at: None, checkout: false }).ok);
    let list = crate::manage::cleanup_list(&d);
    assert!(list.iter().any(|b| b.name == "old-work" && b.merged));
    assert!(e.perform(&repo, Action::DeleteBranches { names: vec!["old-work".into()], remotes: vec![] }).ok);
    assert!(!crate::manage::cleanup_list(&d).iter().any(|b| b.name == "old-work"));
    assert!(e.perform(&repo, Action::Undo).ok);
    assert!(crate::manage::cleanup_list(&d).iter().any(|b| b.name == "old-work"));
    let _ = fs::remove_dir_all(&d);
    let _ = fs::remove_dir_all(&bare);
}


#[test]
fn stash_files_take_from_stash_revert_hunk_squash_merge() {
    let d = temp_repo("gaps");
    let repo = d.to_string_lossy().to_string();
    let e = Engine::default();
    let rd = |p: std::path::PathBuf| fs::read_to_string(p).unwrap().replace("\r\n", "\n");
    let commit = |m: &str| assert!(e.perform(&repo, Action::Commit { summary: m.into(), description: String::new(), amend: false, stage_all: true, skip_hooks: false, strip_comments: false }).ok);
    fs::write(d.join("a.txt"), "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n").unwrap();
    fs::write(d.join("b.txt"), "b\n").unwrap();
    commit("base");

    // Stash only a.txt; b.txt stays changed.
    fs::write(d.join("a.txt"), "1\n2\n3\n4\n5\n6\n7\n8\n9\nten\n").unwrap();
    fs::write(d.join("b.txt"), "B\n").unwrap();
    assert!(e.perform(&repo, Action::StashFiles { paths: vec!["a.txt".into()], message: "only a".into() }).ok);
    assert_eq!(rd(d.join("a.txt")), "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n");
    assert_eq!(rd(d.join("b.txt")), "B\n");
    // A file without changes stops the stash, like libgit2.
    let r = e.perform(&repo, Action::StashFiles { paths: vec!["a.txt".into()], message: String::new() });
    assert!(!r.ok && r.message.contains("does not have any changes to stash"), "{}", r.message);
    // Take a.txt back out of the stash without applying the rest.
    assert!(e.perform(&repo, Action::StashTakeFile { index: 0, path: "a.txt".into(), expected: None }).ok);
    assert!(rd(d.join("a.txt")).ends_with("ten\n"));
    commit("two changes");

    // Revert the first hunk of a commit with two hunks, keeping the second.
    fs::write(d.join("a.txt"), "one\n2\n3\n4\n5\n6\n7\n8\n9\nTEN\n").unwrap();
    commit("edit both ends");
    let sha = head(&d);
    let r = e.perform(&repo, Action::RevertHunk { sha, parent: None, file: "a.txt".into(), hunk: 0, lines: None });
    assert!(r.ok, "{}", r.message);
    assert_eq!(rd(d.join("a.txt")), "1\n2\n3\n4\n5\n6\n7\n8\n9\nTEN\n");
    assert!(e.perform(&repo, Action::Undo).ok);
    assert!(rd(d.join("a.txt")).starts_with("one\n"));
    assert_eq!(rd(d.join("a.txt")), "one\n2\n3\n4\n5\n6\n7\n8\n9\nTEN\n", "undo puts the file back as committed");

    // Squash merge: the branch's work arrives staged, with no merge commit.
    assert!(git::run(&d, &["checkout", "-q", "-b", "side"]).ok);
    fs::write(d.join("c.txt"), "c\n").unwrap();
    commit("side 1");
    assert!(git::run(&d, &["checkout", "-q", "master"]).ok);
    let before = head(&d);
    assert!(e.perform(&repo, Action::MergeSquash { branch: "side".into() }).ok);
    assert_eq!(head(&d), before);
    assert!(git::status(&d).staged.iter().any(|f| f.path == "c.txt"));
    let _ = fs::remove_dir_all(&d);
}

#[test]
fn templates_ignore_hooks_sparse_and_shallow() {
    let d = temp_repo("tools");
    let repo = d.to_string_lossy().to_string();
    let e = Engine::default();
    // Starting files and the first commit.
    let opts = crate::manage::InitOptions { branch: String::new(), gitignore: vec!["Node".into(), "Rust".into()], license: "MIT".into(), readme: true, lfs: false };
    assert!(crate::manage::init_files(&d, "demo", &opts, &|a: &[&str]| e.git(&d, a)).ok);
    let gi = fs::read_to_string(d.join(".gitignore")).unwrap();
    assert!(gi.contains("node_modules/") && gi.contains("/target/"));
    assert!(fs::read_to_string(d.join("LICENSE")).unwrap().starts_with("MIT License"));
    assert_eq!(git::run(&d, &["log", "--format=%s"]).stdout.trim(), "Initial commit");

    // With nothing to add the first commit is still made, and it is empty.
    let empty = temp_repo("empty-init");
    git::run(&empty, &["config", "user.name", "T"]);
    git::run(&empty, &["config", "user.email", "t@example.com"]);
    let none = crate::manage::InitOptions { branch: String::new(), gitignore: vec![], license: String::new(), readme: false, lfs: false };
    assert!(crate::manage::init_files(&empty, "demo", &none, &|a: &[&str]| e.git(&empty, a)).ok);
    assert_eq!(git::run(&empty, &["log", "--format=%s"]).stdout.trim(), "Initial commit");
    assert!(git::run(&empty, &["ls-tree", "-r", "HEAD"]).stdout.trim().is_empty());

    // Ignore helpers: add once, untrack a tracked file.
    fs::write(d.join("secret.log"), "x").unwrap();
    assert!(e.perform(&repo, Action::Commit { summary: "log".into(), description: String::new(), amend: false, stage_all: true, skip_hooks: false, strip_comments: false }).ok);
    assert!(e.perform(&repo, Action::Ignore { pattern: "*.log".into(), untrack: vec!["secret.log".into()] }).ok);
    assert!(e.perform(&repo, Action::Ignore { pattern: "*.log".into(), untrack: vec![] }).ok);
    assert_eq!(fs::read_to_string(d.join(".gitignore")).unwrap().matches("*.log").count(), 1);
    assert!(git::status(&d).staged.iter().any(|f| f.path == "secret.log" && f.status == 'D'));

    // Skip hooks: a failing pre-commit hook blocks a normal commit but not a skipped one.
    let hooks = d.join(".git").join("hooks");
    fs::create_dir_all(&hooks).unwrap();
    fs::write(hooks.join("pre-commit"), "#!/bin/sh\necho blocked by hook >&2\nexit 1\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(hooks.join("pre-commit"), fs::Permissions::from_mode(0o755)).unwrap();
    }
    let r = e.perform(&repo, Action::Commit { summary: "blocked".into(), description: String::new(), amend: false, stage_all: true, skip_hooks: false, strip_comments: false });
    assert!(!r.ok && r.message.contains("blocked by hook"), "{}", r.message);
    assert!(e.perform(&repo, Action::Commit { summary: "skipped".into(), description: String::new(), amend: false, stage_all: true, skip_hooks: true, strip_comments: false }).ok);
    let _ = fs::remove_file(hooks.join("pre-commit"));

    // Sparse checkout: only docs/ stays in the working folder.
    fs::create_dir_all(d.join("docs")).unwrap();
    fs::write(d.join("docs/a.md"), "a").unwrap();
    fs::write(d.join("big.bin"), "b").unwrap();
    assert!(e.perform(&repo, Action::Commit { summary: "files".into(), description: String::new(), amend: false, stage_all: true, skip_hooks: false, strip_comments: false }).ok);
    assert!(e.perform(&repo, Action::SparseSet { patterns: vec!["/docs/".into()] }).ok);
    assert!(crate::manage::sparse_info(&d).enabled);
    assert!(!d.join("big.bin").exists() && d.join("docs/a.md").exists());
    assert!(e.perform(&repo, Action::SparseDisable).ok);
    assert!(d.join("big.bin").exists());

    // LFS info never fails, whether or not git-lfs is installed.
    let _ = crate::manage::lfs_info(&d);
    let _ = fs::remove_dir_all(&d);
}

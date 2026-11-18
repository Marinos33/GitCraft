//! Long Git jobs (clone, fetch, pull, push) with live progress and Cancel.
//! git writes progress to stderr with carriage returns ("Receiving objects:  45% (450/1000)");
//! each line is passed to a listener (the window shows it in the status bar).

use crate::git::{self, GitOut};
use std::collections::HashMap;
use std::io::Read;
use std::path::Path;
use std::process::Stdio;
use std::sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}, mpsc};
use std::time::{Duration, Instant};

pub type Listener = Arc<dyn Fn(&str, &str, Option<u32>) + Send + Sync>;

#[derive(Default)]
pub struct Jobs {
    running: Mutex<HashMap<String, Arc<Running>>>,
    listener: Mutex<Option<Listener>>,
}

struct Running {
    tree: crate::process_tree::ProcessTree,
    cancelled: AtomicBool,
    pending_progress: AtomicBool,
}

/// The percentage at the end of a git progress line, if any ("...  45% (450/1000)").
pub fn percent(line: &str) -> Option<u32> {
    let i = line.rfind('%')?;
    let digits: String = line[..i].chars().rev().take_while(|c| c.is_ascii_digit()).collect::<Vec<_>>().into_iter().rev().collect();
    digits.parse().ok().filter(|p: &u32| *p <= 100)
}

/// Git uses CR for progress updates, CRLF on some platforms, and may omit the last newline.
#[derive(Default)]
struct ProgressOutput {
    pending: Vec<u8>,
    progress: Option<String>,
    stderr: String,
    truncated: bool,
    line_truncated: bool,
}

const OUTPUT_TAIL: usize = 64 * 1024;
const PROGRESS_LINE: usize = 16 * 1024;
fn append_tail(text: &mut String, value: &str) -> bool {
    text.push_str(value);
    if text.len() <= OUTPUT_TAIL { return false; }
    let mut cut = text.len() - OUTPUT_TAIL;
    while !text.is_char_boundary(cut) { cut += 1; }
    text.drain(..cut);
    true
}
impl ProgressOutput {
    fn retain(&mut self, text: &str) {
        self.truncated |= append_tail(&mut self.stderr, text);
        self.truncated |= append_tail(&mut self.stderr, "\n");
    }
    fn push(&mut self, bytes: &[u8]) -> Vec<String> {
        let mut updates = Vec::new();
        for &b in bytes {
            if b == b'\r' || b == b'\n' {
                if !self.pending.is_empty() {
                    let mut text = String::from_utf8_lossy(&self.pending).trim().to_string();
                    if self.line_truncated { text.push_str(" [line truncated]"); self.truncated = true; self.line_truncated = false; }
                    self.pending.clear();
                    updates.push(text.clone());
                    if b == b'\r' {
                        self.progress = Some(text);
                    } else {
                        self.retain(&text);
                        self.progress = None;
                    }
                } else if b == b'\n' {
                    if let Some(text) = self.progress.take() {
                        self.retain(&text);
                    }
                }
            } else {
                if self.pending.len() < PROGRESS_LINE { self.pending.push(b); } else { self.line_truncated = true; }
            }
        }
        updates
    }

    fn finish(mut self) -> String {
        if !self.pending.is_empty() {
            let mut text = String::from_utf8_lossy(&self.pending).into_owned();
            if self.line_truncated { text.push_str(" [line truncated]"); self.truncated = true; }
            self.retain(&text);
        } else if let Some(text) = self.progress.take() { self.retain(&text); }
        if self.truncated { self.stderr.insert_str(0, "[output truncated; diagnostic tail follows]\n"); }
        self.stderr
    }

}

impl Jobs {
    pub fn set_listener(&self, l: Listener) {
        *self.listener.lock().unwrap() = Some(l);
    }

    /// Runs git, reporting each progress line as (job id, text, percent). Can be cancelled by id.
    pub fn run(&self, id: &str, dir: &Path, args: &[&str]) -> GitOut {
        let mut cmd = git::git_cmd(dir);
        git::add_credentials(&mut cmd, args);
        crate::process_tree::prepare(&mut cmd);
        cmd.args(args).stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => return GitOut { ok: false, stdout: String::new(), stderr: format!("Could not start git: {e}") },
        };
        let tree = match crate::process_tree::ProcessTree::attach(&child) {
            Ok(tree) => tree,
            Err(error) => { let _ = child.kill(); let _ = child.wait(); return GitOut::failed(&format!("Could not create a cancellable Git job: {error}")); }
        };
        let running = Arc::new(Running { tree, cancelled: AtomicBool::new(false), pending_progress: AtomicBool::new(false) });
        self.running.lock().unwrap().insert(id.to_string(), running.clone());
        let mut err_pipe = child.stderr.take().expect("stderr");
        let mut out_pipe = child.stdout.take().expect("stdout");
        let (out_tx, out_rx) = mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let mut tail = Vec::new(); let mut bytes = [0; 4096]; let mut truncated = false;
            loop { match out_pipe.read(&mut bytes) {
                Ok(0) | Err(_) => break,
                Ok(n) => { tail.extend_from_slice(&bytes[..n]); if tail.len() > OUTPUT_TAIL { tail.drain(..tail.len() - OUTPUT_TAIL); truncated = true; } }
            } }
            let mut text = String::from_utf8_lossy(&tail).into_owned();
            if truncated { text.insert_str(0, "[output truncated; diagnostic tail follows]\n"); }
            let _ = out_tx.send(text);
        });
        let (err_tx, err_rx) = mpsc::sync_channel(16);
        std::thread::spawn(move || {
            let mut bytes = [0; 1024];
            loop { match err_pipe.read(&mut bytes) { Ok(0) | Err(_) => break, Ok(n) => if err_tx.send(bytes[..n].to_vec()).is_err() { break; } } }
        });
        let listener = self.listener.lock().unwrap().clone();
        let mut output = ProgressOutput::default();
        let mut cancelled_at = None;
        loop {
            if running.cancelled.load(Ordering::Acquire) {
                let at = cancelled_at.get_or_insert_with(Instant::now);
                if at.elapsed() > Duration::from_secs(2) { break; }
            }
            match err_rx.recv_timeout(Duration::from_millis(50)) {
                Ok(bytes) => for text in output.push(&bytes) { if let Some(listener) = &listener { if !running.pending_progress.swap(true, Ordering::AcqRel) { listener(id, &git::redact_diagnostic(&text), percent(&text)); } } },
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Err(mpsc::RecvTimeoutError::Timeout) => {},
            }
        }
        drop(err_rx);
        let mut stderr = output.finish();
        let status = loop {
            if running.cancelled.load(Ordering::Acquire) { cancelled_at.get_or_insert_with(Instant::now); }
            match child.try_wait() {
                Ok(Some(status)) => break Some(status),
                Ok(None) if cancelled_at.is_none() => std::thread::sleep(Duration::from_millis(20)),
                Ok(None) if cancelled_at.unwrap().elapsed() < Duration::from_secs(2) => std::thread::sleep(Duration::from_millis(20)),
                _ => { let _ = child.kill(); break None; },
            }
        };
        self.running.lock().unwrap().remove(id);
        let stdout = out_rx.recv_timeout(Duration::from_secs(2)).unwrap_or_default();
        let cancelled = running.cancelled.load(Ordering::Acquire);
        if cancelled { stderr = "Cancelled".into(); }
        let ok = !cancelled && status.is_some_and(|status| status.success());
        if let Some(l) = &listener {
            l(id, "", None); // finished
        }
        GitOut { ok, stdout, stderr }
    }

    /// At most one progress event per running job can await renderer delivery.
    pub fn acknowledge(&self, id: &str) {
        if let Some(job) = self.running.lock().unwrap().get(id) { job.pending_progress.store(false, Ordering::Release); }
    }

    /// Stop a running job. Returns false if it already finished.
    pub fn cancel(&self, id: &str) -> bool {
        let job = self.running.lock().unwrap().get(id).cloned();
        match job {
            Some(job) => { job.cancelled.store(true, Ordering::Release); job.tree.kill() },
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slow_renderer_does_not_accumulate_progress_events() {
        let jobs = Jobs::default(); let seen = Arc::new(Mutex::new(Vec::<String>::new())); let capture = seen.clone();
        jobs.set_listener(Arc::new(move |_, text, _| capture.lock().unwrap().push(text.into())));
        let result = jobs.run("slow-renderer", &std::env::temp_dir(), &["-c", "alias.output=!i=0; while [ $i -lt 5000 ]; do echo diagnostic-progress >&2; i=$((i+1)); done; echo fatal-final-diagnostic >&2; exit 1", "output"]);
        assert!(!result.ok); assert!(result.stderr.contains("fatal-final-diagnostic")); assert!(result.stderr.contains("truncated"));
        assert!(seen.lock().unwrap().len() <= 2, "Unacknowledged progress queued in renderer");
        assert!(seen.lock().unwrap().last().unwrap().is_empty());
    }

    #[test]
    fn output_budgets_bound_lines_and_keep_final_errors() {
        let mut output = ProgressOutput::default();
        for _ in 0..16_384 { output.push(&[b'x'; 1024]); }
        assert!(output.pending.len() <= PROGRESS_LINE);
        output.push(b"\nfatal: final diagnostic\n");
        let result = output.finish(); assert!(result.contains("truncated")); assert!(result.ends_with("fatal: final diagnostic\n"));
        let mut output = ProgressOutput::default();
        for _ in 0..16_384 { output.push(b"diagnostic line repeated many times\n"); }
        assert!(output.stderr.len() <= OUTPUT_TAIL);
        let result = output.finish(); assert!(result.len() < OUTPUT_TAIL + 100); assert!(result.contains("truncated"));
    }

    #[test]
    fn reads_git_progress_percentages() {
        assert_eq!(percent("Receiving objects:  45% (450/1000), 1.2 MiB | 3 MiB/s"), Some(45));
        assert_eq!(percent("Resolving deltas: 100% (20/20), done."), Some(100));
        assert_eq!(percent("remote: Enumerating objects: 5, done."), None);
    }

    #[test]
    fn preserves_crlf_and_unterminated_errors() {
        let mut output = ProgressOutput::default();
        output.push(b"remote: message\r");
        output.push(b"\nfatal: authentication failed");
        assert_eq!(output.finish(), "remote: message\nfatal: authentication failed\n");
    }

    #[test]
    fn keeps_only_the_final_progress_update() {
        let mut output = ProgressOutput::default();
        assert_eq!(output.push(b"Receiving: 10%\rReceiving: 20%\r"), vec!["Receiving: 10%", "Receiving: 20%"]);
        output.push(b"\n");
        assert_eq!(output.finish(), "Receiving: 20%\n");
        let mut output = ProgressOutput::default();
        output.push(b"Resolving: 100%\r");
        assert_eq!(output.finish(), "Resolving: 100%\n");
    }

    #[test]
    fn runs_and_reports() {
        let jobs = Jobs::default();
        let seen = Arc::new(Mutex::new(Vec::<String>::new()));
        let s2 = seen.clone();
        jobs.set_listener(Arc::new(move |_id, t, _p| s2.lock().unwrap().push(t.to_string())));
        let o = jobs.run("v", &std::env::temp_dir(), &["--version"]);
        assert!(o.ok && o.stdout.starts_with("git version"));
        assert!(!jobs.cancel("v"));
        assert!(seen.lock().unwrap().last().map(|s| s.is_empty()).unwrap_or(false));
    }
}

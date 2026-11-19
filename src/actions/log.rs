//! The activity log: every Git command the engine runs, with its output.

use super::*;

/// Activity log text: Git's message, or with extended logging both outputs in full.
pub(super) fn log_output(o: &git::GitOut) -> String {
    if crate::settings::current().extended_log {
        let mut t = String::new();
        if !o.stdout.trim().is_empty() {
            t.push_str(o.stdout.trim_end());
        }
        if !o.stderr.trim().is_empty() {
            if !t.is_empty() {
                t.push('\n');
            }
            t.push_str(o.stderr.trim_end());
        }
        t
    } else {
        o.message()
    }
}

impl Engine {
    pub fn log_entries(&self) -> Vec<LogEntry> {
        self.log.lock().unwrap().clone()
    }

    pub fn log_snapshot(&self, version: Option<u64>) -> Option<(u64, Vec<LogEntry>)> {
        let log = self.log.lock().unwrap();
        let current = self.log_version.load(std::sync::atomic::Ordering::Relaxed);
        if version == Some(current) { None } else { Some((current, log.clone())) }
    }

    /// Runs git and records it in the activity log.
    pub fn git(&self, dir: &Path, args: &[&str]) -> git::GitOut {
        // Named-file actions pass exact identities after --, never wildcard selections.
        let literal;
        let args = if args.contains(&"--") {
            literal = std::iter::once("--literal-pathspecs").chain(args.iter().copied()).collect::<Vec<_>>();
            literal.as_slice()
        } else { args };
        let o = git::run(dir, args);
        let entry = LogEntry {
            time: chrono::Local::now().format("%H:%M:%S").to_string(),
            command: format!("git {}", args.join(" ")),
            ok: o.ok,
            output: log_output(&o),
        };
        self.append_log(entry);
        o
    }

    /// Run a long git command as a cancellable job ("<repo>|<kind>"), logged like any other.
    pub fn job(&self, repo: &str, kind: &str, dir: &Path, args: &[&str]) -> git::GitOut {
        let o = self.jobs.run(&format!("{repo}|{kind}"), dir, args);
        let o = if !o.ok && o.stderr.trim().is_empty() { git::GitOut { ok: false, stdout: o.stdout, stderr: "Cancelled".into() } } else { o };
        self.record(&format!("git {}", args.join(" ")), &o);
        o
    }

    pub(super) fn append_log(&self, mut entry: LogEntry) {
        entry.command=git::redact_diagnostic(&entry.command);entry.output=git::redact_diagnostic(&entry.output);
        for (text, limit) in [(&mut entry.command,8192),(&mut entry.output,65536)] {
            if text.len()>limit {
                let mut end=limit;while !text.is_char_boundary(end) {end-=1;}
                text.truncate(end);text.push_str("\n[truncated]");
            }
        }
        let mut log=self.log.lock().unwrap();log.push(entry);
        let excess=log.len().saturating_sub(500);if excess>0 {log.drain(..excess);}
        self.log_version.fetch_add(1,std::sync::atomic::Ordering::Relaxed);
    }

    pub(super) fn record(&self, command: &str, o: &git::GitOut) {
        self.append_log(LogEntry { time: chrono::Local::now().format("%H:%M:%S").to_string(), command: command.to_string(), ok: o.ok, output: log_output(o) });
    }

    pub(super) fn log_note(&self, command: &str) {
        self.append_log(LogEntry { time: chrono::Local::now().format("%H:%M:%S").to_string(), command: command.to_string(), ok: true, output: String::new() });
    }
}

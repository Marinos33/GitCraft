import { useStore } from "./store";
import { invoke } from "@tauri-apps/api/core";
import { flushPersistence } from "./persistence";

// IPC types are generated from the Rust definitions (ts-rs, `cargo test` writes ui/src/bindings).
import type { RefKind } from "../bindings/RefKind";
import type { Commit } from "../bindings/Commit";
import type { GitRef } from "../bindings/GitRef";
import type { Stash } from "../bindings/Stash";
import type { Seg } from "../bindings/Seg";
import type { GraphRow } from "../bindings/GraphRow";
import type { HistoryCursor } from "../bindings/HistoryCursor";
import type { HistoryPage } from "../bindings/HistoryPage";
import type { DiffSource } from "../bindings/DiffSource";
import type { FileVersions } from "../bindings/FileVersions";
import type { HistoryEntry } from "../bindings/HistoryEntry";
import type { BlameLine } from "../bindings/BlameLine";
import type { Stats } from "../bindings/Stats";
import type { RefView } from "../bindings/RefView";
import type { PatchOp } from "../bindings/PatchOp";
import type { Step } from "../bindings/Step";
import type { TodoItem } from "../bindings/TodoItem";
import type { AccountInfo } from "../bindings/AccountInfo";
import type { LfsInfo } from "../bindings/LfsInfo";
import type { CloneOptions } from "../bindings/CloneOptions";
import type { InitOptions } from "../bindings/InitOptions";
import type { GitflowConfig } from "../bindings/GitflowConfig";
import type { CleanupBranch } from "../bindings/CleanupBranch";
import type { Remote as RemoteInfo } from "../bindings/Remote";
import type { Submodule as SubmoduleInfo } from "../bindings/Submodule";
import type { ConflictInfo } from "../bindings/ConflictInfo";
import type { PlanState as RebaseState } from "../bindings/PlanState";
import type { DiffLine } from "../bindings/DiffLine";
import type { LogEntry } from "../bindings/LogEntry";
import type { PushReview } from "../bindings/PushReview";
import type { Action } from "../bindings/Action";
import type { Metrics } from "../bindings/Metrics";
import type { ActionResult } from "../bindings/ActionResult";
import type { FileChange as GeneratedFileChange } from "../bindings/FileChange";
import type { RepoView as GeneratedRepoView } from "../bindings/RepoView";

export type { RefKind, Commit, GitRef, Stash, Seg, GraphRow, HistoryCursor, HistoryPage, DiffSource, FileVersions, HistoryEntry, BlameLine, Stats, RefView, PatchOp, Step, TodoItem, AccountInfo, LfsInfo, CloneOptions, InitOptions, GitflowConfig, CleanupBranch, RemoteInfo, SubmoduleInfo, ConflictInfo, RebaseState, DiffLine, LogEntry, PushReview, Action, Metrics, ActionResult };

/** A changed file; files of a stash's untracked-files commit also name where they are stored. */
export type FileChange = GeneratedFileChange & { storage?: string | null };

/** The repository as loaded, plus where the window restores the history scroll position. */
export type RepoView = GeneratedRepoView & { history_restore?: number; history_restore_selection?: string | null };

export const WORKDIR = "WORKDIR";

/** "job-progress" events (built in the setup hook). */
export interface JobProgress {
  id: string;
  text: string;
  percent: number | null;
}

let readSequence = 0;
export function invokeRead<T>(command: string, args: Record<string, unknown>, signal?: AbortSignal): Promise<T> {
  if (signal?.aborted) return Promise.reject(new DOMException("Read cancelled", "AbortError"));
  const readId = `read-${Date.now()}-${++readSequence}`;
  const cancel = () => { void invoke("cancel_read", { id: readId }).catch(() => undefined); };
  signal?.addEventListener("abort", cancel, { once: true });
  return invoke<T>(command, { ...args, readId }).finally(() => signal?.removeEventListener("abort", cancel));
}

export const api = {
  launchOptions: () => invoke<{ screenshot: boolean; shot_view: string | null; version: string; metrics: Metrics }>("launch_options"),
  repoStatus: (path: string, signal?: AbortSignal) => invokeRead<RepoView["status"]>("repo_status", { path }, signal),
  openRepo: (path: string, max?: number, view?: RefView, signal?: AbortSignal) => invokeRead<RepoView>("open_repo", { path, max: max ?? null, view: view ?? null }, signal),
  historyPage: (path: string, cursor: HistoryCursor, max = 2000, signal?: AbortSignal) => invokeRead<HistoryPage>("history_page", { path, cursor, max }, signal),
  commitDetails: (path: string, sha: string, signal?: AbortSignal) => invokeRead<Commit>("commit_details", { path, sha }, signal),
  commitFiles: (path: string, sha: string, signal?: AbortSignal) => invokeRead<FileChange[]>("commit_files", { path, sha }, signal),
  fileVersions: (path: string, source: DiffSource, file: string, untracked: boolean, parent: string | null, allowLarge = false, signal?: AbortSignal) =>
    invokeRead<FileVersions>("file_versions", { path, source, file, untracked, parent, allowLarge }, signal),
  saveFile: (path: string, file: string, content: string, expected?: string, utf8 = false) => invoke<void>("save_file", { path, file, content, expected, utf8 }),
  fileHistory: (path: string, file: string, signal?: AbortSignal) => invokeRead<HistoryEntry[]>("file_history", { path, file }, signal),
  blame: (path: string, file: string, rev: string | null, signal?: AbortSignal) => invokeRead<BlameLine[]>("blame", { path, file, rev }, signal),
  commitStats: (path: string, shas: string[], signal?: AbortSignal) => invokeRead<Stats[]>("commit_stats", { path, shas }, signal),
  rangeFiles: (path: string, from: string, to: string, signal?: AbortSignal) => invokeRead<FileChange[]>("range_files", { path, from, to }, signal),
  trackedFiles: (path: string, signal?: AbortSignal) => invokeRead<string[]>("tracked_files", { path }, signal),
  /** Preferences > External Tools > Editor. */
  openInEditor: async (path: string, file?: string) => {
    // The editor comes from the saved settings, so save a choice made a moment ago first.
    await flushPersistence();
    return invoke<void>("open_in_editor", { path, file: file ?? null });
  },
  accountInfo: (path: string | null, signal?: AbortSignal) => invokeRead<AccountInfo>("account_info", { path }, signal),
  gitConfig: (path: string | null, scope: "global" | "local", keys: string[], signal?: AbortSignal) => invokeRead<Record<string, string>>("git_config_get", { path, scope, keys }, signal),
  setGitConfig: (path: string | null, scope: "global" | "local", key: string, value: string) => invoke<void>("set_git_config", { path, scope, key, value }),
  generateSshKey: (email: string, name: string) => invoke<string>("generate_ssh_key", { email, name }),
  acknowledgeJob: (id: string) => invoke<void>("acknowledge_job", { id }),
  cancelJob: (id: string) => invoke<boolean>("cancel_job", { id }),
  lfsLocks: (path: string, signal?: AbortSignal) => invokeRead<LfsInfo["locks"]>("lfs_locks", { path }, signal),
  lfsInfo: (path: string, signal?: AbortSignal) => invokeRead<LfsInfo>("lfs_info", { path }, signal),
  sparseInfo: (path: string, signal?: AbortSignal) => invokeRead<{ enabled: boolean; patterns: string[] }>("sparse_info", { path }, signal),
  gitignoreTemplates: () => invoke<string[]>("gitignore_templates"),
  gitflowConfig: (path: string, signal?: AbortSignal) => invokeRead<GitflowConfig>("gitflow_config", { path }, signal),
  cleanupList: (path: string, signal?: AbortSignal) => invokeRead<CleanupBranch[]>("cleanup_list", { path }, signal),
  remotesList: (path: string, signal?: AbortSignal) => invokeRead<RemoteInfo[]>("remotes_list", { path }, signal),
  submodulesList: (path: string, signal?: AbortSignal) => invokeRead<SubmoduleInfo[]>("submodules_list", { path }, signal),
  rebasePlan: (path: string, base: string, signal?: AbortSignal) => invokeRead<{ expected: RebaseState; commits: Commit[] }>("rebase_plan", { path, base }, signal),
  rebaseCommits: (path: string, base: string, signal?: AbortSignal) => invokeRead<Commit[]>("rebase_commits", { path, base }, signal),
  conflictInfo: (path: string, file: string, signal?: AbortSignal) => invokeRead<ConflictInfo>("conflict_info", { path, file }, signal),
  fileDiff: (path: string, source: DiffSource, file: string, untracked: boolean, parent: string | null, ignoreWs = false, signal?: AbortSignal) =>
    invokeRead<DiffLine[]>("file_diff", { path, source, file, untracked, parent, ignoreWs }, signal),
  run: (path: string, action: Action) => invoke<ActionResult>("run_action", { path, action }),
  activityLogSnapshot: (version?: number) => invoke<[number, LogEntry[]] | null>("activity_log_snapshot", { version }),
  activityLog: () => invoke<LogEntry[]>("activity_log"),
  sampleRepo: () => invoke<string>("sample_repo"),
  conflictSample: () => invoke<string>("conflict_sample"),
  lastCommitMessage: (path: string, signal?: AbortSignal) => invokeRead<[string, string]>("last_commit_message", { path }, signal),
  pickFolder: (title: string) => invoke<string | null>("pick_folder", { title }),
  initRepo: (parent: string, name: string, options?: InitOptions) => invoke<{path: string; warning: string | null}>("init_repo", { parent, name, options: options ?? null }),
  cloneRepo: (url: string, parent: string, options?: CloneOptions) => invoke<string>("clone_repo", { url, parent, options: options ?? null }),
  /** Preferences > External Tools > Terminal; General > Use In-App Terminal Instead opens the built-in one. */
  openTerminal: (path: string) => {
    const t = useStore.getState().settings;
    if (t.useInAppTerminal) {
      useStore.getState().setSettings({ showTerminal: true });
      return Promise.resolve();
    }
    return flushPersistence().then(() => invoke<void>("open_terminal", { path }));
  },
  openFileManager: (path: string) => invoke<void>("open_file_manager", { path }),
  /** Open a repository file with the system's usual app for it. */
  openDefault: (path: string, file: string) => invoke<void>("open_default", { path, file }),
  /** Show a repository file selected in the file manager. */
  revealFile: (path: string, file: string) => invoke<void>("reveal_file", { path, file }),
  /** A file's text from the working folder, or as it was in `rev`. */
  fileText: (path: string, file: string, rev?: string, signal?: AbortSignal) => invokeRead<string>("file_text", { path, file, rev: rev ?? null }, signal),
  loadSettings: () => invoke<unknown>("load_settings"),
  /** Returns the settings GitCraft restored because a new command was not approved. */
  saveSettings: (settings: unknown) => invoke<Record<string, unknown> | null | undefined>("save_settings", { settings }),
  watchRepo: (path: string) => invoke<void>("watch_repo", { path }),
  unwatchRepo: (path: string) => invoke<void>("unwatch_repo", { path }),
  screenshotReady: () => invoke<void>("screenshot_ready"),
};

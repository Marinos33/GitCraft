// Shapes of the window state: tabs, dialogs, notifications and the saved settings.
import type { DiffSource } from "../api";

export type TabKind = "repo" | "start" | "launchpad";

export interface Tab {
  id: string;
  kind: TabKind;
  path?: string;
  title: string;
}

export type Selection = { type: "wip" } | { type: "commit"; sha: string };

export interface DiffTarget {
  source: DiffSource;
  file: string;
  untracked: boolean;
  parent: string | null;
}

export interface TabUi {
  selection: Selection;
  diff: DiffTarget | null;
  search: string;
  searchOpen: boolean;
  /** Commits picked with Ctrl / Shift + click (2 = compare them). */
  multi: string[];
  /** Comparing a commit with the working folder. */
  compareWorkdir: string | null;
  /** File History / Blame view in the centre. */
  history: { file: string; mode: "history" | "blame"; sha: string | null } | null;
  /** How many commits the graph has loaded (grows when scrolling near the end). */
  limit: number;
  /** Interactive rebase / cherry pick being planned in the centre. */
  rebase: RebasePlan | null;
  /** File open in the merge conflict tool. */
  conflict: string | null;
  /** Pull request / issue open in the centre. */
  pr: number | null;
  issue: string | number | null;
}

export interface RebaseItem {
  action: import("../api").Step;
  sha: string;
  author: string;
  email: string;
  summary: string;
  /** Full message (summary + body); edited for reword. */
  message: string;
}

export interface RebasePlan {
  expected?: import("../api").RebaseState;
  mode: "rebase" | "cherry";
  /** Commit (or "--root") the rebase replays onto; unused for cherry pick. */
  base: string;
  onto: string;
  /** Newest first, as shown. */
  items: RebaseItem[];
  original: RebaseItem[];
}

export interface Draft {
  summary: string;
  description: string;
  amend: boolean;
}

/** One entry of the bell's menu. */
export interface Notice {
  id: number;
  text: string;
  at: number;
  readAt: number | null;
}
export type NoticeFilter = "All" | "Read" | "Unread";


export interface Toast {
  id: number;
  ok: boolean;
  text: string;
}

export type DialogState =
  | null
  | { kind: "prompt"; title: string; label: string; value: string; confirm: string; onSubmit: (v: string, extra?: string) => void; extraLabel?: string; allowEmpty?: boolean }
  | { kind: "confirm"; title: string; text: string; confirm: string; danger?: boolean; onConfirm: () => void }
  | { kind: "clone" }
  | { kind: "addRemote"; path: string }
  | { kind: "init" }
  | { kind: "open" }
  | { kind: "log" }
  | { kind: "shortcuts" }
  | { kind: "about" }
  | { kind: "plan" }
  | { kind: "prefs"; section?: string }
  | { kind: "fileFinder" }
  | {
      kind: "form";
      title: string;
      fields: FormField[];
      confirm: string;
      help?: string;
      onSubmit: (v: Record<string, string | boolean>) => void;
      /** An extra button that fills in the fields (e.g. "Write with AI"). */
      action?: { label: string; run: (v: Record<string, string | boolean>) => Promise<Record<string, string | boolean>> };
    }
  | { kind: "cleanup" }
  | { kind: "sparse" }
  | { kind: "accounts" }
  | { kind: "profiles" }
  | { kind: "addProfile" }
  | { kind: "workspaces" }
  | { kind: "choice"; title: string; text: string; choices: { label: string; variant?: "primary" | "danger" | "default"; onSelect: () => void }[] }
  | { kind: "info"; title: string; text: string }
  | { kind: "repoManagement" }
  | { kind: "message"; title: string; label: string; value: string; confirm: string; help?: string; generate?: () => Promise<string>; onSubmit: (v: string) => void };

export interface Settings {
  showLeft: boolean;
  showRight: boolean;
  leftWidth: number;
  rightWidth: number;
  pullMode: "ff" | "ffonly" | "rebase" | "fetch";
  zoom: number;
  recents: string[];
  pushAfterCommit: boolean;
  fileView: "path" | "tree";
  fileSort: "path" | "status";
  leftView: "list" | "agents";
  toolbarLabels: boolean;
  defaultCloneFolder: string;
  diffMode: "hunk" | "inline" | "split";
  ignoreWhitespace: boolean;
  wordWrap: boolean;
  columns: GraphColumns;
  graphColumnWidths: Partial<Record<GraphColumn, number>>;
  /** Hidden and soloed branches / tags per repository (full ref names). */
  hidden: Record<string, string[]>;
  solo: Record<string, string[]>;
  lazyLoad: boolean;
  /** Minutes between background fetches (0 = off); the default is 1. */
  autoFetchMin: number;
  autoPrune: boolean;
  /** "Default Branch Name" for new repositories. */
  defaultBranch: string;
  /** "Skip git hooks by default when committing". */
  skipHooks: boolean;
  /** Custom keyboard shortcuts: command id -> keys ("" = no shortcut). */
  keybindings: Record<string, string>;
  showTerminal: boolean;
  terminalHeight: number;
  theme: string;
  workspaces: Workspace[];
  /** Tab / repository aliases (path -> name). */
  aliases: Record<string, string>;
  /** User-sized repository tabs, in CSS pixels, keyed by repository path. */
  tabWidths: Record<string, number>;
  spellcheck: boolean;
  /** Check GitHub Releases for new versions and download them in the background. */
  autoUpdate: boolean;
  /** Update channel: full releases, or every build of the main branch. */
  updateChannel: "stable" | "beta";
  /** Starred repositories (Repository Management, start screen, Open Favorite Repo 1-9). */
  favorites: string[];
  /** Commit template: prefilled summary / description. */
  templateSummary: string;
  templateDescription: string;
  /** External diff / merge tools (git difftool / mergetool names). */
  diffTool: string;
  mergeTool: string;
  /** Show a notice after every successful action (errors always show). */
  notifySuccess: boolean;
  /** "Pin to Left": per repository, the full ref name drawn in lane 0. */
  pinned: Record<string, string>;
  /** "Show ghost branch/tag when hovering over or selecting a commit". */
  ghostRefs: boolean;
  /** Self-managed GitLab host (empty = gitlab.com only). */
  gitlabHost: string;
  // ---- Preferences screen ----
  /** General > Keep submodules up to date (autoUpdateSubmodules: true). */
  autoUpdateSubmodules: boolean;
  /** General > Delete ".orig" files after merging (deleteOrigAfterMerge: false). */
  deleteOrigAfterMerge: boolean;
  /** General > Remember tabs (rememberTabs: true). */
  rememberTabs: boolean;
  /** General > Initial Commits in Graph (maxCommitsInGraph 2000, minimum 500) / Show All Commits. */
  initialCommits: number;
  showAllCommits: boolean;
  /** General > Use extended logging in activity log. */
  extendedLog: boolean;
  /** General > Enable proactive conflict detection with target branch (conflictDetection.enabled: true). */
  conflictDetection: boolean;
  /** General > Use In-App Terminal Instead (of an external terminal). */
  useInAppTerminal: boolean;
  /** Commit > Merge Behavior: squash local merges. */
  squashMerge: boolean;
  /** SSH: use the system SSH agent / config, or this key pair. */
  sshUseAgent: boolean;
  sshPrivateKey: string;
  sshPublicKey: string;
  /** UI Customization. */
  notificationLocation: "top-left" | "top-right" | "bottom-left" | "bottom-right";
  dateFormat: string;
  dateTimeFormat: string;
  dateVerboseFormat: string;
  authorInitials: boolean;
  highlightRowsOnHover: boolean;
  /** Coloured ticks beside the graph scrollbar (off by default). */
  scrollMarkers: boolean;
  smartBranches: boolean;
  autogroupWorktreeTabs: boolean;
  showAgentsView: boolean;
  /** Notifications > Enable Desktop Notifications (showDesktopNotifications: false). */
  desktopNotifications: boolean;
  /** Profiles: switching a profile also writes its author name and email to the global Git config. */
  keepGitconfigUpdated: boolean;
  /** Editor (editor: fontSize 12, tabSize 4, showLineNumbers, syntaxHighlighting, CRLF on Windows). */
  editorFontFamily: string;
  editorFontSize: number;
  editorLineNumbers: boolean;
  editorSyntax: boolean;
  editorTabSize: number;
  editorEol: "lf" | "crlf";
  monospaceOnly: boolean;
  /** In-App Terminal (cli: fontSize 12, lineHeight 1.2, cursor bar, dim when unfocused, PowerShell). */
  termShell: string;
  termFontFamily: string;
  termFontSize: number;
  termLineHeight: number;
  termCursor: "block" | "underline" | "bar";
  termDim: boolean;
  /** External Tools: editor, terminal, coding agent. */
  externalEditor: string;
  editorPath: string;
  editorFileArgs: string;
  editorRepoArgs: string;
  externalTerminal: string;
  terminalPath: string;
  terminalArgs: string;
  codingAgent: string;
  agentArgs: string;
  /** Agents: setup commands for a new agent worktree, and WIP rows for worktrees. */
  agentSetup: string;
  showWorktreeWip: boolean;
  /** Commit: apply the template; strip # comments (templates.useCommitTemplate true, shouldRemoveComments false). */
  useCommitTemplate: boolean;
  stripComments: boolean;
  /** Experimental > Git Executable (empty = git on PATH). */
  gitPath: string;
  ai: AiSettings;
  integrations: IntegrationSettings;
  /** Repo-specific preferences, per repository path. */
  repoPrefs: Record<string, RepoPrefs>;
}

export type AiProvider = "anthropic" | "openai" | "codex" | "claude_code" | "google" | "ollama" | "custom";

export interface AiConnection {
  endpoint: string;
  cliPath: string;
  cliConfigDir: string;
  defaultModel: string;
  responseFormat: "auto" | "json_schema" | "json_object" | "text";
  /** Empty feature models inherit the shared default. */
  models: Record<string, string>;
}

export interface AiSettings extends AiConnection {
  enabled: boolean;
  provider: AiProvider;
  instructions: string;
  featureInstructions: Record<string, string>;
  /** Remember connection details when switching providers; keys stay in the OS credential store. */
  connections: Partial<Record<AiProvider, AiConnection>>;
}

export interface IntegrationSettings {
  githubEnterpriseHost: string;
  gitlabHost: string;
  bitbucketUser: string;
  bitbucketServerHost: string;
  bitbucketServerUser: string;
  azureOrg: string;
  jiraSite: string;
  jiraEmail: string;
  jiraServerHost: string;
  trelloKey: string;
}

export interface RepoPrefs {
  /** Issue Tracker: "" = the hosting service's issues; jira, jira-server, trello, azure. */
  tracker?: { kind: string; project: string };
  /** Conflict Prevention for this repository, base branch patterns and excluded branches (one per line). */
  conflictDetection?: boolean;
  baseBranches?: string;
  excludedBranches?: string;
  /** Encoding: default encoding for reading / writing files. */
  encoding?: string;
}


export interface Workspace {
  id: string;
  name: string;
  repos: string[];
}

export interface Profile {
  id: string;
  name: string;
  gitName: string;
  gitEmail: string;
  /** Which profile picture (index into PROFILE_COLORS). */
  icon?: number;
  settings: Partial<Settings> | null;
}

export interface FormField {
  key: string;
  label: string;
  value: string | boolean;
  type?: "text" | "checkbox";
  placeholder?: string;
  /** Show a Browse button that picks a folder. */
  browse?: boolean;
}

export interface GraphColumns {
  refs: boolean;
  graph: boolean;
  description: boolean;
  author: boolean;
  date: boolean;
  sha: boolean;
  changes: boolean;
}

export type GraphColumn = "refs" | "graph" | "message" | "author" | "date" | "sha" | "changes";

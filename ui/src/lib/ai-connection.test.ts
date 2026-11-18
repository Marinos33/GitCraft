import { afterEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { ai, generateCommitMessage } from "./ai";
import { aiApi } from "./integrations";
import { flushPersistence, startPersistence } from "./persistence";
import { DEFAULT_AI, useStore } from "./store";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("./repo", () => ({ act: vi.fn() }));

const original = useStore.getState().settings;
afterEach(() => {
  useStore.setState({ settings: original });
  vi.mocked(invoke).mockReset();
});

describe("all AI features use the saved connection", () => {
  const requests: [string, () => Promise<unknown>][] = [
    ["ai_commit_message", () => ai.commitMessage("/repo")],
    ["ai_stash_message", () => ai.stashMessage("/repo")],
    ["ai_compose", () => ai.compose("/repo")],
    ["ai_explain_commit", () => ai.explainCommit("/repo", "abc")],
    ["ai_explain_branch", () => ai.explainBranch("/repo", "feature")],
    ["ai_pull_request", () => aiApi.pullRequest("/repo", "feature", "main")],
    ["ai_resolve_conflict", () => aiApi.resolveConflict("base", "ours", "theirs", "file")],
    ["ai_test_connection", () => aiApi.test()],
  ];
  it.each(requests)("%s sends no connection settings with the operation", async (command, run) => {
    vi.mocked(invoke).mockResolvedValueOnce("result");
    await expect(run()).resolves.toBe("result");
    expect(invoke).toHaveBeenCalledTimes(1);
    expect(vi.mocked(invoke).mock.calls[0][0]).toBe(command);
    expect(vi.mocked(invoke).mock.calls[0][1] ?? {}).not.toHaveProperty("config");
  });

  it("saves pending preference edits before a request, and sends nothing if saving fails", async () => {
    const stop = startPersistence();
    try {
      const config = { ...DEFAULT_AI, provider: "custom" as const, endpoint: "http://localhost:1234/v1" };
      useStore.getState().setSettings({ ai: config });
      vi.mocked(invoke).mockResolvedValueOnce(undefined).mockResolvedValueOnce("ok");
      await expect(aiApi.test()).resolves.toBe("ok");
      expect(vi.mocked(invoke).mock.calls.map(([command]) => command)).toEqual(["save_settings", "ai_test_connection"]);
      expect(vi.mocked(invoke).mock.calls[0][1]).toMatchObject({ settings: { settings: { ai: config } } });
      vi.mocked(invoke).mockReset();
      useStore.getState().setSettings({ ai: { ...config, defaultModel: "other" } });
      vi.mocked(invoke).mockRejectedValueOnce(new Error("settings unavailable"));
      await expect(aiApi.listModels()).rejects.toThrow("settings unavailable");
      expect(vi.mocked(invoke).mock.calls.map(([command]) => command)).toEqual(["save_settings"]);
    } finally {
      stop();
      vi.mocked(invoke).mockReset().mockResolvedValue(undefined);
      await flushPersistence();
    }
  });
});

it("discovers the program and models without changing the active connection or requesting a draft", async () => {
  const config = { ...DEFAULT_AI, provider: "claude_code" as const, cliConfigDir: "/personal/claude" };
  const model = { id: "sonnet", name: "Sonnet" };
  vi.mocked(invoke).mockResolvedValueOnce({ path: "/bin/claude", version: "Claude Code" }).mockResolvedValueOnce([model]);
  await expect(aiApi.findProgram(config.provider)).resolves.toMatchObject({ path: "/bin/claude" });
  await expect(aiApi.listModels()).resolves.toEqual([model]);
  expect(vi.mocked(invoke).mock.calls).toEqual([
    ["ai_find_program", { provider: "claude_code" }],
    ["ai_list_models", undefined],
  ]);
});

it("preserves a commit message edited while AI generation was running", async () => {
  const path = "/repo/ai-race";
  let finish!: (message: [string, string]) => void;
  const request = vi.spyOn(ai, "commitMessage").mockImplementation(() => new Promise((resolve) => { finish = resolve; }));
  try {
    const generating = generateCommitMessage(path);
    useStore.getState().setDraft(path, { summary: "Keep my newer draft", description: "Typed while waiting" });
    finish(["Generated suggestion", "Generated body"]);
    await generating;
    expect(useStore.getState().drafts[path].summary).toBe("Keep my newer draft");
    expect(useStore.getState().dialog).toMatchObject({ kind: "choice", text: expect.stringContaining("Generated suggestion") });
  } finally { request.mockRestore(); useStore.setState({ dialog: null }); }
});

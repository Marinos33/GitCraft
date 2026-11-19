import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({ invoke: vi.fn(), check: vi.fn(), relaunch: vi.fn(), listen: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: mocks.listen }));
vi.mock("@tauri-apps/plugin-process", () => ({ relaunch: mocks.relaunch }));

/** The Rust update commands: `update_check` answers with `mocks.check`, the rest with the offered update's mocks. */
function offeredUpdate() {
  return { version: "0.1.99", notes: "Release notes", channel: "stable", download: vi.fn().mockResolvedValue(undefined), install: vi.fn().mockResolvedValue(undefined) };
}
let current: ReturnType<typeof offeredUpdate> | null = null;
function backend(command: string) {
  if (command === "update_config") return Promise.resolve({ configured: true });
  if (command === "update_check") return Promise.resolve(mocks.check()).then((u) => { current = u; return u && { version: u.version, notes: u.notes, channel: u.channel }; });
  if (command === "update_download") return current!.download();
  if (command === "update_install") return current!.install();
  return Promise.resolve(undefined);
}

describe("automatic GitCraft updates", () => {
  let updates: typeof import("./updates");
  let useStore: typeof import("./store").useStore;
  let stop: (() => void) | undefined;

  beforeEach(async () => {
    vi.resetModules();
    vi.resetAllMocks();
    vi.useFakeTimers();
    current = null;
    mocks.invoke.mockImplementation(backend);
    mocks.listen.mockResolvedValue(() => undefined);
    mocks.check.mockResolvedValue(null);
    updates = await import("./updates");
    ({ useStore } = await import("./store"));
  });

  afterEach(() => {
    stop?.();
    stop = undefined;
    vi.useRealTimers();
  });

  it("checks at launch by default, then every hour, and cancels the schedule on cleanup", async () => {
    expect(updates.CHECK_EVERY_MS).toBe(3_600_000);
    expect(useStore.getState().settings.autoUpdate).toBe(true);
    stop = updates.startUpdateChecks();
    await vi.advanceTimersByTimeAsync(0);
    expect(mocks.check).toHaveBeenCalledTimes(1);
    await vi.advanceTimersByTimeAsync(updates.CHECK_EVERY_MS - 1);
    expect(mocks.check).toHaveBeenCalledTimes(1);
    await vi.advanceTimersByTimeAsync(1);
    expect(mocks.check).toHaveBeenCalledTimes(2);
    stop();
    await vi.advanceTimersByTimeAsync(updates.CHECK_EVERY_MS);
    expect(mocks.check).toHaveBeenCalledTimes(2);
  });

  it("skips background checks while disabled but allows a manual download", async () => {
    useStore.getState().setSettings({ autoUpdate: false });
    const update = offeredUpdate();
    mocks.check.mockResolvedValue(update);
    stop = updates.startUpdateChecks();
    await vi.advanceTimersByTimeAsync(updates.CHECK_EVERY_MS * 2);
    await updates.checkForUpdates(false);
    expect(mocks.invoke).not.toHaveBeenCalled();
    expect(mocks.check).not.toHaveBeenCalled();
    await updates.checkForUpdates(true);
    expect(mocks.check).toHaveBeenCalledTimes(1);
    expect(update.download).toHaveBeenCalledTimes(1);
    expect(updates.useUpdates.getState().phase).toBe("ready");
  });

  it("keeps a downloaded update ready across later checks and installs only on request", async () => {
    const update = offeredUpdate();
    mocks.check.mockResolvedValue(update);
    stop = updates.startUpdateChecks();
    await vi.advanceTimersByTimeAsync(0);
    expect(updates.useUpdates.getState()).toMatchObject({ phase: "ready", version: update.version });
    await vi.advanceTimersByTimeAsync(updates.CHECK_EVERY_MS * 3);
    expect(updates.useUpdates.getState().phase).toBe("ready");
    expect(mocks.check).toHaveBeenCalledTimes(1);
    expect(update.download).toHaveBeenCalledTimes(1);
    expect(update.install).not.toHaveBeenCalled();
    await updates.installAndRestart();
    expect(update.install).toHaveBeenCalledTimes(1);
    expect(mocks.relaunch).toHaveBeenCalledTimes(1);
  });

  it("keeps the ready update when saving preferences fails and retries before installation", async () => {
    const update=offeredUpdate();mocks.check.mockResolvedValue(update);
    stop=updates.startUpdateChecks();await vi.advanceTimersByTimeAsync(0);
    const {startPersistence}=await import("./persistence");const stopSaving=startPersistence();
    let fail=true;const order:string[]=[];
    mocks.invoke.mockImplementation(async(command:string)=>{if(command==="save_settings"){order.push("save");if(fail)throw new Error("disk full");return null;}return backend(command);});
    update.install.mockImplementation(async()=>{order.push("install");});
    useStore.getState().setSettings({rememberTabs:true});
    await updates.installAndRestart();expect(update.install).not.toHaveBeenCalled();expect(mocks.relaunch).not.toHaveBeenCalled();
    expect(updates.useUpdates.getState().phase).toBe("ready");
    fail=false;await updates.installAndRestart();expect(order).toEqual(["save","save","install"]);expect(mocks.relaunch).toHaveBeenCalledTimes(1);stopSaving();
  });

  it("stops an automatic check before download if the preference changes while it is checking", async () => {
    const update = offeredUpdate();
    let offer!: (value: ReturnType<typeof offeredUpdate>) => void;
    const started = new Promise<void>((resolve) => {
      mocks.check.mockImplementationOnce(() => new Promise((accept) => {
        offer = accept;
        resolve();
      }));
    });
    const checking = updates.checkForUpdates(false);
    await started;
    useStore.getState().setSettings({ autoUpdate: false });
    offer(update);
    await checking;
    expect(update.download).not.toHaveBeenCalled();
    expect(updates.useUpdates.getState().phase).toBe("available");
    await updates.download();
    expect(update.download).toHaveBeenCalledTimes(1);
  });

  it("does not overlap a scheduled check and a manual check", async () => {
    await Promise.all([updates.checkForUpdates(false), updates.checkForUpdates(true)]);
    expect(mocks.invoke.mock.calls.map(([command]) => command)).toEqual(["update_config", "update_check"]);
    expect(mocks.check).toHaveBeenCalledTimes(1);
  });

  it("saves a channel change before checking and reports download progress", async () => {
    const { startPersistence } = await import("./persistence");
    const stopSaving = startPersistence();
    let progress!: (event: { payload: { downloaded: number; total: number | null } }) => void;
    mocks.listen.mockImplementation(async (_event: string, handler: typeof progress) => { progress = handler; return () => undefined; });
    const update = offeredUpdate();
    update.download.mockImplementation(async () => { progress({ payload: { downloaded: 25, total: 100 } }); expect(updates.useUpdates.getState().percent).toBe(25); });
    mocks.check.mockResolvedValue(update);
    useStore.getState().setSettings({ updateChannel: "beta" });
    await updates.checkForUpdates(true);
    const commands = mocks.invoke.mock.calls.map(([command]) => command);
    expect(commands.indexOf("save_settings")).toBeLessThan(commands.indexOf("update_check"));
    expect(mocks.listen).toHaveBeenCalledWith("update-progress", expect.any(Function));
    expect(updates.useUpdates.getState()).toMatchObject({ phase: "ready", percent: 100 });
    stopSaving();
  });

  it("checks every hour", () => {
    expect(updates.CHECK_EVERY_MS).toBe(3_600_000);
  });

  it("shows the update menu lines for each state, with 'Last checked' cleared by a failure", () => {
    const now = 1_800_000_000_000;
    const base = { version: null, lastChecked: null, errorAt: null };
    const labels = (state: Parameters<typeof updates.updateMenuItems>[0]) => updates.updateMenuItems(state, now).map((i) => i.label);
    expect(labels({ ...base, phase: "idle" })).toEqual(["Check for Update"]);
    expect(labels({ ...base, phase: "none", lastChecked: now - 5_000 })).toEqual(["Check for Update (Last checked a few seconds ago)"]);
    expect(labels({ ...base, phase: "checking" })).toEqual(["Checking for updates..."]);
    expect(labels({ ...base, phase: "downloading" })).toEqual(["Downloading Update..."]);
    expect(labels({ ...base, phase: "ready" })).toEqual(["Restart and Install Update"]);
    expect(labels({ ...base, phase: "available", version: "0.2.0" })).toEqual(["Update available (0.2.0)"]);
    expect(labels({ ...base, phase: "error", errorAt: now - 120_000 })).toEqual(["Check for Update", "Update Errored (2 minutes ago)"]);
    expect(updates.updateMenuItems({ ...base, phase: "error", errorAt: now }, now).map((i) => i.disabled)).toEqual([false, true]);
  });
});

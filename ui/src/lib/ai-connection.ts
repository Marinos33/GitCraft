import { invoke } from "@tauri-apps/api/core";
import { flushPersistence } from "./persistence";

/** AI requests use the connection saved in the settings file. Pending preference edits are saved
 *  first, so a test or model list right after a change uses that change. */
export async function invokeAi<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  await flushPersistence();
  return invoke<T>(command, args);
}

import type { TabUi } from "./types";

let nextId = 1;
export const newId = () => `t${Date.now().toString(36)}${nextId++}`;

export const defaultUi = (): TabUi => ({ selection: { type: "wip" }, diff: null, search: "", searchOpen: false, multi: [], compareWorkdir: null, history: null, limit: 0, rebase: null, conflict: null, pr: null, issue: null });

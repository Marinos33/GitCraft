// What the Rust side reports once at start-up (launch_options) and the progress of long Git jobs
// ("job-progress" events). Both come from the back end, so they live in the react-query cache,
// not in the window's zustand store.
import { useSyncExternalStore } from "react";
import { useQuery } from "@tanstack/react-query";
import { api, type JobProgress, type Metrics } from "./api";
import { queryClient } from "./query-client";
import { DEFAULT_METRICS } from "./store";

export interface LaunchOptions { screenshot: boolean; shot_view: string | null; version: string; metrics: Metrics }

const LAUNCH_KEY = ["launchOptions"] as const;
const JOB_KEY = ["jobProgress"] as const;
export const DEFAULT_LAUNCH: LaunchOptions = { screenshot: false, shot_view: null, version: "0.1.0", metrics: DEFAULT_METRICS };

const readLaunch = async () => (await api.launchOptions().catch(() => null)) ?? DEFAULT_LAUNCH;

/** Read the launch options once (start-up). They do not change while GitCraft runs. */
export function loadLaunchOptions(): Promise<LaunchOptions> {
  return queryClient.fetchQuery({ queryKey: LAUNCH_KEY, queryFn: readLaunch, staleTime: Infinity, gcTime: Infinity });
}

/** The launch options outside React (defaults until start-up has read them). */
export function launchOptions(): LaunchOptions {
  return queryClient.getQueryData<LaunchOptions>(LAUNCH_KEY) ?? DEFAULT_LAUNCH;
}

export const launchMetrics = (): Metrics => launchOptions().metrics;

/** Calls `changed` when the launch options are written to the cache (once, at start-up). */
function subscribeLaunch(changed: () => void) {
  return queryClient.getQueryCache().subscribe((event) => { if (event.query.queryKey[0] === LAUNCH_KEY[0]) changed(); });
}

/** The launch options in a component. Every graph row reads them, so this is a plain cache
 *  subscription rather than a query observer (about three times cheaper to mount). */
export function useLaunchOptions(): LaunchOptions {
  return useSyncExternalStore(subscribeLaunch, launchOptions, launchOptions);
}

export const useMetrics = (): Metrics => useLaunchOptions().metrics;

/** Record the latest "job-progress" event (null when the job ended). */
export function setJobProgress(progress: JobProgress | null) {
  queryClient.setQueryData(JOB_KEY, progress);
}

/** The running clone / fetch / pull / push with its latest progress line. */
export function useJobProgress(): JobProgress | null {
  return useQuery<JobProgress | null>({ queryKey: JOB_KEY, queryFn: () => null, enabled: false, staleTime: Infinity, gcTime: Infinity }).data ?? null;
}

/** The running job outside React. */
export function jobProgress(): JobProgress | null {
  return queryClient.getQueryData<JobProgress | null>(JOB_KEY) ?? null;
}

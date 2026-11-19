import { QueryClient } from "@tanstack/react-query";

/** The one react-query cache for data read from the Rust side. */
export const queryClient = new QueryClient({
  defaultOptions: { queries: { refetchOnWindowFocus: true, staleTime: 2000, retry: false } },
});

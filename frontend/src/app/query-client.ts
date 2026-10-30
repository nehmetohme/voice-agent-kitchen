import { QueryClient } from '@tanstack/react-query';

/** One QueryClient per app (and per test). Defaults favour a calm UI over refetch churn. */
export function createQueryClient(): QueryClient {
  return new QueryClient({
    defaultOptions: {
      queries: { staleTime: 30_000, retry: 1, refetchOnWindowFocus: false },
    },
  });
}

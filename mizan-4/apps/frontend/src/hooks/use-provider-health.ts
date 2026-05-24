import { useQuery } from "@tanstack/react-query";

import { getProviderHealth, logger } from "@/adapters";
import { QueryKeys } from "@/lib/query-keys";
import { ProviderHealth } from "@/lib/types";

export function useProviderHealth() {
  return useQuery<ProviderHealth[], Error>({
    queryKey: [QueryKeys.PROVIDER_HEALTH],
    queryFn: async () => {
      try {
        return await getProviderHealth();
      } catch (error) {
        const errorMessage = error instanceof Error ? error.message : "Unknown error";
        logger.error(`Error fetching provider health in useProviderHealth: ${errorMessage}`);
        throw new Error(errorMessage);
      }
    },
  });
}

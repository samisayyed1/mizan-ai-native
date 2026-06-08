import {
  getGoal,
  getGoalFunding,
  getGoalPlan,
  previewSaveUpOverview,
  getRetirementOverview,
  getSaveUpOverview,
  saveGoalFunding,
  saveGoalPlan,
  refreshGoalSummary,
} from "@/adapters";
import { QueryKeys } from "@/lib/query-keys";
import type {
  Goal,
  GoalFundingRule,
  GoalFundingRuleInput,
  GoalPlan,
  RetirementOverview,
  SaveGoalPlan,
  SaveUpOverviewDTO,
  SaveUpPreviewInputDTO,
} from "@/lib/types";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useMemo } from "react";
import { toast } from "sonner";

export function useGoalDetail(goalId: string | undefined) {
  const goal = useQuery<Goal, Error>({
    queryKey: QueryKeys.goal(goalId ?? ""),
    queryFn: () => getGoal(goalId!),
    enabled: !!goalId,
  });

  const plan = useQuery<GoalPlan | null, Error>({
    queryKey: QueryKeys.goalPlan(goalId ?? ""),
    queryFn: () => getGoalPlan(goalId!),
    enabled: !!goalId,
  });

  const funding = useQuery<GoalFundingRule[], Error>({
    queryKey: QueryKeys.goalFunding(goalId ?? ""),
    queryFn: () => getGoalFunding(goalId!),
    enabled: !!goalId,
  });

  const fundingRules = useMemo(() => funding.data ?? [], [funding.data]);

  return {
    goal: goal.data,
    plan: plan.data,
    fundingRules,
    isLoading: goal.isLoading || plan.isLoading || funding.isLoading,
    error: goal.error || plan.error || funding.error,
  };
}

export function useGoalPlanMutations(goalId: string) {
  const queryClient = useQueryClient();

  const invalidateGoal = () => {
    queryClient.invalidateQueries({ queryKey: QueryKeys.goalPlan(goalId) });
    queryClient.invalidateQueries({ queryKey: QueryKeys.goal(goalId) });
    queryClient.invalidateQueries({ queryKey: [QueryKeys.GOALS] });
    queryClient.invalidateQueries({ queryKey: QueryKeys.saveUpOverview(goalId) });
    queryClient.invalidateQueries({ queryKey: QueryKeys.retirementOverview(goalId) });
  };

  const savePlanMutation = useMutation({
    mutationFn: (plan: SaveGoalPlan) => saveGoalPlan(plan),
    onSuccess: () => {
      invalidateGoal();
      toast.success("Plan saved successfully.");
    },
    onError: () => toast.error("Failed to save plan."),
  });

  const saveFundingMutation = useMutation({
    mutationFn: (rules: GoalFundingRuleInput[]) => saveGoalFunding(goalId, rules),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: QueryKeys.goalFunding(goalId) });
      invalidateGoal();
      toast.success("Funding saved successfully.");
    },
    onError: (e) => toast.error(e instanceof Error ? e.message : "Failed to save funding."),
  });

  const refreshSummaryMutation = useMutation({
    mutationFn: () => refreshGoalSummary(goalId),
    onSuccess: () => {
      invalidateGoal();
    },
  });

  return { savePlanMutation, saveFundingMutation, refreshSummaryMutation };
}

export function useRetirementOverview(goalId: string | undefined) {
  return useQuery<RetirementOverview, Error>({
    queryKey: QueryKeys.retirementOverview(goalId ?? ""),
    queryFn: async () => {
      // 15s soft timeout. The backend computes a Monte Carlo glidepath
      // which should land in well under a second on the reference machine;
      // 15s of silence means the dispatcher hung. Throwing here flips the
      // query into an error state so the page renders an actionable card
      // instead of the infinite skeleton it would otherwise show.
      const TIMEOUT_MS = 15_000;
      let timer: ReturnType<typeof setTimeout> | undefined;
      try {
        return await Promise.race([
          getRetirementOverview(goalId!),
          new Promise<RetirementOverview>((_, reject) => {
            timer = setTimeout(
              () =>
                reject(
                  new Error(
                    "Retirement projection timed out after 15 seconds. The backend may be busy or disconnected.",
                  ),
                ),
              TIMEOUT_MS,
            );
          }),
        ]);
      } finally {
        if (timer) clearTimeout(timer);
      }
    },
    enabled: !!goalId,
    // Retry once for transient backend hiccups; after that, surface the
    // error so the user sees the "Failed to load retirement projection"
    // card instead of an infinite skeleton.
    retry: 1,
    retryDelay: 500,
    staleTime: 5 * 60 * 1000,
    gcTime: 10 * 60 * 1000,
  });
}

export function useSaveUpOverview(goalId: string | undefined) {
  return useQuery<SaveUpOverviewDTO, Error>({
    queryKey: QueryKeys.saveUpOverview(goalId ?? ""),
    queryFn: () => getSaveUpOverview(goalId!),
    enabled: !!goalId,
  });
}

export function useSaveUpPreview(input: SaveUpPreviewInputDTO | null) {
  return useQuery<SaveUpOverviewDTO, Error>({
    queryKey: [QueryKeys.SAVE_UP_PREVIEW, input],
    queryFn: () => previewSaveUpOverview(input!),
    enabled: !!input,
  });
}

import type { ToolCallMessagePartProps } from "@assistant-ui/react";
import { makeAssistantToolUI } from "@assistant-ui/react";
import {
  Badge,
  Button,
  Card,
  CardContent,
  CardHeader,
  CardTitle,
  Input,
  Label,
  Skeleton,
} from "@mizan/ui";
import { Icons } from "@mizan/ui/components/ui/icons";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@mizan/ui/components/ui/select";
import { memo, useMemo, useState } from "react";
import { Link } from "react-router-dom";

import { updateToolResult } from "@/adapters";
import { useGoalMutations } from "@/features/goals/hooks/use-goals";

import { useRuntimeContext } from "../../hooks/use-runtime-context";
import { useQueryClient } from "@tanstack/react-query";
import { refreshAfterMutation, unwrapToolResult } from "./shared";

// ============================================================================
// Types (mirror crates/ai/src/tools/create_goal.rs)
// ============================================================================

interface CreateGoalArgs {
  title: string;
  goalType: string;
  targetAmount?: number;
  currency?: string;
  targetDate?: string;
  startDate?: string;
  description?: string;
  priority?: number;
}

type GoalType =
  | "retirement"
  | "home"
  | "education"
  | "wedding"
  | "emergency_fund"
  | "custom_save_up";

interface GoalDraft {
  title: string;
  goalType: GoalType;
  targetAmount: number | null;
  currency: string;
  targetDate: string | null;
  startDate: string | null;
  description: string | null;
  priority: number;
}

interface ValidationResult {
  isValid: boolean;
  missingFields: string[];
  warnings: string[];
}

interface ExistingGoal {
  id: string;
  title: string;
  goalType: string;
}

interface GoalTypeOption {
  value: GoalType;
  label: string;
}

interface CreateGoalOutput {
  draft: GoalDraft;
  validation: ValidationResult;
  availableTypes: GoalTypeOption[];
  availableCurrencies: string[];
  existingGoals: ExistingGoal[];
}

interface CreateGoalResult extends CreateGoalOutput {
  submitted?: boolean;
  createdGoalId?: string;
}

type Props = ToolCallMessagePartProps<CreateGoalArgs, CreateGoalResult>;

function normaliseResult(raw: unknown): CreateGoalResult | null {
  if (!raw) return null;
  if (typeof raw === "string") {
    try {
      return normaliseResult(JSON.parse(raw));
    } catch {
      return null;
    }
  }
  const unwrapped = unwrapToolResult(raw, "draft");
  if (!unwrapped || typeof unwrapped !== "object") return null;
  const obj = unwrapped as Partial<CreateGoalResult>;
  if (!obj.draft || typeof obj.draft !== "object") return null;
  return obj as CreateGoalResult;
}

function LoadingSkeleton() {
  return (
    <Card className="bg-muted/40 border-primary/10">
      <CardHeader className="pb-2">
        <Skeleton className="h-5 w-40" />
      </CardHeader>
      <CardContent className="space-y-3">
        <Skeleton className="h-4 w-3/4" />
        <Skeleton className="h-4 w-1/2" />
        <div className="flex gap-2 pt-2">
          <Skeleton className="h-9 w-24" />
          <Skeleton className="h-9 w-28" />
        </div>
      </CardContent>
    </Card>
  );
}

function SuccessState({
  draft,
  createdGoalId,
}: {
  draft: GoalDraft;
  createdGoalId?: string;
}) {
  return (
    <Card className="border-success/30 bg-success/5">
      <CardHeader className="pb-2">
        <div className="flex items-center gap-2">
          <Icons.CheckCircle className="text-success size-5" />
          <CardTitle className="text-base">Goal created</CardTitle>
        </div>
      </CardHeader>
      <CardContent className="space-y-2 text-sm">
        <p>
          <span className="font-medium">{draft.title}</span>
          <span className="text-muted-foreground"> · {draft.goalType}</span>
          {draft.targetAmount !== null && (
            <span className="text-muted-foreground">
              {" "}
              · {draft.currency} {draft.targetAmount.toLocaleString()}
            </span>
          )}
        </p>
        {createdGoalId && (
          <Link
            to={`/goals/${createdGoalId}`}
            className="text-primary inline-flex items-center gap-1 text-xs hover:underline"
          >
            View goal
            <Icons.ArrowRight className="size-3" />
          </Link>
        )}
      </CardContent>
    </Card>
  );
}

function ReviewRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-baseline justify-between gap-2">
      <dt className="text-muted-foreground text-xs uppercase tracking-wide">{label}</dt>
      <dd className="font-medium">{value}</dd>
    </div>
  );
}

function DraftCard({
  initialDraft,
  warnings,
  availableTypes,
  availableCurrencies,
  toolCallId,
  onSuccess,
}: {
  initialDraft: GoalDraft;
  warnings: string[];
  availableTypes: GoalTypeOption[];
  availableCurrencies: string[];
  toolCallId: string;
  onSuccess: (createdGoalId: string) => void;
}) {
  const runtime = useRuntimeContext();
  const queryClient = useQueryClient();
  const threadId = runtime.currentThreadId;

  const [isEditing, setIsEditing] = useState(false);
  const [draft, setDraft] = useState<GoalDraft>(initialDraft);

  const { createMutation } = useGoalMutations();

  const canConfirm =
    draft.title.trim().length >= 2 &&
    !!draft.goalType &&
    draft.targetAmount !== null &&
    draft.targetAmount > 0 &&
    !createMutation.isPending;

  const handleConfirm = async () => {
    try {
      const created = await createMutation.mutateAsync({
        title: draft.title.trim(),
        goalType: draft.goalType,
        targetAmount: draft.targetAmount ?? 0,
        currency: draft.currency,
        targetDate: draft.targetDate ?? undefined,
        startDate: draft.startDate ?? undefined,
        description: draft.description ?? undefined,
      });

      refreshAfterMutation(queryClient);

      if (threadId) {
        try {
          await updateToolResult({
            threadId,
            toolCallId,
            resultPatch: {
              submitted: true,
              createdGoalId: created.id,
            },
          });
        } catch (e) {
          console.error("Failed to persist tool result:", e);
        }
      }
      onSuccess(created.id);
    } catch {
      // toast handled
    }
  };

  return (
    <Card className="bg-muted/40 border-primary/10">
      <CardHeader className="pb-3">
        <div className="flex items-center gap-2">
          <CardTitle className="text-base">
            {isEditing ? "Edit goal draft" : "Create goal?"}
          </CardTitle>
          <Badge variant="secondary" className="text-xs">
            {availableTypes.find((t) => t.value === draft.goalType)?.label ?? draft.goalType}
          </Badge>
        </div>
        <p className="text-muted-foreground mt-1 text-xs">
          Review before saving. Nothing is created until you click Confirm.
        </p>
      </CardHeader>
      <CardContent className="space-y-4">
        {warnings.length > 0 && (
          <div className="border-warning/40 bg-warning/10 text-warning rounded-md border px-3 py-2 text-xs">
            {warnings.map((w, i) => (
              <div key={i}>{w}</div>
            ))}
          </div>
        )}

        {isEditing ? (
          <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
            <div className="col-span-full">
              <Label htmlFor="ai-create-goal-title">Title</Label>
              <Input
                id="ai-create-goal-title"
                value={draft.title}
                onChange={(e) => setDraft({ ...draft, title: e.target.value })}
                className="mt-1.5"
              />
            </div>
            <div>
              <Label htmlFor="ai-create-goal-type">Type</Label>
              <Select
                value={draft.goalType}
                onValueChange={(v) =>
                  setDraft({ ...draft, goalType: v as GoalType })
                }
              >
                <SelectTrigger id="ai-create-goal-type" className="mt-1.5">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {availableTypes.map((t) => (
                    <SelectItem key={t.value} value={t.value}>
                      {t.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            <div>
              <Label htmlFor="ai-create-goal-currency">Currency</Label>
              <Select
                value={draft.currency}
                onValueChange={(v) => setDraft({ ...draft, currency: v })}
              >
                <SelectTrigger id="ai-create-goal-currency" className="mt-1.5">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {availableCurrencies.map((c) => (
                    <SelectItem key={c} value={c}>
                      {c}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            <div>
              <Label htmlFor="ai-create-goal-target">Target amount</Label>
              <Input
                id="ai-create-goal-target"
                type="number"
                inputMode="decimal"
                value={draft.targetAmount ?? ""}
                onChange={(e) =>
                  setDraft({
                    ...draft,
                    targetAmount: e.target.value === "" ? null : Number(e.target.value),
                  })
                }
                className="mt-1.5"
              />
            </div>
            <div>
              <Label htmlFor="ai-create-goal-date">Target date</Label>
              <Input
                id="ai-create-goal-date"
                type="date"
                value={draft.targetDate ?? ""}
                onChange={(e) =>
                  setDraft({
                    ...draft,
                    targetDate: e.target.value ? e.target.value : null,
                  })
                }
                className="mt-1.5"
              />
            </div>
          </div>
        ) : (
          <dl className="grid grid-cols-1 gap-x-6 gap-y-2 text-sm sm:grid-cols-2">
            <ReviewRow label="Title" value={draft.title || "—"} />
            <ReviewRow
              label="Type"
              value={
                availableTypes.find((t) => t.value === draft.goalType)?.label ?? draft.goalType
              }
            />
            <ReviewRow
              label="Target"
              value={
                draft.targetAmount !== null
                  ? `${draft.currency} ${draft.targetAmount.toLocaleString()}`
                  : "—"
              }
            />
            <ReviewRow label="Target date" value={draft.targetDate ?? "open-ended"} />
            {draft.description && (
              <div className="col-span-full">
                <dt className="text-muted-foreground text-xs uppercase tracking-wide">
                  Description
                </dt>
                <dd className="mt-0.5">{draft.description}</dd>
              </div>
            )}
          </dl>
        )}

        <div className="flex flex-wrap items-center justify-end gap-2 pt-2">
          {isEditing ? (
            <Button
              variant="ghost"
              size="sm"
              onClick={() => {
                setDraft(initialDraft);
                setIsEditing(false);
              }}
            >
              Cancel edits
            </Button>
          ) : (
            <Button variant="ghost" size="sm" onClick={() => setIsEditing(true)}>
              <Icons.Pencil className="mr-1.5 size-3.5" />
              Edit
            </Button>
          )}
          <Button onClick={handleConfirm} disabled={!canConfirm} size="sm">
            {createMutation.isPending ? (
              <>
                <Icons.Spinner className="mr-1.5 size-3.5 animate-spin" />
                Saving…
              </>
            ) : (
              <>
                <Icons.Check className="mr-1.5 size-3.5" />
                Confirm & create
              </>
            )}
          </Button>
        </div>

        {draft.targetAmount === null || draft.targetAmount <= 0 ? (
          <p className="text-muted-foreground text-xs">
            A target amount is required to save the goal.
          </p>
        ) : null}
      </CardContent>
    </Card>
  );
}

export function CreateGoalToolUIContentImpl({ result, status, toolCallId }: Props) {
  const parsed = useMemo(() => normaliseResult(result), [result]);
  const [submitSuccess, setSubmitSuccess] = useState<
    { submitted: true; createdGoalId: string } | { submitted: false }
  >({ submitted: false });

  if (status?.type === "running") return <LoadingSkeleton />;
  if (status?.type === "incomplete") {
    return (
      <Card className="border-destructive/30 bg-destructive/5">
        <CardContent className="py-4">
          <p className="text-destructive text-sm font-medium">
            Couldn't prepare the goal draft.
          </p>
        </CardContent>
      </Card>
    );
  }
  if (!parsed) {
    return (
      <Card className="border-destructive/30 bg-destructive/5">
        <CardContent className="py-4">
          <p className="text-destructive text-sm font-medium">No goal draft available.</p>
        </CardContent>
      </Card>
    );
  }

  const submitted = parsed.submitted || submitSuccess.submitted;
  if (submitted) {
    return (
      <SuccessState
        draft={parsed.draft}
        createdGoalId={
          parsed.createdGoalId ??
          (submitSuccess.submitted ? submitSuccess.createdGoalId : undefined)
        }
      />
    );
  }

  return (
    <DraftCard
      initialDraft={parsed.draft}
      warnings={parsed.validation.warnings}
      availableTypes={parsed.availableTypes}
      availableCurrencies={parsed.availableCurrencies}
      toolCallId={toolCallId}
      onSuccess={(createdGoalId) =>
        setSubmitSuccess({ submitted: true, createdGoalId })
      }
    />
  );
}

const CreateGoalToolUIContent = memo(CreateGoalToolUIContentImpl);

export const CreateGoalToolUI = makeAssistantToolUI<CreateGoalArgs, CreateGoalResult>({
  toolName: "create_goal",
  render: (props) => <CreateGoalToolUIContent {...props} />,
});

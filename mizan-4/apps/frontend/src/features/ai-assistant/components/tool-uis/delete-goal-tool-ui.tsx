import type { ToolCallMessagePartProps } from "@assistant-ui/react";
import { makeAssistantToolUI } from "@assistant-ui/react";
import { Badge, Button, Card, CardContent, CardHeader, CardTitle, Skeleton } from "@mizan/ui";
import { Icons } from "@mizan/ui/components/ui/icons";
import { memo, useMemo, useState } from "react";

import { updateToolResult } from "@/adapters";
import { useGoalMutations } from "@/features/goals/hooks/use-goals";

import { useRuntimeContext } from "../../hooks/use-runtime-context";
import { useQueryClient } from "@tanstack/react-query";
import { refreshAfterMutation, unwrapToolResult } from "./shared";

// Types mirror crates/ai/src/tools/delete_goal.rs

interface DeleteGoalArgs {
  goalRef: string;
  reason?: string;
}

interface GoalSnapshot {
  id: string;
  title: string;
  goalType: string;
  targetAmount: number | null;
  currency: string | null;
  statusLifecycle: string;
  progressPercent: number | null;
  currentValue: number | null;
  targetDate: string | null;
}

interface DeletionImpact {
  hadFundingRules: boolean;
  hadPlan: boolean;
  isActive: boolean;
}

interface ValidationResult {
  resolved: boolean;
  candidates: GoalSnapshot[];
  warnings: string[];
  isValid: boolean;
}

interface DeleteGoalOutput {
  target: GoalSnapshot;
  impact: DeletionImpact;
  reason: string | null;
  validation: ValidationResult;
}

interface DeleteGoalResult extends DeleteGoalOutput {
  submitted?: boolean;
  deletedAt?: string;
}

type Props = ToolCallMessagePartProps<DeleteGoalArgs, DeleteGoalResult>;

function normaliseResult(raw: unknown): DeleteGoalResult | null {
  if (!raw) return null;
  if (typeof raw === "string") {
    try {
      return normaliseResult(JSON.parse(raw));
    } catch {
      return null;
    }
  }
  const unwrapped = unwrapToolResult(raw, "target");
  if (!unwrapped || typeof unwrapped !== "object") return null;
  const obj = unwrapped as Partial<DeleteGoalResult>;
  if (!obj.target || !obj.impact || !obj.validation) return null;
  return obj as DeleteGoalResult;
}

function LoadingSkeleton() {
  return (
    <Card className="bg-muted/40 border-destructive/10">
      <CardHeader className="pb-2">
        <Skeleton className="h-5 w-44" />
      </CardHeader>
      <CardContent className="space-y-3">
        <Skeleton className="h-4 w-3/4" />
        <Skeleton className="h-4 w-1/2" />
      </CardContent>
    </Card>
  );
}

function ErrorCard({ title, body }: { title: string; body: string }) {
  return (
    <Card className="border-destructive/30 bg-destructive/5">
      <CardContent className="py-4">
        <p className="text-destructive text-sm font-medium">{title}</p>
        <p className="text-muted-foreground mt-1 text-xs">{body}</p>
      </CardContent>
    </Card>
  );
}

function SuccessState({ target }: { target: GoalSnapshot }) {
  return (
    <Card className="border-success/30 bg-success/5">
      <CardHeader className="pb-2">
        <div className="flex items-center gap-2">
          <Icons.CheckCircle className="text-success size-5" />
          <CardTitle className="text-base">Goal deleted</CardTitle>
        </div>
      </CardHeader>
      <CardContent className="text-sm">
        <p>
          <span className="font-medium">{target.title}</span>
          <span className="text-muted-foreground"> · {target.goalType}</span>
        </p>
      </CardContent>
    </Card>
  );
}

function UnresolvedCard({
  candidates,
  warnings,
}: {
  candidates: GoalSnapshot[];
  warnings: string[];
}) {
  return (
    <Card className="border-warning/30 bg-warning/5">
      <CardHeader className="pb-2">
        <div className="flex items-center gap-2">
          <Icons.AlertTriangle className="text-warning size-5" />
          <CardTitle className="text-base">Couldn't pick a target</CardTitle>
        </div>
      </CardHeader>
      <CardContent className="space-y-3 text-sm">
        {warnings.map((w, i) => (
          <p key={i} className="text-muted-foreground text-xs">
            {w}
          </p>
        ))}
        {candidates.length > 0 && (
          <div className="space-y-1">
            <p className="text-xs font-medium uppercase tracking-wide">Possible matches</p>
            <ul className="space-y-1">
              {candidates.map((c) => (
                <li key={c.id} className="text-xs">
                  <span className="font-medium">{c.title}</span>
                  <span className="text-muted-foreground"> · {c.goalType}</span>
                </li>
              ))}
            </ul>
          </div>
        )}
      </CardContent>
    </Card>
  );
}

function ConfirmCard({
  target,
  impact,
  reason,
  warnings,
  toolCallId,
  onSuccess,
}: {
  target: GoalSnapshot;
  impact: DeletionImpact;
  reason: string | null;
  warnings: string[];
  toolCallId: string;
  onSuccess: () => void;
}) {
  const runtime = useRuntimeContext();
  const queryClient = useQueryClient();
  const threadId = runtime.currentThreadId;
  const [armed, setArmed] = useState(false);
  const { deleteMutation } = useGoalMutations();

  const handleConfirm = async () => {
    try {
      await deleteMutation.mutateAsync(target.id);
      refreshAfterMutation(queryClient);
      if (threadId) {
        try {
          await updateToolResult({
            threadId,
            toolCallId,
            resultPatch: { submitted: true, deletedAt: new Date().toISOString() },
          });
        } catch (e) {
          console.error("Failed to persist tool result:", e);
        }
      }
      onSuccess();
    } catch {
      // Toast handled by useGoalMutations onError.
    }
  };

  const progress =
    typeof target.progressPercent === "number"
      ? `${Math.round(target.progressPercent)}%`
      : null;

  return (
    <Card className="border-destructive/30 bg-destructive/5">
      <CardHeader className="pb-3">
        <div className="flex items-center gap-2">
          <Icons.Trash2 className="text-destructive size-4" />
          <CardTitle className="text-base">Delete goal?</CardTitle>
          {impact.isActive && (
            <Badge variant="secondary" className="text-xs">
              Active
            </Badge>
          )}
        </div>
        <p className="text-muted-foreground mt-1 text-xs">
          This is irreversible. Funding rules + plans attached to this goal will be lost too.
        </p>
      </CardHeader>

      <CardContent className="space-y-3 text-sm">
        <div className="bg-background/50 rounded-md border px-3 py-2">
          <p>
            <span className="font-medium">{target.title}</span>
            <span className="text-muted-foreground"> · {target.goalType}</span>
          </p>
          {(progress || target.targetDate) && (
            <p className="text-muted-foreground mt-0.5 text-xs">
              {progress && <>Progress: {progress}</>}
              {progress && target.targetDate && <> · </>}
              {target.targetDate && <>Target date: {target.targetDate}</>}
            </p>
          )}
          {reason && (
            <p className="text-muted-foreground mt-1 text-xs italic">"{reason}"</p>
          )}
        </div>

        {warnings.length > 0 && (
          <div className="border-warning/40 bg-warning/10 text-warning rounded-md border px-3 py-2 text-xs">
            {warnings.map((w, i) => (
              <div key={i}>{w}</div>
            ))}
          </div>
        )}

        <div className="flex flex-wrap items-center justify-end gap-2 pt-1">
          <Button
            variant="ghost"
            size="sm"
            onClick={() => setArmed(false)}
            disabled={!armed || deleteMutation.isPending}
          >
            Cancel
          </Button>
          {armed ? (
            <Button
              variant="destructive"
              size="sm"
              onClick={handleConfirm}
              disabled={deleteMutation.isPending}
            >
              {deleteMutation.isPending ? (
                <>
                  <Icons.Spinner className="mr-1.5 size-3.5 animate-spin" />
                  Deleting…
                </>
              ) : (
                <>
                  <Icons.Trash2 className="mr-1.5 size-3.5" />
                  Yes, delete forever
                </>
              )}
            </Button>
          ) : (
            <Button variant="destructive" size="sm" onClick={() => setArmed(true)}>
              <Icons.Trash2 className="mr-1.5 size-3.5" />
              Delete
            </Button>
          )}
        </div>
      </CardContent>
    </Card>
  );
}

function DeleteGoalToolUIContentImpl({ result, status, toolCallId }: Props) {
  const parsed = useMemo(() => normaliseResult(result), [result]);
  const [submitted, setSubmitted] = useState(false);

  if (status?.type === "running") return <LoadingSkeleton />;
  if (status?.type === "incomplete")
    return (
      <ErrorCard
        title="Couldn't prepare the deletion"
        body="The request was interrupted before the target was confirmed."
      />
    );
  if (!parsed)
    return (
      <ErrorCard
        title="No target available"
        body="The AI didn't return a parseable delete request."
      />
    );

  if (parsed.submitted || submitted) return <SuccessState target={parsed.target} />;

  if (!parsed.validation.resolved || !parsed.validation.isValid) {
    return (
      <UnresolvedCard
        candidates={parsed.validation.candidates}
        warnings={parsed.validation.warnings}
      />
    );
  }

  return (
    <ConfirmCard
      target={parsed.target}
      impact={parsed.impact}
      reason={parsed.reason}
      warnings={parsed.validation.warnings}
      toolCallId={toolCallId}
      onSuccess={() => setSubmitted(true)}
    />
  );
}

const DeleteGoalToolUIContent = memo(DeleteGoalToolUIContentImpl);

export const DeleteGoalToolUI = makeAssistantToolUI<DeleteGoalArgs, DeleteGoalResult>({
  toolName: "delete_goal",
  render: (props) => <DeleteGoalToolUIContent {...props} />,
});

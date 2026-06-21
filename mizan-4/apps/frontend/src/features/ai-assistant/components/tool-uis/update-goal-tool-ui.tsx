import type { ToolCallMessagePartProps } from "@assistant-ui/react";
import { makeAssistantToolUI } from "@assistant-ui/react";
import {
  Badge,
  Button,
  Card,
  CardContent,
  CardHeader,
  CardTitle,
  Skeleton,
} from "@mizan/ui";
import { Icons } from "@mizan/ui/components/ui/icons";
import { memo, useMemo, useState } from "react";

import { updateToolResult } from "@/adapters";
import { useGoalMutations } from "@/features/goals/hooks/use-goals";
import { useQueryClient } from "@tanstack/react-query";
import type { Goal, GoalLifecycle } from "@/lib/types";

import { useRuntimeContext } from "../../hooks/use-runtime-context";
import { refreshAfterMutation, unwrapToolResult } from "./shared";

// ============================================================================
// Types (mirror crates/ai/src/tools/update_goal.rs)
// ============================================================================

interface UpdateGoalArgs {
  goalRef: string;
  title?: string;
  targetAmount?: number;
  targetDate?: string;
  priority?: number;
  statusLifecycle?: string;
  description?: string;
}

interface GoalSnapshot {
  id: string;
  title: string;
  goalType: string;
  targetAmount: number | null;
  currency: string | null;
  statusLifecycle: string;
  priority: number;
  targetDate: string | null;
  description: string | null;
}

interface GoalUpdateDraft {
  id: string;
  title: string;
  targetAmount: number | null;
  statusLifecycle: string;
  priority: number;
  targetDate: string | null;
  description: string | null;
}

interface FieldDiff {
  field: string;
  old: string | null;
  new: string | null;
}

interface ValidationResult {
  resolved: boolean;
  candidates: GoalSnapshot[];
  warnings: string[];
  isValid: boolean;
  hasChanges: boolean;
}

interface UpdateGoalOutput {
  draft: GoalUpdateDraft;
  current: GoalSnapshot;
  diff: FieldDiff[];
  validation: ValidationResult;
}

interface UpdateGoalResult extends UpdateGoalOutput {
  submitted?: boolean;
  updatedAt?: string;
}

type Props = ToolCallMessagePartProps<UpdateGoalArgs, UpdateGoalResult>;

function normaliseResult(raw: unknown): UpdateGoalResult | null {
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
  const obj = unwrapped as Partial<UpdateGoalResult>;
  if (!obj.draft || !obj.current || !obj.validation) return null;
  return obj as UpdateGoalResult;
}

function formatField(field: string): string {
  switch (field) {
    case "title":
      return "Title";
    case "targetAmount":
      return "Target amount";
    case "targetDate":
      return "Target date";
    case "priority":
      return "Priority";
    case "statusLifecycle":
      return "Status";
    case "description":
      return "Description";
    default:
      return field;
  }
}

function LoadingSkeleton() {
  return (
    <Card className="bg-muted/40 border-primary/10">
      <CardHeader className="pb-2">
        <Skeleton className="h-5 w-44" />
      </CardHeader>
      <CardContent className="space-y-3">
        <Skeleton className="h-4 w-3/4" />
        <Skeleton className="h-9 w-28" />
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

function SuccessState({ title }: { title: string }) {
  return (
    <Card className="border-success/30 bg-success/5">
      <CardHeader className="pb-2">
        <div className="flex items-center gap-2">
          <Icons.CheckCircle className="text-success size-5" />
          <CardTitle className="text-base">Goal updated</CardTitle>
        </div>
      </CardHeader>
      <CardContent className="text-sm">{title}</CardContent>
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
          <CardTitle className="text-base">Couldn't pick a goal</CardTitle>
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

function DiffRow({ row }: { row: FieldDiff }) {
  return (
    <div className="flex items-baseline gap-3 text-sm">
      <span className="text-muted-foreground w-32 shrink-0 text-xs uppercase tracking-wide">
        {formatField(row.field)}
      </span>
      <span className="text-muted-foreground line-through text-xs">
        {row.old ?? "—"}
      </span>
      <Icons.ArrowRight className="text-muted-foreground size-3" />
      <span className="font-medium">{row.new ?? "(clear)"}</span>
    </div>
  );
}

function DraftCard({
  draft,
  current,
  diff,
  warnings,
  toolCallId,
  onSuccess,
}: {
  draft: GoalUpdateDraft;
  current: GoalSnapshot;
  diff: FieldDiff[];
  warnings: string[];
  toolCallId: string;
  onSuccess: () => void;
}) {
  const runtime = useRuntimeContext();
  const queryClient = useQueryClient();
  const threadId = runtime.currentThreadId;
  const { updateMutation } = useGoalMutations();

  const handleConfirm = async () => {
    // Merge draft into the current goal so we send a complete Goal object
    // to the backend. Fields the AI didn't touch keep their current value.
    const updated: Goal = {
      id: current.id,
      goalType: current.goalType as Goal["goalType"],
      title: draft.title,
      description: draft.description ?? undefined,
      targetAmount: draft.targetAmount ?? undefined,
      statusLifecycle: draft.statusLifecycle as GoalLifecycle,
      // statusHealth is recomputed server-side from progress; we don't
      // touch it here. Cast to satisfy the type — backend ignores it on
      // update and rederives from the latest valuation.
      statusHealth: "ON_TRACK" as Goal["statusHealth"],
      priority: draft.priority,
      currency: current.currency ?? undefined,
      targetDate: draft.targetDate ?? undefined,
      // Audit fields the backend rewrites — supplying placeholders so
      // the type is satisfied; mutation fn ignores them.
      createdAt: "",
      updatedAt: "",
    };

    try {
      await updateMutation.mutateAsync(updated);
      refreshAfterMutation(queryClient);
      if (threadId) {
        try {
          await updateToolResult({
            threadId,
            toolCallId,
            resultPatch: { submitted: true, updatedAt: new Date().toISOString() },
          });
        } catch (e) {
          console.error("Failed to persist tool result:", e);
        }
      }
      onSuccess();
    } catch {
      // toast handled by useGoalMutations onError.
    }
  };

  return (
    <Card className="bg-muted/40 border-primary/10">
      <CardHeader className="pb-3">
        <div className="flex items-center gap-2">
          <CardTitle className="text-base">Update goal?</CardTitle>
          <Badge variant="secondary" className="text-xs">
            {current.title}
          </Badge>
        </div>
        <p className="text-muted-foreground mt-1 text-xs">
          Review before saving. Nothing changes until you click Confirm.
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

        <div className="space-y-1.5">
          {diff.length === 0 ? (
            <p className="text-muted-foreground text-sm">No fields will change.</p>
          ) : (
            diff.map((row) => <DiffRow key={row.field} row={row} />)
          )}
        </div>

        <div className="flex justify-end gap-2 pt-2">
          <Button
            onClick={handleConfirm}
            disabled={diff.length === 0 || updateMutation.isPending}
            size="sm"
          >
            {updateMutation.isPending ? (
              <>
                <Icons.Spinner className="mr-1.5 size-3.5 animate-spin" />
                Saving…
              </>
            ) : (
              <>
                <Icons.Check className="mr-1.5 size-3.5" />
                Confirm & save
              </>
            )}
          </Button>
        </div>
      </CardContent>
    </Card>
  );
}

// Exported for render-test coverage. Same pattern as
// delete-stock-position-tool-ui.tsx — tests render the impl directly
// without the assistant-ui runtime. The makeAssistantToolUI wrapper
// below still owns the chat-time render path.
export function UpdateGoalToolUIContentImpl({ result, status, toolCallId }: Props) {
  const parsed = useMemo(() => normaliseResult(result), [result]);
  const [submitted, setSubmitted] = useState(false);

  if (status?.type === "running") return <LoadingSkeleton />;
  if (status?.type === "incomplete") {
    return <ErrorCard title="Couldn't prepare the update." body="The agent didn't return a draft." />;
  }
  if (!parsed) {
    return <ErrorCard title="No update data available." body="The tool result wasn't a recognized update_goal payload." />;
  }

  if (parsed.submitted || submitted) {
    return <SuccessState title={parsed.draft.title} />;
  }

  if (!parsed.validation.resolved) {
    return (
      <UnresolvedCard
        candidates={parsed.validation.candidates}
        warnings={parsed.validation.warnings}
      />
    );
  }

  return (
    <DraftCard
      draft={parsed.draft}
      current={parsed.current}
      diff={parsed.diff}
      warnings={parsed.validation.warnings}
      toolCallId={toolCallId}
      onSuccess={() => setSubmitted(true)}
    />
  );
}

const UpdateGoalToolUIContent = memo(UpdateGoalToolUIContentImpl);

export const UpdateGoalToolUI = makeAssistantToolUI<UpdateGoalArgs, UpdateGoalResult>({
  toolName: "update_goal",
  render: (props) => <UpdateGoalToolUIContent {...props} />,
});

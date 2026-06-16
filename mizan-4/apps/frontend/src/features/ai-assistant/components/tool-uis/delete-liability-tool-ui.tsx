import type { ToolCallMessagePartProps } from "@assistant-ui/react";
import { makeAssistantToolUI } from "@assistant-ui/react";
import { Button, Card, CardContent, CardHeader, CardTitle, Skeleton } from "@mizan/ui";
import { Icons } from "@mizan/ui/components/ui/icons";
import { memo, useMemo, useState } from "react";

import { updateToolResult } from "@/adapters";
import { useAssetManagement } from "@/pages/asset/hooks/use-asset-management";

import { useRuntimeContext } from "../../hooks/use-runtime-context";
import { useQueryClient } from "@tanstack/react-query";
import { refreshAfterMutation, unwrapToolResult } from "./shared";

// Types mirror crates/ai/src/tools/delete_liability.rs

interface DeleteLiabilityArgs {
  liabilityRef: string;
  reason?: string;
}

interface LiabilitySnapshot {
  id: string;
  name: string;
  liabilityType: string | null;
  currency: string;
  principal: number | null;
  displayCode: string | null;
}

interface ValidationResult {
  resolved: boolean;
  candidates: LiabilitySnapshot[];
  warnings: string[];
  isValid: boolean;
}

interface DeleteLiabilityOutput {
  target: LiabilitySnapshot;
  reason: string | null;
  validation: ValidationResult;
}

interface DeleteLiabilityResult extends DeleteLiabilityOutput {
  submitted?: boolean;
  deletedAt?: string;
}

type Props = ToolCallMessagePartProps<DeleteLiabilityArgs, DeleteLiabilityResult>;

function normaliseResult(raw: unknown): DeleteLiabilityResult | null {
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
  const obj = unwrapped as Partial<DeleteLiabilityResult>;
  if (!obj.target || !obj.validation) return null;
  return obj as DeleteLiabilityResult;
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

function SuccessState({ target }: { target: LiabilitySnapshot }) {
  return (
    <Card className="border-success/30 bg-success/5">
      <CardHeader className="pb-2">
        <div className="flex items-center gap-2">
          <Icons.CheckCircle className="text-success size-5" />
          <CardTitle className="text-base">Liability deleted</CardTitle>
        </div>
      </CardHeader>
      <CardContent className="text-sm">
        <p>
          <span className="font-medium">{target.name}</span>
          {target.liabilityType && (
            <span className="text-muted-foreground"> · {target.liabilityType}</span>
          )}
          <span className="text-muted-foreground"> · {target.currency}</span>
        </p>
      </CardContent>
    </Card>
  );
}

function UnresolvedCard({
  candidates,
  warnings,
}: {
  candidates: LiabilitySnapshot[];
  warnings: string[];
}) {
  return (
    <Card className="border-warning/30 bg-warning/5">
      <CardHeader className="pb-2">
        <div className="flex items-center gap-2">
          <Icons.AlertTriangle className="text-warning size-5" />
          <CardTitle className="text-base">Couldn't pick a liability</CardTitle>
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
                  <span className="font-medium">{c.name}</span>
                  {c.liabilityType && (
                    <span className="text-muted-foreground"> · {c.liabilityType}</span>
                  )}
                  <span className="text-muted-foreground"> · {c.currency}</span>
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
  reason,
  warnings,
  toolCallId,
  onSuccess,
}: {
  target: LiabilitySnapshot;
  reason: string | null;
  warnings: string[];
  toolCallId: string;
  onSuccess: () => void;
}) {
  const runtime = useRuntimeContext();
  const queryClient = useQueryClient();
  const threadId = runtime.currentThreadId;
  const [armed, setArmed] = useState(false);
  const { deleteAssetMutation } = useAssetManagement();

  const handleConfirm = async () => {
    try {
      await deleteAssetMutation.mutateAsync(target.id);
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
      // Toast handled by useAssetManagement onError.
    }
  };

  return (
    <Card className="border-destructive/30 bg-destructive/5">
      <CardHeader className="pb-3">
        <div className="flex items-center gap-2">
          <Icons.Trash2 className="text-destructive size-4" />
          <CardTitle className="text-base">Delete liability?</CardTitle>
        </div>
        <p className="text-muted-foreground mt-1 text-xs">
          Irreversible. The amortization schedule + payoff history go away with it.
        </p>
      </CardHeader>

      <CardContent className="space-y-3 text-sm">
        <div className="bg-background/50 rounded-md border px-3 py-2">
          <p>
            <span className="font-medium">{target.name}</span>
            {target.liabilityType && (
              <span className="text-muted-foreground"> · {target.liabilityType}</span>
            )}
            <span className="text-muted-foreground"> · {target.currency}</span>
            {typeof target.principal === "number" && (
              <span className="text-muted-foreground">
                {" "}
                · {target.currency} {Math.round(target.principal).toLocaleString()}
              </span>
            )}
          </p>
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
            disabled={!armed || deleteAssetMutation.isPending}
          >
            Cancel
          </Button>
          {armed ? (
            <Button
              variant="destructive"
              size="sm"
              onClick={handleConfirm}
              disabled={deleteAssetMutation.isPending}
            >
              {deleteAssetMutation.isPending ? (
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

function DeleteLiabilityToolUIContentImpl({ result, status, toolCallId }: Props) {
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
      reason={parsed.reason}
      warnings={parsed.validation.warnings}
      toolCallId={toolCallId}
      onSuccess={() => setSubmitted(true)}
    />
  );
}

const DeleteLiabilityToolUIContent = memo(DeleteLiabilityToolUIContentImpl);

export const DeleteLiabilityToolUI = makeAssistantToolUI<
  DeleteLiabilityArgs,
  DeleteLiabilityResult
>({
  toolName: "delete_liability",
  render: (props) => <DeleteLiabilityToolUIContent {...props} />,
});

import type { ToolCallMessagePartProps } from "@assistant-ui/react";
import { makeAssistantToolUI } from "@assistant-ui/react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
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
import { toast } from "@mizan/ui/components/ui/use-toast";
import { memo, useMemo, useState } from "react";

import { deleteActivity, updateToolResult } from "@/adapters";

import { useRuntimeContext } from "../../hooks/use-runtime-context";
import { refreshAfterMutation, unwrapToolResult } from "./shared";

// ============================================================================
// Types (mirror crates/ai/src/tools/delete_stock_position.rs)
// ============================================================================

interface DeleteStockPositionArgs {
  assetRef: string;
  accountRef?: string;
  reason?: string;
}

interface AssetSnapshot {
  id: string;
  symbol: string;
  name: string;
  kind: string;
}

interface PositionImpact {
  accountId: string;
  accountName: string;
  quantity: number;
  marketValueLocal: number;
  marketValueBase: number;
  localCurrency: string;
  activityCount: number;
}

interface ValidationResult {
  resolved: boolean;
  candidates: AssetSnapshot[];
  warnings: string[];
  isValid: boolean;
}

interface DeleteStockPositionOutput {
  target: AssetSnapshot;
  positions: PositionImpact[];
  activityIds: string[];
  totalMarketValueBase: number;
  reason: string | null;
  validation: ValidationResult;
}

interface DeleteStockPositionResult extends DeleteStockPositionOutput {
  submitted?: boolean;
  deletedAt?: string;
}

type Props = ToolCallMessagePartProps<DeleteStockPositionArgs, DeleteStockPositionResult>;

function normaliseResult(raw: unknown): DeleteStockPositionResult | null {
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
  const obj = unwrapped as Partial<DeleteStockPositionResult>;
  if (!obj.target || !obj.validation || !Array.isArray(obj.positions)) return null;
  return obj as DeleteStockPositionResult;
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

function SuccessState({ target, activityCount }: { target: AssetSnapshot; activityCount: number }) {
  return (
    <Card className="border-success/30 bg-success/5">
      <CardHeader className="pb-2">
        <div className="flex items-center gap-2">
          <Icons.CheckCircle className="text-success size-5" />
          <CardTitle className="text-base">Position removed</CardTitle>
        </div>
      </CardHeader>
      <CardContent className="text-sm">
        <p>
          {target.symbol} ({target.name}) — {activityCount} activit
          {activityCount === 1 ? "y" : "ies"} deleted.
        </p>
      </CardContent>
    </Card>
  );
}

function UnresolvedCard({
  candidates,
  warnings,
}: {
  candidates: AssetSnapshot[];
  warnings: string[];
}) {
  return (
    <Card className="border-warning/30 bg-warning/5">
      <CardHeader className="pb-2">
        <div className="flex items-center gap-2">
          <Icons.AlertTriangle className="text-warning size-5" />
          <CardTitle className="text-base">Couldn't pick a stock</CardTitle>
        </div>
      </CardHeader>
      <CardContent className="space-y-3 text-sm">
        {warnings.map((w, i) => (
          <p key={i} className="text-muted-foreground text-xs">
            {w}
          </p>
        ))}
        {candidates.length > 0 && (
          <ul className="space-y-1 text-xs">
            {candidates.map((c) => (
              <li key={c.id}>
                <span className="font-medium">{c.symbol}</span>
                <span className="text-muted-foreground"> · {c.name}</span>
              </li>
            ))}
          </ul>
        )}
      </CardContent>
    </Card>
  );
}

function useDeleteStockPositionMutation() {
  return useMutation({
    mutationFn: async (activityIds: string[]) => {
      // Newest-first cascade — already sorted by the backend tool so
      // the running cost-basis math degrades gracefully if one delete
      // mid-chain fails. We don't run them in parallel: a Promise.all
      // would race on the activities table's per-account locks and a
      // single partial failure would orphan the rest. Sequential keeps
      // the diagnostics clear. Cache invalidation runs at the call
      // site via refreshAfterMutation, so we don't pass queryClient
      // into the mutation itself.
      for (const id of activityIds) {
        await deleteActivity(id);
      }
    },
    onSuccess: () => {
      toast({ title: "Position removed.", variant: "success" });
    },
    onError: (e: unknown) => {
      const msg = e instanceof Error ? e.message : "Failed to remove position.";
      toast({ title: msg, variant: "destructive" });
    },
  });
}

function ConfirmCard({
  target,
  positions,
  activityIds,
  totalMarketValueBase,
  warnings,
  toolCallId,
  onSuccess,
}: {
  target: AssetSnapshot;
  positions: PositionImpact[];
  activityIds: string[];
  totalMarketValueBase: number;
  warnings: string[];
  toolCallId: string;
  onSuccess: () => void;
}) {
  const runtime = useRuntimeContext();
  const queryClient = useQueryClient();
  const threadId = runtime.currentThreadId;
  const mutation = useDeleteStockPositionMutation();
  const totalQty = positions.reduce((acc, p) => acc + p.quantity, 0);

  const handleConfirm = async () => {
    try {
      await mutation.mutateAsync(activityIds);
      // refreshAfterMutation hits invalidateQueries() with no key, which
      // flushes the entire cache — every dashboard surface re-fetches.
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
      // Toast handled in mutation onError.
    }
  };

  return (
    <Card className="border-destructive/30 bg-destructive/5">
      <CardHeader className="pb-3">
        <div className="flex items-center gap-2">
          <Icons.Trash2 className="text-destructive size-4" />
          <CardTitle className="text-base">Remove position?</CardTitle>
          <Badge variant="secondary" className="text-xs">
            {target.symbol}
          </Badge>
        </div>
        <p className="text-muted-foreground mt-1 text-xs">
          {target.name} — {totalQty.toLocaleString()} share
          {totalQty === 1 ? "" : "s"} totalling ~$
          {totalMarketValueBase.toLocaleString(undefined, { maximumFractionDigits: 0 })} across{" "}
          {positions.length} account{positions.length === 1 ? "" : "s"}.
        </p>
      </CardHeader>

      <CardContent className="space-y-3 text-sm">
        <div className="bg-background/50 rounded-md border px-3 py-2 space-y-1">
          {positions.map((p) => (
            <div key={p.accountId} className="flex items-baseline justify-between text-xs">
              <span className="font-medium">{p.accountName}</span>
              <span className="text-muted-foreground">
                {p.quantity.toLocaleString()} sh · {p.localCurrency}{" "}
                {p.marketValueLocal.toLocaleString(undefined, { maximumFractionDigits: 0 })} · {p.activityCount} activit
                {p.activityCount === 1 ? "y" : "ies"}
              </span>
            </div>
          ))}
        </div>

        {warnings.length > 0 && (
          <div className="border-warning/40 bg-warning/10 text-warning rounded-md border px-3 py-2 text-xs">
            {warnings.map((w, i) => (
              <div key={i}>{w}</div>
            ))}
          </div>
        )}

        <div className="flex justify-end gap-2 pt-2">
          <Button
            onClick={handleConfirm}
            disabled={activityIds.length === 0 || mutation.isPending}
            size="sm"
            variant="destructive"
          >
            {mutation.isPending ? (
              <>
                <Icons.Spinner className="mr-1.5 size-3.5 animate-spin" />
                Deleting…
              </>
            ) : (
              <>
                <Icons.Trash2 className="mr-1.5 size-3.5" />
                Confirm & delete
              </>
            )}
          </Button>
        </div>
      </CardContent>
    </Card>
  );
}

function DeleteStockPositionToolUIContentImpl({ result, status, toolCallId }: Props) {
  const parsed = useMemo(() => normaliseResult(result), [result]);
  const [submitted, setSubmitted] = useState(false);

  if (status?.type === "running") return <LoadingSkeleton />;
  if (status?.type === "incomplete") {
    return <ErrorCard title="Couldn't prepare the removal." body="The tool didn't return a draft." />;
  }
  if (!parsed) {
    return <ErrorCard title="No removal data available." body="Tool result wasn't recognised." />;
  }
  if (parsed.submitted || submitted) {
    return <SuccessState target={parsed.target} activityCount={parsed.activityIds.length} />;
  }
  if (!parsed.validation.resolved) {
    return (
      <UnresolvedCard
        candidates={parsed.validation.candidates}
        warnings={parsed.validation.warnings}
      />
    );
  }
  if (parsed.positions.length === 0) {
    return (
      <ErrorCard
        title="Nothing to remove."
        body={parsed.validation.warnings[0] ?? "No open positions matched."}
      />
    );
  }

  return (
    <ConfirmCard
      target={parsed.target}
      positions={parsed.positions}
      activityIds={parsed.activityIds}
      totalMarketValueBase={parsed.totalMarketValueBase}
      warnings={parsed.validation.warnings}
      toolCallId={toolCallId}
      onSuccess={() => setSubmitted(true)}
    />
  );
}

const DeleteStockPositionToolUIContent = memo(DeleteStockPositionToolUIContentImpl);

export const DeleteStockPositionToolUI = makeAssistantToolUI<
  DeleteStockPositionArgs,
  DeleteStockPositionResult
>({
  toolName: "delete_stock_position",
  render: (props) => <DeleteStockPositionToolUIContent {...props} />,
});

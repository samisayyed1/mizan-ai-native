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

import {
  updateAlternativeAssetMetadata,
  updateAlternativeAssetValuation,
  updateToolResult,
} from "@/adapters";
import { QueryKeys } from "@/lib/query-keys";

import { useRuntimeContext } from "../../hooks/use-runtime-context";
import { refreshAfterMutation, unwrapToolResult } from "./shared";

// ============================================================================
// Types (mirror crates/ai/src/tools/update_liability.rs)
// ============================================================================

interface UpdateLiabilityArgs {
  assetId: string;
  name?: string;
  principal?: number;
  ratePct?: number;
  monthlyPayment?: number;
  originatedAt?: string;
  liabilityType?: string;
  notes?: string;
}

interface LiabilityUpdateDraft {
  assetId: string;
  name: string | null;
  principal: number | null;
  ratePct: number | null;
  monthlyPayment: number | null;
  originatedAt: string | null;
  liabilityType: string | null;
  notes: string | null;
}

interface ValidationResult {
  isValid: boolean;
  missingFields: string[];
  warnings: string[];
  hasChanges: boolean;
}

interface UpdateLiabilityOutput {
  draft: LiabilityUpdateDraft;
  validation: ValidationResult;
  availableSubtypes: { value: string; label: string }[];
}

interface UpdateLiabilityResult extends UpdateLiabilityOutput {
  submitted?: boolean;
}

type Props = ToolCallMessagePartProps<UpdateLiabilityArgs, UpdateLiabilityResult>;

function normaliseResult(raw: unknown): UpdateLiabilityResult | null {
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
  const obj = unwrapped as Partial<UpdateLiabilityResult>;
  if (!obj.draft || !obj.validation) return null;
  return obj as UpdateLiabilityResult;
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

function SuccessState() {
  return (
    <Card className="border-success/30 bg-success/5">
      <CardHeader className="pb-2">
        <div className="flex items-center gap-2">
          <Icons.CheckCircle className="text-success size-5" />
          <CardTitle className="text-base">Liability updated</CardTitle>
        </div>
      </CardHeader>
    </Card>
  );
}

function DiffRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-baseline gap-3 text-sm">
      <span className="text-muted-foreground w-32 shrink-0 text-xs uppercase tracking-wide">
        {label}
      </span>
      <span className="font-medium">{value}</span>
    </div>
  );
}

function buildDiffRows(draft: LiabilityUpdateDraft): { label: string; value: string }[] {
  const out: { label: string; value: string }[] = [];
  if (draft.name !== null) out.push({ label: "Name", value: draft.name || "(clear)" });
  if (draft.liabilityType !== null) {
    out.push({ label: "Type", value: draft.liabilityType });
  }
  if (draft.principal !== null) {
    out.push({ label: "Principal", value: draft.principal.toLocaleString() });
  }
  if (draft.ratePct !== null) out.push({ label: "Rate", value: `${draft.ratePct}%` });
  if (draft.monthlyPayment !== null) {
    out.push({
      label: "Monthly payment",
      value: draft.monthlyPayment.toLocaleString(),
    });
  }
  if (draft.originatedAt !== null) {
    out.push({ label: "Originated", value: draft.originatedAt });
  }
  if (draft.notes !== null) out.push({ label: "Notes", value: draft.notes || "(clear)" });
  return out;
}

function useUpdateLiabilityMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (draft: LiabilityUpdateDraft) => {
      const metadataPatch: Record<string, string> = {};
      if (draft.liabilityType !== null) metadataPatch.sub_type = draft.liabilityType;
      if (draft.ratePct !== null) metadataPatch.rate_pct = String(draft.ratePct);
      if (draft.monthlyPayment !== null) {
        metadataPatch.monthly_payment = String(draft.monthlyPayment);
      }
      if (draft.originatedAt !== null) metadataPatch.originated_at = draft.originatedAt;
      // Strip the "example" flag on first edit — the row stops being an Example.
      metadataPatch.example = "";

      const willPatchMetadata =
        Object.keys(metadataPatch).length > 0 || draft.name !== null || draft.notes !== null;

      if (willPatchMetadata) {
        await updateAlternativeAssetMetadata(
          draft.assetId,
          metadataPatch,
          draft.name !== null
            ? draft.name.replace(/^Example — /, "")
            : undefined,
          draft.notes !== null ? draft.notes : undefined,
        );
      }

      if (draft.principal !== null) {
        await updateAlternativeAssetValuation(draft.assetId, {
          value: String(draft.principal),
          date: new Date().toISOString().slice(0, 10),
          notes: undefined,
        });
      }
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [QueryKeys.HOLDINGS] });
      queryClient.invalidateQueries({ queryKey: [QueryKeys.ALTERNATIVE_HOLDINGS] });
      queryClient.invalidateQueries({ queryKey: [QueryKeys.NET_WORTH] });
      toast({ title: "Liability updated.", variant: "success" });
    },
    onError: (e: unknown) => {
      const msg = e instanceof Error ? e.message : "Failed to update liability.";
      toast({ title: msg, variant: "destructive" });
    },
  });
}

function DraftCard({
  draft,
  warnings,
  toolCallId,
  onSuccess,
}: {
  draft: LiabilityUpdateDraft;
  warnings: string[];
  toolCallId: string;
  onSuccess: () => void;
}) {
  const runtime = useRuntimeContext();
  const queryClient = useQueryClient();
  const threadId = runtime.currentThreadId;
  const mutation = useUpdateLiabilityMutation();

  const rows = useMemo(() => buildDiffRows(draft), [draft]);

  const handleConfirm = async () => {
    try {
      await mutation.mutateAsync(draft);
      refreshAfterMutation(queryClient);
      if (threadId) {
        try {
          await updateToolResult({
            threadId,
            toolCallId,
            resultPatch: { submitted: true },
          });
        } catch (e) {
          console.error("Failed to persist tool result:", e);
        }
      }
      onSuccess();
    } catch {
      // toast handled
    }
  };

  return (
    <Card className="bg-muted/40 border-primary/10">
      <CardHeader className="pb-3">
        <div className="flex items-center gap-2">
          <CardTitle className="text-base">Update liability?</CardTitle>
          <Badge variant="secondary" className="text-xs">
            {draft.assetId}
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
          {rows.length === 0 ? (
            <p className="text-muted-foreground text-sm">No fields will change.</p>
          ) : (
            rows.map((r) => <DiffRow key={r.label} {...r} />)
          )}
        </div>

        <div className="flex justify-end gap-2 pt-2">
          <Button
            onClick={handleConfirm}
            disabled={rows.length === 0 || mutation.isPending}
            size="sm"
          >
            {mutation.isPending ? (
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

function UpdateLiabilityToolUIContentImpl({ result, status, toolCallId }: Props) {
  const parsed = useMemo(() => normaliseResult(result), [result]);
  const [submitted, setSubmitted] = useState(false);

  if (status?.type === "running") return <LoadingSkeleton />;
  if (status?.type === "incomplete") {
    return (
      <Card className="border-destructive/30 bg-destructive/5">
        <CardContent className="py-4">
          <p className="text-destructive text-sm font-medium">
            Couldn't prepare the update.
          </p>
        </CardContent>
      </Card>
    );
  }
  if (!parsed) {
    return (
      <Card className="border-destructive/30 bg-destructive/5">
        <CardContent className="py-4">
          <p className="text-destructive text-sm font-medium">No update data available.</p>
        </CardContent>
      </Card>
    );
  }

  if (parsed.submitted || submitted) return <SuccessState />;

  return (
    <DraftCard
      draft={parsed.draft}
      warnings={parsed.validation.warnings}
      toolCallId={toolCallId}
      onSuccess={() => setSubmitted(true)}
    />
  );
}

const UpdateLiabilityToolUIContent = memo(UpdateLiabilityToolUIContentImpl);

export const UpdateLiabilityToolUI = makeAssistantToolUI<
  UpdateLiabilityArgs,
  UpdateLiabilityResult
>({
  toolName: "update_liability",
  render: (props) => <UpdateLiabilityToolUIContent {...props} />,
});

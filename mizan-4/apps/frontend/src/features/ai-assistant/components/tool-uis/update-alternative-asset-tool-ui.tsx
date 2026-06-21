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
// Types (mirror crates/ai/src/tools/update_alternative_asset.rs)
// ============================================================================

interface UpdateAlternativeAssetArgs {
  assetRef: string;
  name?: string;
  notes?: string;
  currentValue?: number;
  metadata?: Record<string, unknown>;
}

interface AssetSnapshot {
  id: string;
  name: string;
  kind: string;
  currency: string;
  currentValue: number | null;
  notes: string | null;
}

interface AssetUpdateDraft {
  id: string;
  name: string;
  kind: string;
  currency: string;
  currentValue: number | null;
  notes: string | null;
  metadataPatch: Record<string, unknown>;
}

interface FieldDiff {
  field: string;
  old: string | null;
  new: string | null;
}

interface ValidationResult {
  resolved: boolean;
  candidates: AssetSnapshot[];
  warnings: string[];
  isValid: boolean;
  hasChanges: boolean;
}

interface UpdateAlternativeAssetOutput {
  draft: AssetUpdateDraft;
  current: AssetSnapshot;
  diff: FieldDiff[];
  validation: ValidationResult;
}

interface UpdateAlternativeAssetResult extends UpdateAlternativeAssetOutput {
  submitted?: boolean;
  updatedAt?: string;
}

type Props = ToolCallMessagePartProps<UpdateAlternativeAssetArgs, UpdateAlternativeAssetResult>;

function normaliseResult(raw: unknown): UpdateAlternativeAssetResult | null {
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
  const obj = unwrapped as Partial<UpdateAlternativeAssetResult>;
  if (!obj.draft || !obj.current || !obj.validation) return null;
  return obj as UpdateAlternativeAssetResult;
}

function formatField(field: string): string {
  if (field.startsWith("metadata.")) {
    const key = field.slice("metadata.".length);
    return key.split("_").map((s) => s.charAt(0).toUpperCase() + s.slice(1)).join(" ");
  }
  switch (field) {
    case "name":
      return "Name";
    case "notes":
      return "Notes";
    case "currentValue":
      return "Current value";
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

function SuccessState({ name }: { name: string }) {
  return (
    <Card className="border-success/30 bg-success/5">
      <CardHeader className="pb-2">
        <div className="flex items-center gap-2">
          <Icons.CheckCircle className="text-success size-5" />
          <CardTitle className="text-base">Asset updated</CardTitle>
        </div>
      </CardHeader>
      <CardContent className="text-sm">{name}</CardContent>
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
          <CardTitle className="text-base">Couldn't pick an asset</CardTitle>
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
                <span className="font-medium">{c.name}</span>
                <span className="text-muted-foreground"> · {c.kind}</span>
              </li>
            ))}
          </ul>
        )}
      </CardContent>
    </Card>
  );
}

function DiffRow({ row }: { row: FieldDiff }) {
  return (
    <div className="flex items-baseline gap-3 text-sm">
      <span className="text-muted-foreground w-36 shrink-0 text-xs uppercase tracking-wide">
        {formatField(row.field)}
      </span>
      <span className="text-muted-foreground line-through text-xs">{row.old ?? "—"}</span>
      <Icons.ArrowRight className="text-muted-foreground size-3" />
      <span className="font-medium">{row.new ?? "(clear)"}</span>
    </div>
  );
}

function useUpdateAlternativeAssetMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (draft: AssetUpdateDraft) => {
      // Metadata patch: backend expects HashMap<String, Option<String>>,
      // so stringify each non-null value and pass null through verbatim.
      const metadataPatch: Record<string, string> = {};
      for (const [key, value] of Object.entries(draft.metadataPatch)) {
        if (value === null || value === undefined) {
          metadataPatch[key] = ""; // backend clears empty-string keys
        } else if (typeof value === "string") {
          metadataPatch[key] = value;
        } else {
          metadataPatch[key] = String(value);
        }
      }

      const willPatchMetadata =
        Object.keys(metadataPatch).length > 0 || draft.notes !== null;

      if (willPatchMetadata) {
        await updateAlternativeAssetMetadata(
          draft.id,
          metadataPatch,
          undefined, // name handled separately below if present
          draft.notes ?? undefined,
        );
      }

      // The metadata endpoint accepts a name. If the only change is the
      // name, still go through metadata so it sticks.
      if (draft.name) {
        await updateAlternativeAssetMetadata(
          draft.id,
          {},
          draft.name,
          undefined,
        );
      }

      if (draft.currentValue !== null) {
        await updateAlternativeAssetValuation(draft.id, {
          value: String(draft.currentValue),
          date: new Date().toISOString().slice(0, 10),
          notes: undefined,
        });
      }
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [QueryKeys.HOLDINGS] });
      queryClient.invalidateQueries({ queryKey: [QueryKeys.ALTERNATIVE_HOLDINGS] });
      queryClient.invalidateQueries({ queryKey: [QueryKeys.NET_WORTH] });
      toast({ title: "Asset updated.", variant: "success" });
    },
    onError: (e: unknown) => {
      const msg = e instanceof Error ? e.message : "Failed to update asset.";
      toast({ title: msg, variant: "destructive" });
    },
  });
}

function DraftCard({
  draft,
  current,
  diff,
  warnings,
  toolCallId,
  onSuccess,
}: {
  draft: AssetUpdateDraft;
  current: AssetSnapshot;
  diff: FieldDiff[];
  warnings: string[];
  toolCallId: string;
  onSuccess: () => void;
}) {
  const runtime = useRuntimeContext();
  const queryClient = useQueryClient();
  const threadId = runtime.currentThreadId;
  const mutation = useUpdateAlternativeAssetMutation();

  const handleConfirm = async () => {
    try {
      await mutation.mutateAsync(draft);
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
      // toast handled
    }
  };

  return (
    <Card className="bg-muted/40 border-primary/10">
      <CardHeader className="pb-3">
        <div className="flex items-center gap-2">
          <CardTitle className="text-base">Update asset?</CardTitle>
          <Badge variant="secondary" className="text-xs">
            {current.kind}
          </Badge>
          <Badge variant="outline" className="text-xs">
            {current.name}
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
            disabled={diff.length === 0 || mutation.isPending}
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

function UpdateAlternativeAssetToolUIContentImpl({ result, status, toolCallId }: Props) {
  const parsed = useMemo(() => normaliseResult(result), [result]);
  const [submitted, setSubmitted] = useState(false);

  if (status?.type === "running") return <LoadingSkeleton />;
  if (status?.type === "incomplete") {
    return <ErrorCard title="Couldn't prepare the update." body="The agent didn't return a draft." />;
  }
  if (!parsed) {
    return <ErrorCard title="No update data available." body="Tool result wasn't recognized." />;
  }

  if (parsed.submitted || submitted) return <SuccessState name={parsed.draft.name} />;

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

const UpdateAlternativeAssetToolUIContent = memo(UpdateAlternativeAssetToolUIContentImpl);

export const UpdateAlternativeAssetToolUI = makeAssistantToolUI<
  UpdateAlternativeAssetArgs,
  UpdateAlternativeAssetResult
>({
  toolName: "update_alternative_asset",
  render: (props) => <UpdateAlternativeAssetToolUIContent {...props} />,
});

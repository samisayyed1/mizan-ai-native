import type { ToolCallMessagePartProps } from "@assistant-ui/react";
import { makeAssistantToolUI } from "@assistant-ui/react";
import { Badge, Button, Card, CardContent, CardHeader, CardTitle, Skeleton } from "@mizan/ui";
import { Icons } from "@mizan/ui/components/ui/icons";
import { memo, useMemo, useState } from "react";

import { updateToolResult } from "@/adapters";
import { useAssetManagement } from "@/pages/asset/hooks/use-asset-management";

import { useRuntimeContext } from "../../hooks/use-runtime-context";

// Types mirror crates/ai/src/tools/delete_alternative_asset.rs

interface DeleteAlternativeAssetArgs {
  assetRef: string;
  reason?: string;
}

interface AssetSnapshot {
  id: string;
  name: string;
  kind: string;
  displayCode: string | null;
  currency: string;
}

interface ValidationResult {
  resolved: boolean;
  candidates: AssetSnapshot[];
  warnings: string[];
  isValid: boolean;
}

interface DeleteAlternativeAssetOutput {
  target: AssetSnapshot;
  reason: string | null;
  validation: ValidationResult;
}

interface DeleteAlternativeAssetResult extends DeleteAlternativeAssetOutput {
  submitted?: boolean;
  deletedAt?: string;
}

type Props = ToolCallMessagePartProps<DeleteAlternativeAssetArgs, DeleteAlternativeAssetResult>;

function normaliseResult(raw: unknown): DeleteAlternativeAssetResult | null {
  if (!raw) return null;
  if (typeof raw === "string") {
    try {
      return normaliseResult(JSON.parse(raw));
    } catch {
      return null;
    }
  }
  if (typeof raw !== "object") return null;
  const obj = raw as Partial<DeleteAlternativeAssetResult>;
  if (!obj.target || !obj.validation) return null;
  return obj as DeleteAlternativeAssetResult;
}

function kindBadge(kind: string): string {
  switch (kind) {
    case "PROPERTY":
      return "Property";
    case "VEHICLE":
      return "Vehicle";
    case "COLLECTIBLE":
      return "Collectible";
    case "PRECIOUS_METAL":
      return "Precious metal";
    case "PRIVATE_EQUITY":
      return "Private equity";
    default:
      return kind;
  }
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

function SuccessState({ target }: { target: AssetSnapshot }) {
  return (
    <Card className="border-success/30 bg-success/5">
      <CardHeader className="pb-2">
        <div className="flex items-center gap-2">
          <Icons.CheckCircle className="text-success size-5" />
          <CardTitle className="text-base">Asset deleted</CardTitle>
        </div>
      </CardHeader>
      <CardContent className="text-sm">
        <p>
          <span className="font-medium">{target.name}</span>
          <span className="text-muted-foreground"> · {kindBadge(target.kind)}</span>
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
          <div className="space-y-1">
            <p className="text-xs font-medium uppercase tracking-wide">Possible matches</p>
            <ul className="space-y-1">
              {candidates.map((c) => (
                <li key={c.id} className="text-xs">
                  <span className="font-medium">{c.name}</span>
                  <span className="text-muted-foreground"> · {kindBadge(c.kind)}</span>
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
  target: AssetSnapshot;
  reason: string | null;
  warnings: string[];
  toolCallId: string;
  onSuccess: () => void;
}) {
  const runtime = useRuntimeContext();
  const threadId = runtime.currentThreadId;
  const [armed, setArmed] = useState(false);
  const { deleteAssetMutation } = useAssetManagement();

  const handleConfirm = async () => {
    try {
      await deleteAssetMutation.mutateAsync(target.id);
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
          <CardTitle className="text-base">Delete asset?</CardTitle>
          <Badge variant="secondary" className="text-xs">
            {kindBadge(target.kind)}
          </Badge>
        </div>
        <p className="text-muted-foreground mt-1 text-xs">
          Irreversible. Activities tied to this asset cascade-delete with it.
        </p>
      </CardHeader>

      <CardContent className="space-y-3 text-sm">
        <div className="bg-background/50 rounded-md border px-3 py-2">
          <p>
            <span className="font-medium">{target.name}</span>
            {target.displayCode && (
              <span className="text-muted-foreground"> · {target.displayCode}</span>
            )}
            <span className="text-muted-foreground"> · {target.currency}</span>
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

function DeleteAlternativeAssetToolUIContentImpl({ result, status, toolCallId }: Props) {
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

const DeleteAlternativeAssetToolUIContent = memo(DeleteAlternativeAssetToolUIContentImpl);

export const DeleteAlternativeAssetToolUI = makeAssistantToolUI<
  DeleteAlternativeAssetArgs,
  DeleteAlternativeAssetResult
>({
  toolName: "delete_alternative_asset",
  render: (props) => <DeleteAlternativeAssetToolUIContent {...props} />,
});

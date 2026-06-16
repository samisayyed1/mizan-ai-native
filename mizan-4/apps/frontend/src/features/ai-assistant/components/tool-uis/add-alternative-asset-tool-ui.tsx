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
import { toast } from "@mizan/ui/components/ui/use-toast";
import { memo, useMemo, useState } from "react";

import { createAlternativeAsset, updateToolResult } from "@/adapters";
import { QueryKeys } from "@/lib/query-keys";
import type { AlternativeAssetKindApi } from "@/lib/types";

import { unwrapToolResult } from "./shared";

import { useRuntimeContext } from "../../hooks/use-runtime-context";

// ============================================================================
// Types (mirror crates/ai/src/tools/add_alternative_asset.rs)
// ============================================================================

interface AddAlternativeAssetArgs {
  kind: string;
  name: string;
  currentValue: number;
  currency?: string;
  purchasePrice?: number;
  purchaseDate?: string;
  valueDate?: string;
  metadata?: Record<string, string>;
  notes?: string;
}

interface AlternativeAssetDraft {
  kind: AlternativeAssetKindApi;
  name: string;
  currentValue: number;
  currency: string;
  purchasePrice: number | null;
  purchaseDate: string | null;
  valueDate: string;
  metadata: Record<string, string>;
  notes: string | null;
}

interface ValidationResult {
  isValid: boolean;
  missingFields: string[];
  warnings: string[];
}

interface KindOption {
  value: AlternativeAssetKindApi;
  label: string;
}

interface AddAlternativeAssetOutput {
  draft: AlternativeAssetDraft;
  validation: ValidationResult;
  availableKinds: KindOption[];
  availableCurrencies: string[];
}

interface AddAlternativeAssetResult extends AddAlternativeAssetOutput {
  submitted?: boolean;
  createdAssetId?: string;
}

type Props = ToolCallMessagePartProps<AddAlternativeAssetArgs, AddAlternativeAssetResult>;

function normaliseResult(raw: unknown): AddAlternativeAssetResult | null {
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
  const obj = unwrapped as Partial<AddAlternativeAssetResult>;
  if (!obj.draft || typeof obj.draft !== "object") return null;
  return obj as AddAlternativeAssetResult;
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
        <Skeleton className="h-9 w-28" />
      </CardContent>
    </Card>
  );
}

function SuccessState({ draft }: { draft: AlternativeAssetDraft }) {
  return (
    <Card className="border-success/30 bg-success/5">
      <CardHeader className="pb-2">
        <div className="flex items-center gap-2">
          <Icons.CheckCircle className="text-success size-5" />
          <CardTitle className="text-base">Asset added</CardTitle>
        </div>
      </CardHeader>
      <CardContent className="text-sm">
        <p>
          <span className="font-medium">{draft.name}</span>
          <span className="text-muted-foreground"> · {draft.kind}</span>
          <span className="text-muted-foreground">
            {" "}
            · {draft.currency} {draft.currentValue.toLocaleString()}
          </span>
        </p>
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

function MetadataReview({ metadata }: { metadata: Record<string, string> }) {
  const entries = Object.entries(metadata).filter(([, v]) => v && v.trim().length > 0);
  if (entries.length === 0) return null;
  return (
    <div className="col-span-full">
      <dt className="text-muted-foreground text-xs uppercase tracking-wide">Metadata</dt>
      <dd className="mt-1 grid grid-cols-1 gap-1 text-xs sm:grid-cols-2">
        {entries.map(([k, v]) => (
          <div key={k}>
            <span className="text-muted-foreground">{k}:</span>{" "}
            <span className="font-medium">{v}</span>
          </div>
        ))}
      </dd>
    </div>
  );
}

function useAddAltAssetMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: createAlternativeAsset,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [QueryKeys.HOLDINGS] });
      queryClient.invalidateQueries({ queryKey: [QueryKeys.ALTERNATIVE_HOLDINGS] });
      queryClient.invalidateQueries({ queryKey: [QueryKeys.NET_WORTH] });
      toast({ title: "Asset added.", variant: "success" });
    },
    onError: (e: unknown) => {
      const msg = e instanceof Error ? e.message : "Failed to add asset.";
      toast({ title: msg, variant: "destructive" });
    },
  });
}

function DraftCard({
  initialDraft,
  warnings,
  availableKinds,
  availableCurrencies,
  toolCallId,
  onSuccess,
}: {
  initialDraft: AlternativeAssetDraft;
  warnings: string[];
  availableKinds: KindOption[];
  availableCurrencies: string[];
  toolCallId: string;
  onSuccess: (assetId: string) => void;
}) {
  const runtime = useRuntimeContext();
  const threadId = runtime.currentThreadId;

  const [isEditing, setIsEditing] = useState(false);
  const [draft, setDraft] = useState<AlternativeAssetDraft>(initialDraft);

  const mutation = useAddAltAssetMutation();

  const canConfirm =
    draft.currentValue > 0 &&
    draft.name.trim().length >= 2 &&
    !!draft.kind &&
    !mutation.isPending;

  const handleConfirm = async () => {
    try {
      const created = await mutation.mutateAsync({
        kind: draft.kind,
        name: draft.name.trim(),
        currency: draft.currency,
        currentValue: String(draft.currentValue),
        valueDate: draft.valueDate,
        purchasePrice:
          draft.purchasePrice !== null ? String(draft.purchasePrice) : undefined,
        purchaseDate: draft.purchaseDate ?? undefined,
        metadata: { ...draft.metadata, ...(draft.notes ? { notes: draft.notes } : {}) },
      });

      if (threadId) {
        try {
          await updateToolResult({
            threadId,
            toolCallId,
            resultPatch: {
              submitted: true,
              createdAssetId: created.assetId,
            },
          });
        } catch (e) {
          console.error("Failed to persist tool result:", e);
        }
      }
      onSuccess(created.assetId);
    } catch {
      // toast handled
    }
  };

  return (
    <Card className="bg-muted/40 border-primary/10">
      <CardHeader className="pb-3">
        <div className="flex items-center gap-2">
          <CardTitle className="text-base">
            {isEditing ? "Edit asset draft" : "Add asset?"}
          </CardTitle>
          <Badge variant="secondary" className="text-xs">
            {availableKinds.find((k) => k.value === draft.kind)?.label ?? draft.kind}
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
              <Label htmlFor="ai-add-alt-name">Name</Label>
              <Input
                id="ai-add-alt-name"
                value={draft.name}
                onChange={(e) => setDraft({ ...draft, name: e.target.value })}
                className="mt-1.5"
              />
            </div>
            <div>
              <Label htmlFor="ai-add-alt-kind">Kind</Label>
              <Select
                value={draft.kind}
                onValueChange={(v) =>
                  setDraft({ ...draft, kind: v as AlternativeAssetKindApi })
                }
              >
                <SelectTrigger id="ai-add-alt-kind" className="mt-1.5">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {availableKinds.map((k) => (
                    <SelectItem key={k.value} value={k.value}>
                      {k.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            <div>
              <Label htmlFor="ai-add-alt-currency">Currency</Label>
              <Select
                value={draft.currency}
                onValueChange={(v) => setDraft({ ...draft, currency: v })}
              >
                <SelectTrigger id="ai-add-alt-currency" className="mt-1.5">
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
              <Label htmlFor="ai-add-alt-value">Current value</Label>
              <Input
                id="ai-add-alt-value"
                type="number"
                inputMode="decimal"
                value={draft.currentValue}
                onChange={(e) =>
                  setDraft({ ...draft, currentValue: Number(e.target.value) || 0 })
                }
                className="mt-1.5"
              />
            </div>
            <div>
              <Label htmlFor="ai-add-alt-pprice">Purchase price (optional)</Label>
              <Input
                id="ai-add-alt-pprice"
                type="number"
                inputMode="decimal"
                value={draft.purchasePrice ?? ""}
                onChange={(e) =>
                  setDraft({
                    ...draft,
                    purchasePrice: e.target.value === "" ? null : Number(e.target.value),
                  })
                }
                className="mt-1.5"
              />
            </div>
            <div>
              <Label htmlFor="ai-add-alt-pdate">Purchase date (optional)</Label>
              <Input
                id="ai-add-alt-pdate"
                type="date"
                value={draft.purchaseDate ?? ""}
                onChange={(e) =>
                  setDraft({
                    ...draft,
                    purchaseDate: e.target.value ? e.target.value : null,
                  })
                }
                className="mt-1.5"
              />
            </div>
          </div>
        ) : (
          <dl className="grid grid-cols-1 gap-x-6 gap-y-2 text-sm sm:grid-cols-2">
            <ReviewRow label="Name" value={draft.name} />
            <ReviewRow
              label="Kind"
              value={availableKinds.find((k) => k.value === draft.kind)?.label ?? draft.kind}
            />
            <ReviewRow
              label="Value"
              value={`${draft.currency} ${draft.currentValue.toLocaleString()}`}
            />
            <ReviewRow
              label="Purchase price"
              value={
                draft.purchasePrice !== null
                  ? `${draft.currency} ${draft.purchasePrice.toLocaleString()}`
                  : "—"
              }
            />
            <ReviewRow label="Purchase date" value={draft.purchaseDate ?? "—"} />
            <ReviewRow label="Value date" value={draft.valueDate} />
            <MetadataReview metadata={draft.metadata} />
            {draft.notes && (
              <div className="col-span-full">
                <dt className="text-muted-foreground text-xs uppercase tracking-wide">Notes</dt>
                <dd className="mt-0.5">{draft.notes}</dd>
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
            {mutation.isPending ? (
              <>
                <Icons.Spinner className="mr-1.5 size-3.5 animate-spin" />
                Saving…
              </>
            ) : (
              <>
                <Icons.Check className="mr-1.5 size-3.5" />
                Confirm & add
              </>
            )}
          </Button>
        </div>
      </CardContent>
    </Card>
  );
}

function AddAlternativeAssetToolUIContentImpl({ result, status, toolCallId }: Props) {
  const parsed = useMemo(() => normaliseResult(result), [result]);
  const [submitted, setSubmitted] = useState(false);

  if (status?.type === "running") return <LoadingSkeleton />;
  if (status?.type === "incomplete") {
    return (
      <Card className="border-destructive/30 bg-destructive/5">
        <CardContent className="py-4">
          <p className="text-destructive text-sm font-medium">
            Couldn't prepare the asset draft.
          </p>
        </CardContent>
      </Card>
    );
  }
  if (!parsed) {
    return (
      <Card className="border-destructive/30 bg-destructive/5">
        <CardContent className="py-4">
          <p className="text-destructive text-sm font-medium">No asset data.</p>
        </CardContent>
      </Card>
    );
  }

  if (parsed.submitted || submitted) {
    return <SuccessState draft={parsed.draft} />;
  }

  return (
    <DraftCard
      initialDraft={parsed.draft}
      warnings={parsed.validation.warnings}
      availableKinds={parsed.availableKinds}
      availableCurrencies={parsed.availableCurrencies}
      toolCallId={toolCallId}
      onSuccess={() => setSubmitted(true)}
    />
  );
}

const AddAlternativeAssetToolUIContent = memo(AddAlternativeAssetToolUIContentImpl);

export const AddAlternativeAssetToolUI = makeAssistantToolUI<
  AddAlternativeAssetArgs,
  AddAlternativeAssetResult
>({
  toolName: "add_alternative_asset",
  render: (props) => <AddAlternativeAssetToolUIContent {...props} />,
});

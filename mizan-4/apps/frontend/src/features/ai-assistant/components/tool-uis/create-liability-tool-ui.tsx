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

import { useRuntimeContext } from "../../hooks/use-runtime-context";
import { refreshAfterMutation, unwrapToolResult } from "./shared";

// ============================================================================
// Types (mirror crates/ai/src/tools/create_liability.rs)
// ============================================================================

interface CreateLiabilityArgs {
  liabilityType: string;
  name?: string;
  principal: number;
  currency?: string;
  ratePct?: number;
  monthlyPayment?: number;
  originatedAt?: string;
  linkedAssetId?: string;
  notes?: string;
}

interface LiabilityDraft {
  kind: string;
  name: string;
  liabilityType: string;
  principal: number;
  currency: string;
  ratePct: number | null;
  monthlyPayment: number | null;
  originatedAt: string | null;
  linkedAssetId: string | null;
  notes: string | null;
}

interface ValidationResult {
  isValid: boolean;
  missingFields: string[];
  warnings: string[];
}

interface LiabilitySubtypeOption {
  value: string;
  label: string;
}

interface CreateLiabilityOutput {
  draft: LiabilityDraft;
  validation: ValidationResult;
  availableSubtypes: LiabilitySubtypeOption[];
  availableCurrencies: string[];
}

interface CreateLiabilityResult extends CreateLiabilityOutput {
  submitted?: boolean;
  createdAssetId?: string;
}

type Props = ToolCallMessagePartProps<CreateLiabilityArgs, CreateLiabilityResult>;

function normaliseResult(raw: unknown): CreateLiabilityResult | null {
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
  const obj = unwrapped as Partial<CreateLiabilityResult>;
  if (!obj.draft || typeof obj.draft !== "object") return null;
  return obj as CreateLiabilityResult;
}

function LoadingSkeleton() {
  return (
    <Card className="bg-muted/40 border-primary/10">
      <CardHeader className="pb-2">
        <Skeleton className="h-5 w-44" />
      </CardHeader>
      <CardContent className="space-y-3">
        <Skeleton className="h-4 w-3/4" />
        <Skeleton className="h-4 w-1/2" />
        <div className="flex gap-2 pt-2">
          <Skeleton className="h-9 w-28" />
        </div>
      </CardContent>
    </Card>
  );
}

function SuccessState({ draft }: { draft: LiabilityDraft }) {
  return (
    <Card className="border-success/30 bg-success/5">
      <CardHeader className="pb-2">
        <div className="flex items-center gap-2">
          <Icons.CheckCircle className="text-success size-5" />
          <CardTitle className="text-base">Liability added</CardTitle>
        </div>
      </CardHeader>
      <CardContent className="text-sm">
        <p>
          <span className="font-medium">{draft.name}</span>
          <span className="text-muted-foreground">
            {" "}
            · {draft.currency} {draft.principal.toLocaleString()}
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

/**
 * Carves the existing `createAlternativeAsset` adapter — same Tauri command
 * the legacy quick-add modal uses, so the resulting asset is identical to
 * one created through the UI form.
 */
function useCreateLiabilityMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: createAlternativeAsset,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [QueryKeys.HOLDINGS] });
      queryClient.invalidateQueries({ queryKey: [QueryKeys.ALTERNATIVE_HOLDINGS] });
      queryClient.invalidateQueries({ queryKey: [QueryKeys.NET_WORTH] });
      toast({ title: "Liability added.", variant: "success" });
    },
    onError: (e: unknown) => {
      const msg = e instanceof Error ? e.message : "Failed to add liability.";
      toast({ title: msg, variant: "destructive" });
    },
  });
}

function DraftCard({
  initialDraft,
  warnings,
  availableSubtypes,
  availableCurrencies,
  toolCallId,
  onSuccess,
}: {
  initialDraft: LiabilityDraft;
  warnings: string[];
  availableSubtypes: LiabilitySubtypeOption[];
  availableCurrencies: string[];
  toolCallId: string;
  onSuccess: (assetId: string) => void;
}) {
  const runtime = useRuntimeContext();
  const queryClient = useQueryClient();
  const threadId = runtime.currentThreadId;

  const [isEditing, setIsEditing] = useState(false);
  const [draft, setDraft] = useState<LiabilityDraft>(initialDraft);

  const mutation = useCreateLiabilityMutation();

  const todayIso = useMemo(() => new Date().toISOString().slice(0, 10), []);

  const canConfirm =
    draft.principal > 0 &&
    !!draft.currency &&
    !!draft.liabilityType &&
    !mutation.isPending;

  const handleConfirm = async () => {
    try {
      // Build metadata that the AlternativeAssetService persists with the asset.
      // Reads at display time via the existing liability-card surfaces.
      const metadata: Record<string, string> = {
        sub_type: draft.liabilityType,
      };
      if (draft.ratePct !== null) metadata.rate_pct = String(draft.ratePct);
      if (draft.monthlyPayment !== null) {
        metadata.monthly_payment = String(draft.monthlyPayment);
      }
      if (draft.originatedAt) metadata.originated_at = draft.originatedAt;
      if (draft.notes) metadata.notes = draft.notes;

      const created = await mutation.mutateAsync({
        kind: "liability",
        name: draft.name.trim(),
        currency: draft.currency,
        currentValue: String(draft.principal),
        valueDate: todayIso,
        metadata,
        linkedAssetId: draft.linkedAssetId ?? undefined,
      });

      refreshAfterMutation(queryClient);

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
            {isEditing ? "Edit liability draft" : "Add liability?"}
          </CardTitle>
          <Badge variant="secondary" className="text-xs">
            {availableSubtypes.find((s) => s.value === draft.liabilityType)?.label ??
              draft.liabilityType}
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
              <Label htmlFor="ai-create-liab-name">Name</Label>
              <Input
                id="ai-create-liab-name"
                value={draft.name}
                onChange={(e) => setDraft({ ...draft, name: e.target.value })}
                className="mt-1.5"
              />
            </div>
            <div>
              <Label htmlFor="ai-create-liab-type">Type</Label>
              <Select
                value={draft.liabilityType}
                onValueChange={(v) => setDraft({ ...draft, liabilityType: v })}
              >
                <SelectTrigger id="ai-create-liab-type" className="mt-1.5">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {availableSubtypes.map((t) => (
                    <SelectItem key={t.value} value={t.value}>
                      {t.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            <div>
              <Label htmlFor="ai-create-liab-currency">Currency</Label>
              <Select
                value={draft.currency}
                onValueChange={(v) => setDraft({ ...draft, currency: v })}
              >
                <SelectTrigger id="ai-create-liab-currency" className="mt-1.5">
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
              <Label htmlFor="ai-create-liab-principal">Principal</Label>
              <Input
                id="ai-create-liab-principal"
                type="number"
                inputMode="decimal"
                value={draft.principal}
                onChange={(e) =>
                  setDraft({ ...draft, principal: Number(e.target.value) || 0 })
                }
                className="mt-1.5"
              />
            </div>
            <div>
              <Label htmlFor="ai-create-liab-rate">Rate (%)</Label>
              <Input
                id="ai-create-liab-rate"
                type="number"
                inputMode="decimal"
                value={draft.ratePct ?? ""}
                onChange={(e) =>
                  setDraft({
                    ...draft,
                    ratePct: e.target.value === "" ? null : Number(e.target.value),
                  })
                }
                className="mt-1.5"
              />
            </div>
            <div>
              <Label htmlFor="ai-create-liab-monthly">Monthly payment</Label>
              <Input
                id="ai-create-liab-monthly"
                type="number"
                inputMode="decimal"
                value={draft.monthlyPayment ?? ""}
                onChange={(e) =>
                  setDraft({
                    ...draft,
                    monthlyPayment: e.target.value === "" ? null : Number(e.target.value),
                  })
                }
                className="mt-1.5"
              />
            </div>
            <div>
              <Label htmlFor="ai-create-liab-originated">Originated</Label>
              <Input
                id="ai-create-liab-originated"
                type="date"
                value={draft.originatedAt ?? ""}
                onChange={(e) =>
                  setDraft({
                    ...draft,
                    originatedAt: e.target.value ? e.target.value : null,
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
              label="Type"
              value={
                availableSubtypes.find((s) => s.value === draft.liabilityType)?.label ??
                draft.liabilityType
              }
            />
            <ReviewRow
              label="Principal"
              value={`${draft.currency} ${draft.principal.toLocaleString()}`}
            />
            <ReviewRow
              label="Rate"
              value={draft.ratePct !== null ? `${draft.ratePct}%` : "—"}
            />
            <ReviewRow
              label="Monthly payment"
              value={
                draft.monthlyPayment !== null
                  ? `${draft.currency} ${draft.monthlyPayment.toLocaleString()}`
                  : "—"
              }
            />
            <ReviewRow
              label="Originated"
              value={draft.originatedAt ?? "—"}
            />
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

function CreateLiabilityToolUIContentImpl({ result, status, toolCallId }: Props) {
  const parsed = useMemo(() => normaliseResult(result), [result]);
  const [submitted, setSubmitted] = useState(false);

  if (status?.type === "running") return <LoadingSkeleton />;
  if (status?.type === "incomplete") {
    return (
      <Card className="border-destructive/30 bg-destructive/5">
        <CardContent className="py-4">
          <p className="text-destructive text-sm font-medium">
            Couldn't prepare the liability draft.
          </p>
        </CardContent>
      </Card>
    );
  }
  if (!parsed) {
    return (
      <Card className="border-destructive/30 bg-destructive/5">
        <CardContent className="py-4">
          <p className="text-destructive text-sm font-medium">No liability data.</p>
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
      availableSubtypes={parsed.availableSubtypes}
      availableCurrencies={parsed.availableCurrencies}
      toolCallId={toolCallId}
      onSuccess={() => setSubmitted(true)}
    />
  );
}

const CreateLiabilityToolUIContent = memo(CreateLiabilityToolUIContentImpl);

export const CreateLiabilityToolUI = makeAssistantToolUI<
  CreateLiabilityArgs,
  CreateLiabilityResult
>({
  toolName: "create_liability",
  render: (props) => <CreateLiabilityToolUIContent {...props} />,
});

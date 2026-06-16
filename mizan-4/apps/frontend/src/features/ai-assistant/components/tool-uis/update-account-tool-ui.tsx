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

import { updateToolResult } from "@/adapters";
import { useAccountMutations } from "@/pages/settings/accounts/components/use-account-mutations";

import { useRuntimeContext } from "../../hooks/use-runtime-context";
import { unwrapToolResult } from "./shared";

// ============================================================================
// Types (mirror crates/ai/src/tools/update_account.rs)
// ============================================================================

interface UpdateAccountArgs {
  accountRef: string;
  name?: string;
  accountType?: string;
  isDefault?: boolean;
  isActive?: boolean;
  group?: string;
  notes?: string;
}

interface AccountUpdateDraft {
  id: string;
  name: string;
  accountType: string;
  group: string | null;
  isDefault: boolean;
  isActive: boolean;
  currency: string;
  notes: string | null;
}

interface AccountSnapshot {
  id: string;
  name: string;
  accountType: string;
  group: string | null;
  isDefault: boolean;
  isActive: boolean;
  currency: string;
  notes: string | null;
}

interface FieldDiff {
  field: string;
  old: string | null;
  new: string | null;
}

interface ValidationResult {
  isValid: boolean;
  missingFields: string[];
  warnings: string[];
  resolved: boolean;
  candidates: AccountSnapshot[];
}

interface AccountTypeOption {
  value: string;
  label: string;
}

interface UpdateAccountOutput {
  draft: AccountUpdateDraft;
  current: AccountSnapshot;
  diff: FieldDiff[];
  validation: ValidationResult;
  availableTypes: AccountTypeOption[];
}

interface UpdateAccountResult extends UpdateAccountOutput {
  submitted?: boolean;
  updatedAt?: string;
}

type Props = ToolCallMessagePartProps<UpdateAccountArgs, UpdateAccountResult>;

function normaliseResult(raw: unknown): UpdateAccountResult | null {
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
  const obj = unwrapped as Partial<UpdateAccountResult>;
  if (!obj.draft || !obj.current) return null;
  return obj as UpdateAccountResult;
}

function LoadingSkeleton() {
  return (
    <Card className="bg-muted/40 border-primary/10">
      <CardHeader className="pb-2">
        <Skeleton className="h-5 w-48" />
      </CardHeader>
      <CardContent className="space-y-3">
        <Skeleton className="h-4 w-3/4" />
        <Skeleton className="h-4 w-2/3" />
        <div className="flex gap-2 pt-2">
          <Skeleton className="h-9 w-24" />
          <Skeleton className="h-9 w-28" />
        </div>
      </CardContent>
    </Card>
  );
}

function FieldDiffRow({ diff }: { diff: FieldDiff }) {
  return (
    <div className="flex items-baseline gap-3 text-sm">
      <span className="text-muted-foreground w-28 shrink-0 text-xs uppercase tracking-wide">
        {diff.field}
      </span>
      <span className="text-muted-foreground line-through">{diff.old ?? "—"}</span>
      <Icons.ArrowRight className="text-muted-foreground size-3 shrink-0" />
      <span className="font-medium">{diff.new ?? "—"}</span>
    </div>
  );
}

function NeedsResolutionCard({
  validation,
}: {
  validation: ValidationResult;
}) {
  return (
    <Card className="border-warning/40 bg-warning/5">
      <CardHeader className="pb-2">
        <CardTitle className="text-base">Which account?</CardTitle>
      </CardHeader>
      <CardContent className="space-y-2">
        {validation.warnings.map((w, i) => (
          <p key={i} className="text-warning text-sm">
            {w}
          </p>
        ))}
        {validation.candidates.length > 0 && (
          <ul className="text-foreground mt-2 space-y-1 text-sm">
            {validation.candidates.map((c) => (
              <li key={c.id}>
                <span className="font-medium">{c.name}</span>
                <span className="text-muted-foreground"> · {c.accountType}</span>
                <span className="text-muted-foreground"> · {c.currency}</span>
              </li>
            ))}
          </ul>
        )}
      </CardContent>
    </Card>
  );
}

function SuccessState({ draft }: { draft: AccountUpdateDraft }) {
  return (
    <Card className="border-success/30 bg-success/5">
      <CardHeader className="pb-2">
        <div className="flex items-center gap-2">
          <Icons.CheckCircle className="text-success size-5" />
          <CardTitle className="text-base">Account updated</CardTitle>
        </div>
      </CardHeader>
      <CardContent className="text-sm">
        <p>
          <span className="font-medium">{draft.name}</span>
          <span className="text-muted-foreground"> · {draft.accountType}</span>
          <span className="text-muted-foreground"> · {draft.currency}</span>
        </p>
      </CardContent>
    </Card>
  );
}

function DraftCard({
  initialDraft,
  current,
  initialDiff,
  warnings,
  availableTypes,
  toolCallId,
  onSuccess,
}: {
  initialDraft: AccountUpdateDraft;
  current: AccountSnapshot;
  initialDiff: FieldDiff[];
  warnings: string[];
  availableTypes: AccountTypeOption[];
  toolCallId: string;
  onSuccess: () => void;
}) {
  const runtime = useRuntimeContext();
  const threadId = runtime.currentThreadId;

  const [isEditing, setIsEditing] = useState(false);
  const [draft, setDraft] = useState<AccountUpdateDraft>(initialDraft);

  const { updateAccountMutation } = useAccountMutations({});

  // Recompute the live diff as the user edits.
  const liveDiff = useMemo<FieldDiff[]>(() => {
    if (!isEditing) return initialDiff;
    const out: FieldDiff[] = [];
    if (draft.name !== current.name) {
      out.push({ field: "name", old: current.name, new: draft.name });
    }
    if (draft.accountType !== current.accountType) {
      out.push({
        field: "accountType",
        old: current.accountType,
        new: draft.accountType,
      });
    }
    if ((draft.group ?? "") !== (current.group ?? "")) {
      out.push({ field: "group", old: current.group, new: draft.group });
    }
    if (draft.isDefault !== current.isDefault) {
      out.push({
        field: "isDefault",
        old: current.isDefault ? "yes" : "no",
        new: draft.isDefault ? "yes" : "no",
      });
    }
    if (draft.isActive !== current.isActive) {
      out.push({
        field: "isActive",
        old: current.isActive ? "yes" : "no",
        new: draft.isActive ? "yes" : "no",
      });
    }
    return out;
  }, [draft, current, initialDiff, isEditing]);

  const canConfirm =
    liveDiff.length > 0 &&
    draft.name.trim().length >= 2 &&
    !updateAccountMutation.isPending;

  const handleConfirm = async () => {
    try {
      await updateAccountMutation.mutateAsync({
        id: draft.id,
        name: draft.name.trim(),
        accountType: draft.accountType as "SECURITIES" | "CASH" | "CRYPTOCURRENCY",
        currency: draft.currency,
        group: draft.group?.trim() || undefined,
        isDefault: draft.isDefault,
        isActive: draft.isActive,
        isArchived: false,
        trackingMode: "NOT_SET",
        meta: draft.notes ?? null,
      });

      if (threadId) {
        try {
          await updateToolResult({
            threadId,
            toolCallId,
            resultPatch: {
              submitted: true,
              updatedAt: new Date().toISOString(),
            },
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
          <CardTitle className="text-base">
            {isEditing ? "Edit changes" : "Update account?"}
          </CardTitle>
          <Badge variant="secondary" className="text-xs">
            {current.name}
          </Badge>
        </div>
        <p className="text-muted-foreground mt-1 text-xs">
          Review the change before saving. Nothing is updated until you click Confirm.
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
            <div>
              <Label htmlFor="ai-update-account-name">Name</Label>
              <Input
                id="ai-update-account-name"
                value={draft.name}
                onChange={(e) => setDraft({ ...draft, name: e.target.value })}
                className="mt-1.5"
              />
            </div>
            <div>
              <Label htmlFor="ai-update-account-type">Type</Label>
              <Select
                value={draft.accountType}
                onValueChange={(v) => setDraft({ ...draft, accountType: v })}
              >
                <SelectTrigger id="ai-update-account-type" className="mt-1.5">
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
            <div className="col-span-full">
              <Label htmlFor="ai-update-account-group">Group</Label>
              <Input
                id="ai-update-account-group"
                value={draft.group ?? ""}
                placeholder="e.g. Retirement, Spouse"
                onChange={(e) =>
                  setDraft({
                    ...draft,
                    group: e.target.value ? e.target.value : null,
                  })
                }
                className="mt-1.5"
              />
            </div>
          </div>
        ) : (
          <div className="space-y-1.5">
            {liveDiff.length === 0 ? (
              <p className="text-muted-foreground text-sm">
                No fields will change. Click Edit to make adjustments.
              </p>
            ) : (
              liveDiff.map((d) => <FieldDiffRow key={d.field} diff={d} />)
            )}
          </div>
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
            {updateAccountMutation.isPending ? (
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

function UpdateAccountToolUIContentImpl({ result, status, toolCallId }: Props) {
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
          <p className="text-destructive text-sm font-medium">
            No update data available.
          </p>
        </CardContent>
      </Card>
    );
  }

  if (!parsed.validation.resolved) {
    return <NeedsResolutionCard validation={parsed.validation} />;
  }

  if (parsed.submitted || submitted) {
    return <SuccessState draft={parsed.draft} />;
  }

  return (
    <DraftCard
      initialDraft={parsed.draft}
      current={parsed.current}
      initialDiff={parsed.diff}
      warnings={parsed.validation.warnings}
      availableTypes={parsed.availableTypes}
      toolCallId={toolCallId}
      onSuccess={() => setSubmitted(true)}
    />
  );
}

const UpdateAccountToolUIContent = memo(UpdateAccountToolUIContentImpl);

export const UpdateAccountToolUI = makeAssistantToolUI<UpdateAccountArgs, UpdateAccountResult>({
  toolName: "update_account",
  render: (props) => <UpdateAccountToolUIContent {...props} />,
});

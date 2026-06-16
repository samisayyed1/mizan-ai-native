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
import { Link } from "react-router-dom";

import { updateToolResult } from "@/adapters";
import { useAccountMutations } from "@/pages/settings/accounts/components/use-account-mutations";

import { useRuntimeContext } from "../../hooks/use-runtime-context";
import { useQueryClient } from "@tanstack/react-query";
import { refreshAfterMutation, unwrapToolResult } from "./shared";

// ============================================================================
// Types (mirror crates/ai/src/tools/create_account.rs)
// ============================================================================

interface CreateAccountArgs {
  name: string;
  accountType: string;
  currency?: string;
  isDefault?: boolean;
  group?: string;
  notes?: string;
}

interface AccountDraft {
  name: string;
  accountType: string;
  currency: string;
  isDefault: boolean;
  group: string | null;
  notes: string | null;
  provider: string;
}

interface ValidationResult {
  isValid: boolean;
  missingFields: string[];
  warnings: string[];
}

interface ExistingAccount {
  id: string;
  name: string;
  accountType: string;
  currency: string;
}

interface AccountTypeOption {
  value: string;
  label: string;
}

interface CreateAccountOutput {
  draft: AccountDraft;
  validation: ValidationResult;
  existingAccounts: ExistingAccount[];
  availableCurrencies: string[];
  availableTypes: AccountTypeOption[];
}

/** Result the runtime persists in the thread (we patch `submitted` on success). */
interface CreateAccountResult extends CreateAccountOutput {
  submitted?: boolean;
  createdAccountId?: string;
  createdAt?: string;
}

type Props = ToolCallMessagePartProps<CreateAccountArgs, CreateAccountResult>;

// ============================================================================
// Helpers
// ============================================================================

/** Defensive JSON-string handling — the runtime occasionally serialises mid-flight. */
function normaliseResult(raw: unknown): CreateAccountResult | null {
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
  const obj = unwrapped as Partial<CreateAccountResult>;
  if (!obj.draft || typeof obj.draft !== "object") return null;
  return obj as CreateAccountResult;
}

function labelForType(value: string, options: AccountTypeOption[]): string {
  return options.find((o) => o.value === value)?.label ?? value;
}

// ============================================================================
// Sub-components
// ============================================================================

function LoadingSkeleton() {
  return (
    <Card className="bg-muted/40 border-primary/10">
      <CardHeader className="pb-2">
        <div className="flex items-center justify-between">
          <Skeleton className="h-5 w-44" />
          <Skeleton className="h-5 w-20" />
        </div>
      </CardHeader>
      <CardContent className="space-y-3">
        <Skeleton className="h-4 w-3/4" />
        <Skeleton className="h-4 w-1/2" />
        <div className="flex gap-2 pt-2">
          <Skeleton className="h-9 w-24" />
          <Skeleton className="h-9 w-24" />
        </div>
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

function SuccessState({
  draft,
  createdAccountId,
}: {
  draft: AccountDraft;
  createdAccountId?: string;
}) {
  return (
    <Card className="border-success/30 bg-success/5">
      <CardHeader className="pb-2">
        <div className="flex items-center gap-2">
          <Icons.CheckCircle className="text-success size-5" />
          <CardTitle className="text-base">Account created</CardTitle>
        </div>
      </CardHeader>
      <CardContent className="space-y-2 text-sm">
        <p>
          <span className="font-medium">{draft.name}</span>
          <span className="text-muted-foreground"> · {draft.accountType}</span>
          <span className="text-muted-foreground"> · {draft.currency}</span>
        </p>
        {createdAccountId && (
          <Link
            to={`/accounts/${createdAccountId}`}
            className="text-primary inline-flex items-center gap-1 text-xs hover:underline"
          >
            View account
            <Icons.ArrowRight className="size-3" />
          </Link>
        )}
      </CardContent>
    </Card>
  );
}

interface DraftCardProps {
  initialDraft: AccountDraft;
  validation: ValidationResult;
  existing: ExistingAccount[];
  availableCurrencies: string[];
  availableTypes: AccountTypeOption[];
  toolCallId: string;
  onSuccess: (createdAccountId: string) => void;
}

function DraftCard({
  initialDraft,
  validation,
  existing,
  availableCurrencies,
  availableTypes,
  toolCallId,
  onSuccess,
}: DraftCardProps) {
  const runtime = useRuntimeContext();
  const queryClient = useQueryClient();
  const threadId = runtime.currentThreadId;

  const [isEditing, setIsEditing] = useState(false);
  const [draft, setDraft] = useState<AccountDraft>(initialDraft);

  const { createAccountMutation } = useAccountMutations({});

  // A near-duplicate match against existing accounts — only by exact-name now,
  // since fuzzy similarity would create false positives. Backend already adds
  // a warning when this is true.
  const duplicateExists = useMemo(() => {
    const lower = draft.name.trim().toLowerCase();
    return existing.some((a) => a.name.toLowerCase() === lower);
  }, [draft.name, existing]);

  const canConfirm =
    draft.name.trim().length >= 2 &&
    !!draft.accountType &&
    !!draft.currency &&
    !createAccountMutation.isPending;

  const handleConfirm = async () => {
    try {
      const created = await createAccountMutation.mutateAsync({
        name: draft.name.trim(),
        accountType: draft.accountType as "SECURITIES" | "CASH" | "CRYPTOCURRENCY",
        currency: draft.currency,
        group: draft.group?.trim() || undefined,
        isDefault: draft.isDefault,
        isActive: true,
        isArchived: false,
        trackingMode: "NOT_SET",
        meta: draft.notes?.trim() ? draft.notes.trim() : null,
      });

      refreshAfterMutation(queryClient);

      if (threadId) {
        try {
          await updateToolResult({
            threadId,
            toolCallId,
            resultPatch: {
              submitted: true,
              createdAccountId: created.id,
              createdAt: new Date().toISOString(),
            },
          });
        } catch (e) {
          console.error("Failed to persist tool result:", e);
        }
      }
      onSuccess(created.id);
    } catch {
      // Toast handled by useAccountMutations onError.
    }
  };

  return (
    <Card className="bg-muted/40 border-primary/10">
      <CardHeader className="pb-3">
        <div className="flex flex-wrap items-start justify-between gap-2">
          <div>
            <div className="flex items-center gap-2">
              <CardTitle className="text-base">
                {isEditing ? "Edit account draft" : "Create account?"}
              </CardTitle>
              <Badge variant="secondary" className="text-xs">
                {draft.provider === "MANUAL" ? "Manual" : draft.provider}
              </Badge>
            </div>
            <p className="text-muted-foreground mt-1 text-xs">
              Review before saving. Nothing is created until you click Confirm.
            </p>
          </div>
        </div>
      </CardHeader>

      <CardContent className="space-y-4">
        {/* Warnings */}
        {validation.warnings.length > 0 && (
          <div className="border-warning/40 bg-warning/10 text-warning rounded-md border px-3 py-2 text-xs">
            {validation.warnings.map((w, i) => (
              <div key={i}>{w}</div>
            ))}
          </div>
        )}
        {!validation.warnings.length && duplicateExists && (
          <div className="border-warning/40 bg-warning/10 text-warning rounded-md border px-3 py-2 text-xs">
            An account named "{draft.name}" already exists — consider asking the assistant to
            update it instead.
          </div>
        )}

        {isEditing ? (
          // ---------- EDIT MODE ----------
          <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
            <div>
              <Label htmlFor="ai-create-account-name">Name</Label>
              <Input
                id="ai-create-account-name"
                value={draft.name}
                onChange={(e) => setDraft({ ...draft, name: e.target.value })}
                className="mt-1.5"
              />
            </div>
            <div>
              <Label htmlFor="ai-create-account-type">Type</Label>
              <Select
                value={draft.accountType}
                onValueChange={(v) => setDraft({ ...draft, accountType: v })}
              >
                <SelectTrigger id="ai-create-account-type" className="mt-1.5">
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
            <div>
              <Label htmlFor="ai-create-account-currency">Currency</Label>
              <Select
                value={draft.currency}
                onValueChange={(v) => setDraft({ ...draft, currency: v })}
              >
                <SelectTrigger id="ai-create-account-currency" className="mt-1.5">
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
              <Label htmlFor="ai-create-account-group">Group (optional)</Label>
              <Input
                id="ai-create-account-group"
                value={draft.group ?? ""}
                placeholder="e.g. Retirement, Spouse"
                onChange={(e) =>
                  setDraft({ ...draft, group: e.target.value ? e.target.value : null })
                }
                className="mt-1.5"
              />
            </div>
          </div>
        ) : (
          // ---------- REVIEW MODE ----------
          <dl className="grid grid-cols-1 gap-x-6 gap-y-2 text-sm sm:grid-cols-2">
            <ReviewRow label="Name" value={draft.name || "—"} />
            <ReviewRow label="Type" value={labelForType(draft.accountType, availableTypes)} />
            <ReviewRow label="Currency" value={draft.currency} />
            {draft.group && <ReviewRow label="Group" value={draft.group} />}
            {draft.isDefault && <ReviewRow label="Default" value="Yes" />}
            {draft.notes && (
              <div className="col-span-full">
                <dt className="text-muted-foreground text-xs uppercase tracking-wide">Notes</dt>
                <dd className="mt-0.5">{draft.notes}</dd>
              </div>
            )}
          </dl>
        )}

        {/* Buttons */}
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
            {createAccountMutation.isPending ? (
              <>
                <Icons.Spinner className="mr-1.5 size-3.5 animate-spin" />
                Saving…
              </>
            ) : (
              <>
                <Icons.Check className="mr-1.5 size-3.5" />
                Confirm & create
              </>
            )}
          </Button>
        </div>

        {!validation.isValid && (
          <p className="text-muted-foreground text-xs">
            Missing: {validation.missingFields.join(", ")}
          </p>
        )}
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

// ============================================================================
// Top-level renderer
// ============================================================================

function CreateAccountToolUIContentImpl({ result, status, toolCallId }: Props) {
  const parsed = useMemo(() => normaliseResult(result), [result]);
  const [submitSuccess, setSubmitSuccess] = useState<
    { submitted: true; createdAccountId: string } | { submitted: false }
  >({ submitted: false });

  if (status?.type === "running") return <LoadingSkeleton />;
  if (status?.type === "incomplete")
    return (
      <ErrorCard
        title="Couldn't prepare the account"
        body="The request was interrupted before the draft was ready."
      />
    );

  if (!parsed)
    return (
      <ErrorCard
        title="No draft available"
        body="The AI didn't return a parseable account draft."
      />
    );

  const submitted =
    parsed.submitted ||
    (submitSuccess.submitted &&
      "createdAccountId" in submitSuccess &&
      !!submitSuccess.createdAccountId);

  if (submitted) {
    return (
      <SuccessState
        draft={parsed.draft}
        createdAccountId={
          parsed.createdAccountId ??
          (submitSuccess.submitted && "createdAccountId" in submitSuccess
            ? submitSuccess.createdAccountId
            : undefined)
        }
      />
    );
  }

  return (
    <DraftCard
      initialDraft={parsed.draft}
      validation={parsed.validation}
      existing={parsed.existingAccounts}
      availableCurrencies={parsed.availableCurrencies}
      availableTypes={parsed.availableTypes}
      toolCallId={toolCallId}
      onSuccess={(createdAccountId) =>
        setSubmitSuccess({ submitted: true, createdAccountId })
      }
    />
  );
}

const CreateAccountToolUIContent = memo(CreateAccountToolUIContentImpl);

export const CreateAccountToolUI = makeAssistantToolUI<CreateAccountArgs, CreateAccountResult>({
  toolName: "create_account",
  render: (props) => <CreateAccountToolUIContent {...props} />,
});

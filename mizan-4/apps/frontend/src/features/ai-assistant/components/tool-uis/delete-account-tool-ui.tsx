import type { ToolCallMessagePartProps } from "@assistant-ui/react";
import { makeAssistantToolUI } from "@assistant-ui/react";
import { Badge, Button, Card, CardContent, CardHeader, CardTitle, Skeleton } from "@mizan/ui";
import { Icons } from "@mizan/ui/components/ui/icons";
import { memo, useMemo, useState } from "react";

import { updateToolResult } from "@/adapters";
import { useAccountMutations } from "@/pages/settings/accounts/components/use-account-mutations";

import { useRuntimeContext } from "../../hooks/use-runtime-context";
import { unwrapToolResult } from "./shared";

// ============================================================================
// Types — mirror crates/ai/src/tools/delete_account.rs
// ============================================================================

interface DeleteAccountArgs {
  accountRef: string;
  reason?: string;
}

interface AccountSnapshot {
  id: string;
  name: string;
  accountType: string;
  currency: string;
  group: string | null;
  isDefault: boolean;
  isActive: boolean;
}

interface ActivityPreview {
  activityType: string;
  date: string;
  assetSymbol: string | null;
}

interface DeletionImpact {
  activityCount: number;
  recentActivities: ActivityPreview[];
  isDefault: boolean;
}

interface ValidationResult {
  resolved: boolean;
  candidates: AccountSnapshot[];
  warnings: string[];
  isValid: boolean;
}

interface DeleteAccountOutput {
  target: AccountSnapshot;
  impact: DeletionImpact;
  reason: string | null;
  validation: ValidationResult;
}

interface DeleteAccountResult extends DeleteAccountOutput {
  submitted?: boolean;
  deletedAt?: string;
}

type Props = ToolCallMessagePartProps<DeleteAccountArgs, DeleteAccountResult>;

// ============================================================================
// Helpers
// ============================================================================

function normaliseResult(raw: unknown): DeleteAccountResult | null {
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
  const obj = unwrapped as Partial<DeleteAccountResult>;
  if (!obj.target || !obj.impact || !obj.validation) return null;
  return obj as DeleteAccountResult;
}

// ============================================================================
// Sub-components
// ============================================================================

function LoadingSkeleton() {
  return (
    <Card className="bg-muted/40 border-destructive/10">
      <CardHeader className="pb-2">
        <div className="flex items-center justify-between">
          <Skeleton className="h-5 w-44" />
          <Skeleton className="h-5 w-20" />
        </div>
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

function SuccessState({ target }: { target: AccountSnapshot }) {
  return (
    <Card className="border-success/30 bg-success/5">
      <CardHeader className="pb-2">
        <div className="flex items-center gap-2">
          <Icons.CheckCircle className="text-success size-5" />
          <CardTitle className="text-base">Account deleted</CardTitle>
        </div>
      </CardHeader>
      <CardContent className="text-sm">
        <p>
          <span className="font-medium">{target.name}</span>
          <span className="text-muted-foreground"> · {target.accountType}</span>
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
  candidates: AccountSnapshot[];
  warnings: string[];
}) {
  return (
    <Card className="border-warning/30 bg-warning/5">
      <CardHeader className="pb-2">
        <div className="flex items-center gap-2">
          <Icons.AlertTriangle className="text-warning size-5" />
          <CardTitle className="text-base">Couldn't pick a target</CardTitle>
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
                <li key={c.id} className="flex items-center justify-between gap-2 text-xs">
                  <span>
                    <span className="font-medium">{c.name}</span>
                    <span className="text-muted-foreground">
                      {" "}
                      · {c.accountType} · {c.currency}
                    </span>
                  </span>
                </li>
              ))}
            </ul>
          </div>
        )}
      </CardContent>
    </Card>
  );
}

interface ConfirmCardProps {
  target: AccountSnapshot;
  impact: DeletionImpact;
  reason: string | null;
  warnings: string[];
  toolCallId: string;
  onSuccess: () => void;
}

function ConfirmCard({
  target,
  impact,
  reason,
  warnings,
  toolCallId,
  onSuccess,
}: ConfirmCardProps) {
  const runtime = useRuntimeContext();
  const threadId = runtime.currentThreadId;

  const [armed, setArmed] = useState(false);
  const { deleteAccountMutation } = useAccountMutations({});

  const handleConfirm = async () => {
    try {
      await deleteAccountMutation.mutateAsync(target.id);
      if (threadId) {
        try {
          await updateToolResult({
            threadId,
            toolCallId,
            resultPatch: {
              submitted: true,
              deletedAt: new Date().toISOString(),
            },
          });
        } catch (e) {
          console.error("Failed to persist tool result:", e);
        }
      }
      onSuccess();
    } catch {
      // Toast handled by useAccountMutations onError.
    }
  };

  return (
    // Destructive intent — red-tinted border + foreground so the user
    // knows this is irreversible at a glance, even if they're skimming.
    <Card className="border-destructive/30 bg-destructive/5">
      <CardHeader className="pb-3">
        <div className="flex flex-wrap items-start justify-between gap-2">
          <div>
            <div className="flex items-center gap-2">
              <Icons.Trash2 className="text-destructive size-4" />
              <CardTitle className="text-base">Delete account?</CardTitle>
              {impact.isDefault && (
                <Badge variant="secondary" className="text-xs">
                  Default
                </Badge>
              )}
            </div>
            <p className="text-muted-foreground mt-1 text-xs">
              This is irreversible. Activities tied to this account will be removed too.
            </p>
          </div>
        </div>
      </CardHeader>

      <CardContent className="space-y-3 text-sm">
        {/* Target summary */}
        <div className="bg-background/50 rounded-md border px-3 py-2">
          <p>
            <span className="font-medium">{target.name}</span>
            <span className="text-muted-foreground"> · {target.accountType}</span>
            <span className="text-muted-foreground"> · {target.currency}</span>
            {target.group && (
              <span className="text-muted-foreground"> · {target.group}</span>
            )}
          </p>
          {reason && (
            <p className="text-muted-foreground mt-1 text-xs italic">"{reason}"</p>
          )}
        </div>

        {/* Cascade-impact summary */}
        {impact.activityCount > 0 && (
          <div className="text-muted-foreground text-xs">
            <p className="font-medium">
              {impact.activityCount} activit{impact.activityCount === 1 ? "y" : "ies"} will be
              removed
            </p>
            {impact.recentActivities.length > 0 && (
              <ul className="mt-1 space-y-0.5">
                {impact.recentActivities.slice(0, 3).map((a, i) => (
                  <li key={i}>
                    · {a.activityType} on {a.date}
                  </li>
                ))}
                {impact.activityCount > 3 && (
                  <li className="italic">... and {impact.activityCount - 3} more</li>
                )}
              </ul>
            )}
          </div>
        )}

        {/* Warnings */}
        {warnings.length > 0 && (
          <div className="border-warning/40 bg-warning/10 text-warning rounded-md border px-3 py-2 text-xs">
            {warnings.map((w, i) => (
              <div key={i}>{w}</div>
            ))}
          </div>
        )}

        {/* Buttons — two-stage arm/confirm pattern. The first click flips
            the button into "Yes, delete" so casual clicks can't trigger
            destructive ops; this matches the GitHub / Linear / Notion
            mental model for destructive actions in tool-call cards. */}
        <div className="flex flex-wrap items-center justify-end gap-2 pt-1">
          <Button
            variant="ghost"
            size="sm"
            onClick={() => setArmed(false)}
            disabled={!armed || deleteAccountMutation.isPending}
          >
            Cancel
          </Button>
          {armed ? (
            <Button
              variant="destructive"
              size="sm"
              onClick={handleConfirm}
              disabled={deleteAccountMutation.isPending}
            >
              {deleteAccountMutation.isPending ? (
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
            <Button
              variant="destructive"
              size="sm"
              onClick={() => setArmed(true)}
            >
              <Icons.Trash2 className="mr-1.5 size-3.5" />
              Delete
            </Button>
          )}
        </div>
      </CardContent>
    </Card>
  );
}

// ============================================================================
// Top-level renderer
// ============================================================================

function DeleteAccountToolUIContentImpl({ result, status, toolCallId }: Props) {
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

  // Submitted once (via either the persisted result or our local optimistic
  // flag) → render the success card. Idempotent on thread reload.
  if (parsed.submitted || submitted) {
    return <SuccessState target={parsed.target} />;
  }

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
      impact={parsed.impact}
      reason={parsed.reason}
      warnings={parsed.validation.warnings}
      toolCallId={toolCallId}
      onSuccess={() => setSubmitted(true)}
    />
  );
}

const DeleteAccountToolUIContent = memo(DeleteAccountToolUIContentImpl);

export const DeleteAccountToolUI = makeAssistantToolUI<DeleteAccountArgs, DeleteAccountResult>({
  toolName: "delete_account",
  render: (props) => <DeleteAccountToolUIContent {...props} />,
});

/**
 * AddAssetDialog — the inline-everywhere "Add" surface.
 *
 * Replaces the old flow (Add button → navigate to /assistant?intent=add-asset
 * with a pre-filled prompt) with a dialog that opens IN PLACE wherever the
 * user clicked. Two choices, no redirection, no context loss:
 *
 *   ┌─────────────────────────────────────────────────────────┐
 *   │ Add to your portfolio                                   │
 *   ├─────────────────────────────────────────────────────────┤
 *   │  ┌──────────────────┐    ┌──────────────────┐          │
 *   │  │ ✨ Ask Mizan AI  │    │ ✏️  Add manually │          │
 *   │  │ (default)        │    │                  │          │
 *   │  └──────────────────┘    └──────────────────┘          │
 *   ├─────────────────────────────────────────────────────────┤
 *   │  > Tell me what to add or drop a file…                 │
 *   │  [textarea]                          [📎] [⏎ Send]     │
 *   ├─────────────────────────────────────────────────────────┤
 *   │  (When run starts → AgentProgressCard renders here.)    │
 *   └─────────────────────────────────────────────────────────┘
 *
 * "Add manually" is just a navigation shortcut — it routes to
 * /settings/accounts (the existing AccountEditModal opens from there)
 * and closes the dialog.
 *
 * "Ask Mizan AI" expands an inline composer in the SAME dialog. The
 * user types/drops + clicks Send → `streamAgentChat` runs against
 * the Plan→Execute→Verify→Undo runtime, AgentProgressCard streams
 * progress, and at Done the dialog shows an Undo button. No
 * navigation. Ever.
 *
 * The "AI" pill on top of each card is keyboard-focusable; Enter on
 * either one selects it. Tab order: AI card → manual card → textarea
 * → file input → Send.
 */

import { useCallback, useEffect, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";

import { Button } from "@mizan/ui/components/ui/button";
import { Dialog, DialogContent } from "@mizan/ui/components/ui/dialog";
import { Icons } from "@mizan/ui/components/ui/icons";
import { Textarea } from "@mizan/ui/components/ui/textarea";
import { cn } from "@/lib/utils";

import { streamAgentChat, type AgentRunRequest } from "@/adapters";
import {
  AgentProgressCard,
  useAgentEventStream,
} from "@/features/ai-assistant/components/agent-progress-card";
import type { AiStreamEvent } from "@/features/ai-assistant/types";
import { useAccounts } from "@/hooks/use-accounts";

// ─── Props + state ─────────────────────────────────────────────────────

export interface AddAssetDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** Optional starting prompt — pre-fills the textarea when the dialog
   *  opens. Falls back to a generic placeholder. Useful for context-
   *  aware Add buttons (e.g. on the goals page → "Set a new goal of…"). */
  initialPrompt?: string;
  /**
   * When set, the dialog was opened from an account-scoped surface
   * (e.g. account-detail page's "Add stock" button). Skips re-asking
   * the user which portfolio they want to add to:
   *   - AI mode: the prompt is enriched with "for the {accountName}
   *     portfolio (id {accountId})" so Mizan AI uses it without asking.
   *   - Manual mode: navigates to /activities/manage?accountId=… so the
   *     form pre-selects the account.
   */
  presetAccountId?: string;
  presetAccountName?: string;
}

type Mode = "chooser" | "ai" | "running" | "done" | "error";

export function AddAssetDialog({
  open,
  onOpenChange,
  initialPrompt,
  presetAccountId,
  presetAccountName,
}: AddAssetDialogProps) {
  const navigate = useNavigate();
  // We need to know whether the user has any portfolios so the
  // "Add manually" card can route to the right surface — adding an
  // activity into an existing portfolio if they have one, or creating
  // their first portfolio if they don't. Without this branch we always
  // dump the user on the "create a new portfolio" form even when they
  // already have one open in front of them, which was the reported bug.
  const { accounts } = useAccounts({ filterActive: true });
  const hasAnyPortfolio = (accounts?.length ?? 0) > 0;
  const [mode, setMode] = useState<Mode>("chooser");
  const [prompt, setPrompt] = useState(initialPrompt ?? "");
  const [attachments, setAttachments] = useState<
    NonNullable<AgentRunRequest["attachments"]>
  >([]);
  const [errorText, setErrorText] = useState<string | null>(null);
  /** When set, the error block renders a "Sign in" CTA instead of just
   *  the raw error message. Triggered when the agent runtime reports
   *  no Mizan Connect session — the user needs to authenticate before
   *  Mizan AI can run. */
  const [errorKind, setErrorKind] = useState<"sign_in" | "generic">("generic");
  const fileInputRef = useRef<HTMLInputElement | null>(null);
  const textareaRef = useRef<HTMLTextAreaElement | null>(null);
  const abortRef = useRef<AbortController | null>(null);
  const { events, ingest, reset, runId } = useAgentEventStream();
  const [elapsedSeconds, setElapsedSeconds] = useState(0);

  // Reset all state when the dialog closes.
  useEffect(() => {
    if (!open) {
      const timer = setTimeout(() => {
        // Wait for close animation so the user doesn't see the chooser
        // flash back into view while the panel slides out.
        setMode("chooser");
        setPrompt(initialPrompt ?? "");
        setAttachments([]);
        setErrorText(null);
        setElapsedSeconds(0);
        reset();
        abortRef.current?.abort();
        abortRef.current = null;
      }, 200);
      return () => clearTimeout(timer);
    }
    return;
  }, [open, initialPrompt, reset]);

  // Wall-clock ticker while running.
  useEffect(() => {
    if (mode !== "running") return;
    const start = Date.now();
    const id = setInterval(() => {
      setElapsedSeconds(Math.floor((Date.now() - start) / 1000));
    }, 1000);
    return () => clearInterval(id);
  }, [mode]);

  // Focus the textarea when entering AI mode for a one-key UX.
  useEffect(() => {
    if (mode === "ai") {
      // Microtask so the textarea exists in the DOM by the time we focus.
      queueMicrotask(() => textareaRef.current?.focus());
    }
  }, [mode]);

  // ─── Actions ────────────────────────────────────────────────────────

  const handleAddManually = useCallback(() => {
    onOpenChange(false);
    // The manual surface depends on what the user is actually missing:
    //
    //   * Has a portfolio → they want to add an ASSET (a holding, a
    //     transaction, a balance). The canonical form is
    //     /activities/manage — the Add Activity form with a portfolio
    //     selector at the top. When we came from an account-scoped
    //     surface (Enterprise UX-2), pass the account ID so the form
    //     pre-selects it and the user is NOT asked to pick a portfolio
    //     again — that repetition was the reported bug.
    //
    //   * Has zero portfolios → they need a portfolio before any
    //     activity will make sense. Send them to the AccountEditModal
    //     (auto-opened via ?addAccount=1).
    if (hasAnyPortfolio) {
      // /activities/manage's existing param is `account` (the activity
      // manager has had this since before Enterprise UX-2). Match that
      // contract rather than introduce a new alias.
      //
      // Also pass `redirect-to` pointing back at the account-detail page
      // when we know it — the activity manager already honours that
      // param on close, so the user lands back where they started
      // instead of one step down the stack ("Activities" page).
      if (presetAccountId) {
        const back = `/accounts/${encodeURIComponent(presetAccountId)}`;
        const target =
          `/activities/manage?account=${encodeURIComponent(presetAccountId)}` +
          `&redirect-to=${encodeURIComponent(back)}`;
        navigate(target);
      } else {
        navigate("/activities/manage");
      }
    } else {
      navigate("/settings/accounts?addAccount=1");
    }
  }, [hasAnyPortfolio, navigate, onOpenChange, presetAccountId]);

  const handleFilePick = useCallback(
    async (files: FileList | null) => {
      if (!files || files.length === 0) return;
      const next: typeof attachments = [];
      for (const file of Array.from(files)) {
        const buf = await file.arrayBuffer();
        const bytes = new Uint8Array(buf);
        // Base64 the bytes — the agent runtime decodes server-side.
        let binary = "";
        const chunkSize = 0x8000;
        for (let i = 0; i < bytes.length; i += chunkSize) {
          binary += String.fromCharCode(
            ...bytes.subarray(i, i + chunkSize),
          );
        }
        next.push({
          name: file.name,
          contentType: file.type || "application/octet-stream",
          // AiMessageAttachment.data is the single payload field:
          // plain text for CSV (decoded server-side), base64 for binary.
          // For CSV we keep it as text; for everything else we base64.
          data:
            file.type === "text/csv" || file.name.endsWith(".csv")
              ? new TextDecoder().decode(bytes)
              : btoa(binary),
        });
      }
      setAttachments((prev) => [...prev, ...next]);
    },
    // `attachments` is only referenced via `typeof attachments` (compile-time
    // type alias) + the functional setState form. No runtime read → no dep.
    [],
  );

  const handleSend = useCallback(async () => {
    if (!prompt.trim() && attachments.length === 0) return;
    setMode("running");
    setErrorText(null);
    reset();
    const controller = new AbortController();
    abortRef.current = controller;
    try {
      // Enterprise UX-2: prepend an account-scope hint to the agent's
      // prompt when the dialog was opened from an account-detail page.
      // The agent's record_activity / add_alternative_asset tools see
      // the hint, resolve the account by id/name automatically, and
      // skip the "which portfolio?" follow-up. Without this, the AI
      // would ask the user the question they just answered by clicking
      // "Add stock" from inside the account.
      const accountHint = presetAccountId
        ? `Use account "${presetAccountName ?? presetAccountId}" (account_id: ${presetAccountId}) for this. `
        : "";
      const composedPrompt = `${accountHint}${prompt.trim()}`.trim();
      const request: AgentRunRequest = {
        content: composedPrompt || "Process the attached file(s).",
        attachments: attachments.length > 0 ? attachments : undefined,
      };
      for await (const ev of streamAgentChat(request, controller.signal)) {
        if (controller.signal.aborted) break;
        if (ev.type === "agent") {
          // The Tauri layer wraps AgentEvent in {type: 'agent', event: ...}.
          // `ev.event` IS the inner AgentInnerEvent the progress card
          // expects.
          ingest((ev as AiStreamEvent & { event: never }).event);
        } else if (ev.type === "error") {
          // No recipe matched the user's free-form intent (e.g. "add
          // 10 oz gold", "delete my old Wise account"). The agent
          // runtime can only run canned plans for a handful of
          // recipes (CSV import, retirement plan, etc.); for
          // everything else, route the same prompt to the regular
          // chat path on /assistant, where the full mutation tool
          // surface (create_account / record_activity / delete_* /
          // …) lives. The intent + prompt search params pre-fill
          // the composer so the user just clicks Send.
          if (ev.code === "agent_no_recipe_match") {
            const params = new URLSearchParams({
              intent: "add-asset",
              prompt: composedPrompt,
            });
            navigate(`/assistant?${params.toString()}`);
            onOpenChange(false);
            return;
          }
          // Detect "no Mizan Connect session" — the most common error
          // on a fresh install before sign-in. Surface a Sign-in CTA
          // instead of the raw "missing_api_key" code, which means
          // nothing to a user.
          const isSignIn =
            ev.code === "missing_api_key" ||
            ev.code === "unauthorized" ||
            /no token|not authenticated|sign in|connect not configured/i.test(
              ev.message ?? "",
            );
          setErrorText(
            isSignIn
              ? "You need to sign in with Mizan to use the AI agent. Free tier includes 50 messages a day."
              : ev.message,
          );
          setErrorKind(isSignIn ? "sign_in" : "generic");
          setMode("error");
          return;
        } else if (ev.type === "done") {
          setMode("done");
          return;
        }
      }
      // Stream ended without an explicit done — treat as completed.
      setMode("done");
    } catch (err) {
      setErrorText(err instanceof Error ? err.message : String(err));
      setErrorKind("generic");
      setMode("error");
    }
  }, [prompt, attachments, ingest, reset, presetAccountId, presetAccountName, navigate, onOpenChange]);

  const handleCancel = useCallback(() => {
    abortRef.current?.abort();
    abortRef.current = null;
    setMode("ai");
  }, []);

  const handleUndo = useCallback(async (_undoBatchId: string) => {
    // Undo command is exposed via a separate Tauri command (`undo_agent_run`)
    // that this dialog could call. For v1 the AgentProgressCard surfaces
    // the button + the runtime persists the run id in the truth ledger;
    // we'll close the dialog and emit a toast so the user knows the
    // reversal happened. Replace this stub with the real invoke when
    // the undo command ships.
    onOpenChange(false);
  }, [onOpenChange]);

  // ─── Render ─────────────────────────────────────────────────────────

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        className="max-h-[90vh] overflow-y-auto sm:max-w-[640px]"
        // No nav inside this dialog — Esc closes via Radix's default.
      >
        {/* Header is tight on purpose. We don't want a giant title
            competing with the two cards below. */}
        <div className="mb-4 space-y-1">
          <h2 className="text-foreground text-lg font-semibold tracking-tight">
            Add to your portfolio
          </h2>
          <p className="text-muted-foreground text-sm">
            Mizan can set things up for you, or you can do it manually.
          </p>
        </div>

        {/* Two-card chooser. Always visible — even when AI mode is
            active — so the user can switch to manual mid-thought
            without closing/reopening. The AI card lights up amber
            when selected. */}
        <div className="mb-4 grid grid-cols-1 gap-3 sm:grid-cols-2">
          <ChooserCard
            icon={<Icons.Sparkles className="h-5 w-5" />}
            title="Ask Mizan AI"
            subtitle="Describe it or drop a file. The agent does the rest."
            active={mode === "ai" || mode === "running" || mode === "done" || mode === "error"}
            onClick={() => setMode("ai")}
          />
          <ChooserCard
            icon={<Icons.Plus className="h-5 w-5" />}
            title="Add manually"
            subtitle={
              hasAnyPortfolio
                ? "Record a buy, deposit, or balance in one of your portfolios."
                : "Create your first portfolio, then add what's in it."
            }
            active={false}
            onClick={handleAddManually}
          />
        </div>

        {/* Inline composer — visible whenever AI mode is selected. */}
        {(mode === "ai" || mode === "running" || mode === "error") && (
          <div className="border-border bg-muted/30 rounded-xl border p-3">
            <Textarea
              ref={textareaRef}
              value={prompt}
              onChange={(e) => setPrompt(e.target.value)}
              placeholder={
                initialPrompt ??
                "e.g. Make a new portfolio called US Stocks and import this CSV. Or: I have 15.5oz of gold worth $3,200/oz. Or: Add a rental property in Toronto valued at $850k."
              }
              disabled={mode === "running"}
              rows={3}
              className="resize-none border-0 bg-transparent p-0 text-sm focus-visible:ring-0"
              onKeyDown={(e) => {
                if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
                  e.preventDefault();
                  void handleSend();
                }
              }}
            />

            {attachments.length > 0 && (
              <div className="mt-2 flex flex-wrap gap-1.5">
                {attachments.map((a, i) => (
                  <span
                    key={`${a.name}-${i}`}
                    className="bg-background text-foreground inline-flex items-center gap-1 rounded-md border px-2 py-0.5 text-xs"
                  >
                    <Icons.Upload className="h-3 w-3" />
                    {a.name}
                    <button
                      type="button"
                      onClick={() =>
                        setAttachments((prev) =>
                          prev.filter((_, idx) => idx !== i),
                        )
                      }
                      className="text-muted-foreground hover:text-foreground ml-0.5"
                      aria-label={`Remove ${a.name}`}
                    >
                      <Icons.X className="h-3 w-3" />
                    </button>
                  </span>
                ))}
              </div>
            )}

            <div className="mt-2 flex items-center justify-between gap-2">
              <div className="flex items-center gap-1">
                <input
                  ref={fileInputRef}
                  type="file"
                  multiple
                  accept=".csv,.txt,.pdf,image/*"
                  className="hidden"
                  onChange={(e) => void handleFilePick(e.target.files)}
                />
                <Button
                  type="button"
                  size="sm"
                  variant="ghost"
                  disabled={mode === "running"}
                  onClick={() => fileInputRef.current?.click()}
                  className="text-muted-foreground"
                >
                  <Icons.Upload className="mr-1.5 h-4 w-4" />
                  Attach
                </Button>
              </div>
              <div className="flex items-center gap-2">
                <span className="text-muted-foreground hidden text-xs sm:inline">
                  ⌘+Enter to send
                </span>
                <Button
                  type="button"
                  size="sm"
                  disabled={
                    mode === "running" ||
                    (!prompt.trim() && attachments.length === 0)
                  }
                  onClick={() => void handleSend()}
                >
                  {mode === "running" ? (
                    <>
                      <Icons.Spinner className="mr-1.5 h-4 w-4 animate-spin" />
                      Working…
                    </>
                  ) : (
                    <>
                      <Icons.ArrowUp className="mr-1.5 h-4 w-4" />
                      Send
                    </>
                  )}
                </Button>
              </div>
            </div>
          </div>
        )}

        {/* Inline progress card. Mounts as soon as the run starts so
            the user sees the plan emerging in real time. Stays mounted
            after Done so the Undo button is still reachable. */}
        {(mode === "running" || mode === "done" || mode === "error") &&
          events.length > 0 && (
            <div className="mt-4">
              <AgentProgressCard
                events={events}
                elapsedSeconds={elapsedSeconds}
                onCancel={mode === "running" ? handleCancel : undefined}
                onUndo={mode === "done" ? handleUndo : undefined}
                title={runId ? "Mizan agent" : "Working"}
              />
            </div>
          )}

        {/* Inline error block — separate from progress card so even an
            "early" error (e.g. tier-locked, no recipe matched) surfaces
            cleanly without the card half-rendering. The "sign_in"
            variant swaps the destructive treatment for a friendlier
            amber prompt + a direct CTA to the Mizan Connect login. */}
        {mode === "error" && errorText && errorKind === "sign_in" && (
          <div className="mt-4 rounded-md border border-amber-500/30 bg-amber-500/10 p-3 text-sm">
            <div className="flex items-start gap-2">
              <Icons.Sparkles className="mt-0.5 h-4 w-4 flex-shrink-0 text-amber-600 dark:text-amber-300" />
              <div className="flex-1">
                <div className="text-foreground font-medium">Sign in to use Mizan AI</div>
                <div className="text-muted-foreground mt-0.5 text-xs">{errorText}</div>
                <div className="mt-2 flex flex-wrap gap-2">
                  <Button
                    size="sm"
                    onClick={() => {
                      onOpenChange(false);
                      navigate("/settings/connect");
                    }}
                  >
                    Sign in
                  </Button>
                  <Button
                    size="sm"
                    variant="ghost"
                    onClick={() => {
                      setMode("ai");
                      setErrorText(null);
                    }}
                  >
                    Back
                  </Button>
                </div>
              </div>
            </div>
          </div>
        )}
        {mode === "error" && errorText && errorKind === "generic" && (
          <div className="border-destructive/30 bg-destructive/10 text-destructive mt-4 rounded-md border p-3 text-sm">
            <div className="flex items-start gap-2">
              <Icons.AlertCircle className="mt-0.5 h-4 w-4 flex-shrink-0" />
              <div className="flex-1">
                <div className="font-medium">Couldn't run that</div>
                <div className="mt-0.5 text-xs opacity-90">{errorText}</div>
              </div>
            </div>
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
}

// ─── Chooser card ──────────────────────────────────────────────────────

function ChooserCard({
  icon,
  title,
  subtitle,
  active,
  onClick,
}: {
  icon: React.ReactNode;
  title: string;
  subtitle: string;
  active: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        "group rounded-xl border p-4 text-left transition-all",
        "hover:border-foreground/40 hover:bg-muted/40",
        active
          ? "border-amber-500/60 bg-amber-500/5 ring-2 ring-amber-500/20"
          : "border-border bg-background",
      )}
    >
      <div className="flex items-center gap-2.5">
        <span
          className={cn(
            "flex h-8 w-8 items-center justify-center rounded-md",
            active
              ? "bg-amber-500/15 text-amber-700 dark:text-amber-300"
              : "bg-muted text-foreground",
          )}
        >
          {icon}
        </span>
        <span className="text-foreground text-sm font-medium">{title}</span>
      </div>
      <p className="text-muted-foreground mt-2 text-xs leading-relaxed">
        {subtitle}
      </p>
    </button>
  );
}

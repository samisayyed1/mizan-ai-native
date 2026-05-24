/**
 * AgentProgressCard — live progress display for autonomous agent runs.
 *
 * Replaces the static "..." spinner with a Devin/Cursor-style panel:
 *
 *   ┌─────────────────────────────────────────────────────────┐
 *   │ 🤖 Setting up your US Stocks portfolio        02:14    │
 *   ├─────────────────────────────────────────────────────────┤
 *   │ ✓ Plan ready (5 steps)                                  │
 *   │ ✓ Created account 'US Stocks' (USD)                     │
 *   │ ⏳ Parsing 402 rows from portfolio US Stocks (1).csv     │
 *   │ ○ Record activities                                      │
 *   │ ○ Verify totals match CSV                                │
 *   ├─────────────────────────────────────────────────────────┤
 *   │ [ Cancel run ]                                           │
 *   └─────────────────────────────────────────────────────────┘
 *
 *   On RunComplete:
 *   ┌─────────────────────────────────────────────────────────┐
 *   │ ✓ Setup complete                                         │
 *   │ Created portfolio · 402 activities · totals match        │
 *   │ [ Undo run ]                                             │
 *   └─────────────────────────────────────────────────────────┘
 *
 * Subscribes to AgentInnerEvent stream + maintains its own reducer
 * state. Hooks into the existing chat shell's SSE pipeline through
 * a parent that filters AgentEnvelopeEvent and forwards the inner
 * AgentInnerEvent into this component.
 */

import { useMemo, useReducer } from "react";
import { Icons } from "@mizan/ui/components/ui/icons";
import { cn } from "@mizan/ui/lib/utils";
import { Button } from "@mizan/ui/components/ui/button";
import type { AgentInnerEvent, AgentPlanStep } from "../types";

// Use the curated icon barrel — keeps the bundle lean + lets us swap
// icon library underneath without touching every component.
const CheckCircle2 = Icons.CheckCircle;
const Circle = Icons.Circle;
const Loader2 = Icons.Spinner;
const AlertCircle = Icons.AlertCircle;
const RotateCw = Icons.RefreshCw;

// ─── State ──────────────────────────────────────────────────────────

type StepStatus =
  | { kind: "pending" }
  | { kind: "running"; progress: number; message?: string }
  | { kind: "done"; ledgerEntries: string[] }
  | { kind: "failed"; error: string; retrying: boolean };

interface AgentRunState {
  runId: string | null;
  plan: AgentPlanStep[];
  stepStatus: StepStatus[];
  terminal:
    | { kind: "running" }
    | { kind: "complete"; summary: string; undoBatchId: string; toolCalls: number; wallClockMs: number }
    | { kind: "aborted"; reason: string; partialLedgerEntries: string[] };
  rePlanReason: string | null;
  verifyDiscrepancies: string[] | null;
}

const INITIAL: AgentRunState = {
  runId: null,
  plan: [],
  stepStatus: [],
  terminal: { kind: "running" },
  rePlanReason: null,
  verifyDiscrepancies: null,
};

function reducer(state: AgentRunState, ev: AgentInnerEvent): AgentRunState {
  switch (ev.type) {
    case "plan_ready":
      return {
        ...state,
        runId: ev.runId,
        plan: ev.plan,
        stepStatus: ev.plan.map(() => ({ kind: "pending" }) as StepStatus),
      };

    case "step_start": {
      const status = [...state.stepStatus];
      status[ev.stepIndex] = { kind: "running", progress: 0 };
      return { ...state, stepStatus: status };
    }

    case "step_progress": {
      const status = [...state.stepStatus];
      const existing = status[ev.stepIndex];
      if (existing?.kind === "running") {
        status[ev.stepIndex] = {
          kind: "running",
          progress: ev.progress,
          message: ev.message,
        };
      }
      return { ...state, stepStatus: status };
    }

    case "step_done": {
      const status = [...state.stepStatus];
      status[ev.stepIndex] = { kind: "done", ledgerEntries: ev.ledgerEntries };
      return { ...state, stepStatus: status };
    }

    case "step_failed": {
      const status = [...state.stepStatus];
      status[ev.stepIndex] = {
        kind: "failed",
        error: ev.error,
        retrying: ev.retrying,
      };
      return { ...state, stepStatus: status };
    }

    case "re_planning":
      return { ...state, rePlanReason: ev.reason };

    case "verified":
      return { ...state, verifyDiscrepancies: ev.discrepancies };

    case "run_complete":
      return {
        ...state,
        terminal: {
          kind: "complete",
          summary: ev.summary,
          undoBatchId: ev.undoBatchId,
          toolCalls: ev.toolCalls,
          wallClockMs: ev.wallClockMs,
        },
      };

    case "run_aborted":
      return {
        ...state,
        terminal: {
          kind: "aborted",
          reason: ev.reason,
          partialLedgerEntries: ev.partialLedgerEntries,
        },
      };

    default:
      return state;
  }
}

// ─── Component ──────────────────────────────────────────────────────

export interface AgentProgressCardProps {
  /** Stream of agent events. Pass the dispatcher from the chat shell;
   *  re-renders happen via the internal reducer. */
  events: AgentInnerEvent[];
  /** Title shown at the top of the card. Falls back to "Working" when
   *  the recipe name isn't surfaced. */
  title?: string;
  /** Wall-clock seconds elapsed for the running case. Optional —
   *  parent passes it to avoid a per-card setInterval. */
  elapsedSeconds?: number;
  /** Called when the user clicks "Cancel run" mid-execution. */
  onCancel?: () => void;
  /** Called when the user clicks "Undo run" after completion. */
  onUndo?: (undoBatchId: string) => void;
}

export function AgentProgressCard({
  events,
  title = "Working",
  elapsedSeconds,
  onCancel,
  onUndo,
}: AgentProgressCardProps) {
  // Replay events through the reducer to derive current state.
  // Cheap: events count for one run typically < 50.
  const state = useMemo(() => {
    return events.reduce(reducer, INITIAL);
  }, [events]);

  const isRunning = state.terminal.kind === "running";
  const isComplete = state.terminal.kind === "complete";
  const isAborted = state.terminal.kind === "aborted";

  return (
    <div
      className={cn(
        "border rounded-2xl my-3 overflow-hidden font-mono text-sm",
        isComplete && "border-emerald-300 bg-emerald-50/50",
        isAborted && "border-rose-300 bg-rose-50/50",
        isRunning && "border-amber-200 bg-amber-50/30"
      )}
      role="status"
      aria-live="polite"
    >
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-2.5 border-b border-current/10">
        <div className="flex items-center gap-2">
          {isRunning ? (
            <Loader2 className="size-4 animate-spin text-amber-700" />
          ) : isComplete ? (
            <CheckCircle2 className="size-4 text-emerald-700" />
          ) : (
            <AlertCircle className="size-4 text-rose-700" />
          )}
          <span className="font-semibold">{title}</span>
        </div>
        {typeof elapsedSeconds === "number" && (
          <span className="text-xs opacity-60 tabular-nums">
            {formatElapsed(elapsedSeconds)}
          </span>
        )}
      </div>

      {/* Step list */}
      {state.plan.length > 0 && (
        <ol className="px-4 py-2 space-y-1.5">
          {state.plan.map((step, idx) => {
            const status = state.stepStatus[idx] ?? { kind: "pending" };
            return (
              <li
                key={step.id}
                className="flex items-start gap-2 leading-snug"
                aria-current={status.kind === "running" ? "step" : undefined}
              >
                <StepIcon status={status} />
                <div className="flex-1 min-w-0">
                  <div className="truncate">{step.summary}</div>
                  {status.kind === "running" && status.message && (
                    <div className="text-xs opacity-60 tabular-nums">
                      {status.message}
                      {status.progress > 0 && ` · ${Math.round(status.progress * 100)}%`}
                    </div>
                  )}
                  {status.kind === "failed" && (
                    <div className="text-xs text-rose-700">
                      {status.retrying ? "Retrying… " : "Failed: "}
                      {status.error}
                    </div>
                  )}
                </div>
              </li>
            );
          })}
        </ol>
      )}

      {/* Re-planning notice */}
      {state.rePlanReason && (
        <div className="px-4 py-2 text-xs border-t border-current/10 bg-current/5 flex items-center gap-2">
          <RotateCw className="size-3 animate-spin" />
          Adapting plan: {state.rePlanReason}
        </div>
      )}

      {/* Verification result */}
      {state.verifyDiscrepancies !== null && (
        <div className="px-4 py-2 text-xs border-t border-current/10">
          {state.verifyDiscrepancies.length === 0 ? (
            <span className="text-emerald-700">✓ Verification passed</span>
          ) : (
            <details>
              <summary className="text-amber-700 cursor-pointer">
                {state.verifyDiscrepancies.length} discrepancies found
              </summary>
              <ul className="mt-1 ml-3 list-disc space-y-0.5">
                {state.verifyDiscrepancies.map((d, i) => (
                  <li key={i}>{d}</li>
                ))}
              </ul>
            </details>
          )}
        </div>
      )}

      {/* Terminal panel */}
      <div className="px-4 py-2.5 border-t border-current/10 flex items-center justify-between gap-3">
        <div className="flex-1 text-xs">
          {state.terminal.kind === "complete" && (
            <span className="text-emerald-800">
              {state.terminal.summary} · {state.terminal.toolCalls} tool calls ·{" "}
              {(state.terminal.wallClockMs / 1000).toFixed(1)}s
            </span>
          )}
          {state.terminal.kind === "aborted" && (
            <span className="text-rose-800">
              Aborted: {state.terminal.reason}
              {state.terminal.partialLedgerEntries.length > 0 &&
                ` · ${state.terminal.partialLedgerEntries.length} partial write(s) recorded`}
            </span>
          )}
          {state.terminal.kind === "running" && (
            <span className="opacity-60">
              Running autonomously — you can let it finish or cancel.
            </span>
          )}
        </div>

        <div className="flex gap-2">
          {isRunning && onCancel && (
            <Button
              size="sm"
              variant="outline"
              onClick={onCancel}
              className="text-xs"
            >
              Cancel
            </Button>
          )}
          {isComplete &&
            state.terminal.kind === "complete" &&
            onUndo && (
              <Button
                size="sm"
                variant="outline"
                onClick={() =>
                  state.terminal.kind === "complete" &&
                  onUndo(state.terminal.undoBatchId)
                }
                className="text-xs"
              >
                Undo
              </Button>
            )}
          {(isAborted || isComplete) && state.terminal.kind !== "running" && (
            <span className="text-xs opacity-50">
              {state.terminal.kind === "complete" && "Done"}
            </span>
          )}
        </div>
      </div>
    </div>
  );
}

// ─── Helpers ────────────────────────────────────────────────────────

function StepIcon({ status }: { status: StepStatus }) {
  switch (status.kind) {
    case "pending":
      return <Circle className="size-4 mt-0.5 opacity-40 shrink-0" />;
    case "running":
      return <Loader2 className="size-4 mt-0.5 animate-spin text-amber-700 shrink-0" />;
    case "done":
      return <CheckCircle2 className="size-4 mt-0.5 text-emerald-700 shrink-0" />;
    case "failed":
      return <AlertCircle className="size-4 mt-0.5 text-rose-700 shrink-0" />;
  }
}

function formatElapsed(seconds: number): string {
  if (seconds < 60) return `${Math.floor(seconds)}s`;
  const m = Math.floor(seconds / 60);
  const s = Math.floor(seconds % 60);
  return `${m}:${s.toString().padStart(2, "0")}`;
}

// ─── Stream collector hook ──────────────────────────────────────────

/**
 * Stateful collector that buffers AgentInnerEvent's as they arrive
 * on the SSE stream. Pass the result into AgentProgressCard.
 *
 * Parent component shape:
 *
 *   const { events, ingest, reset, runId } = useAgentEventStream();
 *
 *   // when an AiStreamEvent with type === "agent" lands:
 *   ingest(streamEvent.event);
 *
 *   if (runId) {
 *     return <AgentProgressCard events={events} ... />;
 *   }
 */
export function useAgentEventStream() {
  type Action =
    | { kind: "ingest"; event: AgentInnerEvent }
    | { kind: "reset" };
  const [state, dispatch] = useReducer(
    (s: { events: AgentInnerEvent[] }, a: Action) =>
      a.kind === "reset" ? { events: [] } : { events: [...s.events, a.event] },
    { events: [] }
  );
  const runId = state.events.find((e) => "runId" in e)?.runId ?? null;
  return {
    events: state.events,
    ingest: (event: AgentInnerEvent) => dispatch({ kind: "ingest", event }),
    reset: () => dispatch({ kind: "reset" }),
    runId,
  };
}

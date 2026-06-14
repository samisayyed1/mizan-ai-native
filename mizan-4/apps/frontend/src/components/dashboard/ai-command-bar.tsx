/**
 * AI Command Bar — inline streaming variant (Sami feedback 2026-06-14).
 *
 * Pinned full-width input on the dashboard. Per Sami's direction the
 * bar now answers IN PLACE — the response streams below the input on
 * the dashboard itself, no navigation, no context switch. An "Open in
 * full chat" affordance escalates to `/assistant` when the user wants
 * the persistent thread + multi-turn history.
 *
 * Routing:
 *   - Submit Enter / click send  → inline streaming response on the
 *     dashboard via the SAME `streamChatResponse` the assistant page
 *     uses (no duplicate dispatcher, no skew). Single-turn — the
 *     thread is auto-created server-side so "Open in chat" can
 *     resume the conversation.
 *   - Click "Open in full chat" → navigate to `/assistant` with the
 *     thread_id pre-loaded so the user sees their dashboard answer
 *     plus the full history surface.
 *   - Voice button → still navigates to `/assistant?voice=1` until
 *     PR-C18 wires real transcription.
 *
 * Why this shape: per Sami "when I type in dashboard it should respond
 * THAT area only strictly" — context switching to /assistant breaks
 * the analytical flow ("looking at my net worth → ask why it
 * dropped → answer appears next to the number, not on a different
 * page").
 *
 * Voice button is unchanged (still navigates) because dictation
 * surface is qualitatively different from typing — a dedicated chat
 * surface is the right home for an extended voice session.
 */
import { Icons } from "@mizan/ui/components/ui/icons";
import { type FormEvent, useCallback, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";

import { streamChatResponse } from "@/features/ai-assistant/api";
import type { AiStreamEvent } from "@/features/ai-assistant/types";

export interface AiCommandBarProps {
  /** Override the destination route for "Open in full chat". Defaults to `/assistant`. */
  toRoute?: string;
  /** Placeholder text shown when the input is empty. */
  placeholder?: string;
  /** Optional initial value (e.g. when re-mounting with a deep link). */
  initialValue?: string;
  /** Hidden from screen readers when the page already has an h1 for the bar. */
  ariaLabel?: string;
  /**
   * When `true`, the bar pins to the top of its scroll container via
   * `sticky top-0 z-30`. Use only for the canonical full-width hero
   * placement. The sidebar variant (ADR 0018b·v3) sets this `false`
   * because the sidebar is already sticky and double-stick reads as
   * jitter inside a narrow column.
   */
  pinned?: boolean;
}

/**
 * Suggested starter prompts — surfaced as chip buttons below the input
 * when it's empty AND no answer is currently shown. Track §23-themed
 * so the first-time user sees the promises in §23 made concrete.
 */
const SUGGESTIONS: { label: string; prompt: string }[] = [
  { label: "What's my Zakat?", prompt: "What's my Zakat this year?" },
  { label: "Show net worth this week", prompt: "Show me my net worth this week" },
  { label: "Bonds by maturity", prompt: "Show my Sukuks and bonds by maturity year" },
  { label: "Cash flow this quarter", prompt: "What did my cash flow look like this quarter?" },
];

interface InlineAnswer {
  prompt: string;
  /** Accumulated text deltas from the stream. */
  text: string;
  /** Server-assigned thread id (set after the `system` event). */
  threadId?: string;
  /** Set when the stream errors mid-flight. Prompt + text are preserved. */
  error?: string;
  /** True while a response is actively streaming. */
  isStreaming: boolean;
}

export function AiCommandBar({
  toRoute = "/assistant",
  placeholder = "Ask Mizan anything — Zakat, net worth, holdings, plans…",
  initialValue = "",
  ariaLabel = "Ask Mizan",
  pinned = true,
}: AiCommandBarProps) {
  const [value, setValue] = useState(initialValue);
  const [answer, setAnswer] = useState<InlineAnswer | null>(null);
  const abortRef = useRef<AbortController | null>(null);
  const navigate = useNavigate();

  const runInline = useCallback(async (prompt: string) => {
    const trimmed = prompt.trim();
    if (!trimmed) return;

    // Cancel any in-flight previous stream — last question wins.
    abortRef.current?.abort();
    const controller = new AbortController();
    abortRef.current = controller;

    setAnswer({ prompt: trimmed, text: "", isStreaming: true });

    try {
      for await (const event of streamChatResponse(
        { content: trimmed },
        controller.signal,
      ) as AsyncGenerator<AiStreamEvent>) {
        if (controller.signal.aborted) break;
        // The stream emits a small vocabulary of events. We only
        // need three on the dashboard surface: `system` for the
        // thread id (so "Open in full chat" can deep-link), `textDelta`
        // for the incremental answer, and `error` to surface failures
        // cleanly. Tool-call events show as inline "thinking" rather
        // than the rich card the assistant page renders — the
        // dashboard is a glance surface, not a full chat.
        if (event.type === "system") {
          setAnswer((prev) =>
            prev ? { ...prev, threadId: event.threadId } : prev,
          );
        } else if (event.type === "textDelta") {
          setAnswer((prev) =>
            prev ? { ...prev, text: prev.text + event.delta } : prev,
          );
        } else if (event.type === "error") {
          setAnswer((prev) =>
            prev ? { ...prev, error: event.message, isStreaming: false } : prev,
          );
        }
      }
    } catch (err) {
      // AbortError when a new question lands mid-stream is expected
      // and intentional — ignore so we don't paint a scary toast.
      if (err instanceof DOMException && err.name === "AbortError") return;
      const message =
        err instanceof Error ? err.message : "Failed to reach the AI service.";
      setAnswer((prev) =>
        prev ? { ...prev, error: message, isStreaming: false } : prev,
      );
      return;
    }

    setAnswer((prev) => (prev ? { ...prev, isStreaming: false } : prev));
  }, []);

  const handleSubmit = (e: FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    void runInline(value);
  };

  const handleSuggestionClick = (prompt: string) => {
    setValue(prompt);
    void runInline(prompt);
  };

  const handleOpenFullChat = () => {
    // Deep-link with the thread id if we have one — the assistant
    // page resumes from the server-persisted thread, so the user
    // doesn't lose context they just typed.
    const params = new URLSearchParams({ intent: "command" });
    if (answer?.threadId) params.set("thread", answer.threadId);
    else if (answer?.prompt) params.set("prompt", answer.prompt);
    else if (value.trim()) params.set("prompt", value.trim());
    navigate(`${toRoute}?${params.toString()}`);
  };

  const handleVoiceClick = () => {
    // Voice still routes to the assistant page — see file-level
    // comment. PR-C18 wires real transcription.
    navigate(`${toRoute}?voice=1`);
  };

  const handleNew = () => {
    abortRef.current?.abort();
    setAnswer(null);
    setValue("");
  };

  return (
    <section
      aria-label={ariaLabel}
      className={`bg-card flex flex-col gap-1.5 rounded-xl border px-3 py-2 shadow-sm ${pinned ? "sticky top-0 z-30" : ""}`}
      data-testid="ai-command-bar"
    >
      <form onSubmit={handleSubmit} className="flex items-center gap-2">
        <Icons.Sparkles
          className="text-muted-foreground ml-1 h-4 w-4 shrink-0"
          aria-hidden="true"
        />
        <input
          type="text"
          value={value}
          onChange={(e) => setValue(e.target.value)}
          placeholder={placeholder}
          aria-label={ariaLabel}
          className="bg-transparent text-foreground placeholder:text-muted-foreground flex-1 px-1 py-1 text-sm focus:outline-none"
          data-testid="ai-command-bar-input"
          disabled={answer?.isStreaming === true}
        />
        <button
          type="button"
          onClick={handleVoiceClick}
          aria-label="Use voice"
          className="text-muted-foreground hover:bg-muted hover:text-foreground flex h-8 w-8 shrink-0 items-center justify-center rounded-full transition-colors"
          data-testid="ai-command-bar-voice"
        >
          {/* Inline mic SVG — the Icons registry doesn't expose a Mic
              variant yet; switching to it is a one-line follow-up when
              a downstream PR adds it. */}
          <svg
            xmlns="http://www.w3.org/2000/svg"
            width="16"
            height="16"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
            aria-hidden="true"
          >
            <path d="M12 2a3 3 0 0 0-3 3v7a3 3 0 0 0 6 0V5a3 3 0 0 0-3-3Z" />
            <path d="M19 10v2a7 7 0 0 1-14 0v-2" />
            <line x1="12" x2="12" y1="19" y2="22" />
          </svg>
        </button>
        <button
          type="submit"
          aria-label={answer?.isStreaming ? "Streaming…" : "Send to Mizan"}
          disabled={value.trim().length === 0 || answer?.isStreaming === true}
          className="bg-primary text-primary-foreground hover:bg-primary/90 disabled:bg-muted disabled:text-muted-foreground flex h-8 w-8 shrink-0 items-center justify-center rounded-full transition-colors"
          data-testid="ai-command-bar-submit"
        >
          {answer?.isStreaming ? (
            <Icons.Spinner className="h-4 w-4 animate-spin" />
          ) : (
            <Icons.ArrowUp className="h-4 w-4" />
          )}
        </button>
      </form>

      {/* Suggestions: visible only when the input is empty AND no
          answer is rendered. Once the user starts typing or has an
          answer on screen, the chips would compete with the response
          for attention. */}
      {value.trim().length === 0 && !answer && (
        <div
          className="flex flex-wrap gap-1.5"
          aria-label="Suggested prompts"
          data-testid="ai-command-bar-suggestions"
        >
          {SUGGESTIONS.map((s) => (
            <button
              key={s.label}
              type="button"
              onClick={() => handleSuggestionClick(s.prompt)}
              className="bg-muted text-muted-foreground hover:bg-muted/80 hover:text-foreground rounded-full px-2.5 py-0.5 text-xs transition-colors"
            >
              {s.label}
            </button>
          ))}
        </div>
      )}

      {/* Inline answer pane. Mirrors the small-card aesthetic of the
          dashboard's other "tile" surfaces so it feels native to the
          page rather than a chat overlay. */}
      {answer && (
        <div
          className="border-border/60 bg-muted/30 mt-1 flex flex-col gap-2 rounded-lg border px-3 py-2"
          data-testid="ai-command-bar-answer"
          aria-live="polite"
        >
          <div className="text-muted-foreground flex items-center justify-between text-xs uppercase tracking-wide">
            <span>Mizan{answer.isStreaming ? " is thinking…" : ""}</span>
            <div className="flex items-center gap-2">
              <button
                type="button"
                onClick={handleOpenFullChat}
                className="hover:text-foreground transition-colors"
                data-testid="ai-command-bar-open-full"
              >
                Open in full chat →
              </button>
              <button
                type="button"
                onClick={handleNew}
                aria-label="Clear answer"
                className="hover:text-foreground transition-colors"
                data-testid="ai-command-bar-clear"
              >
                ×
              </button>
            </div>
          </div>
          {answer.error ? (
            <p className="text-destructive text-sm" data-testid="ai-command-bar-error">
              {answer.error}
            </p>
          ) : (
            <p
              className="text-foreground whitespace-pre-wrap text-sm leading-relaxed"
              data-testid="ai-command-bar-response"
            >
              {answer.text}
              {answer.isStreaming && (
                <span className="bg-foreground/60 ml-0.5 inline-block h-3 w-1 animate-pulse align-middle" />
              )}
            </p>
          )}
        </div>
      )}
    </section>
  );
}

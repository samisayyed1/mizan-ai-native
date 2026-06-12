/**
 * AI Command Bar — Track A PR-A6 / Goal v3 §V Phase 1 step A6.
 *
 * Pinned, full-width persistent input on the dashboard. Captures a
 * natural-language question (Zakat, net worth, panel drill, anything)
 * and hands it off to the existing `/assistant` route with the prompt
 * pre-seeded. The voice button routes to `/assistant?voice=1` until
 * C-track multi-modal lands (PR-C18+).
 *
 * §23 step advanced: this is the surface through which the Singapore
 * millionaire types or speaks *"What's my Zakat this year?"* and gets
 * routed to the agent that answers it. Track C tools (PR-C4 series)
 * fill in the answer pipeline; this PR delivers the entry point.
 *
 * Design choices:
 *   - We DON'T duplicate the chat composer inline on the dashboard.
 *     One canonical chat surface (the assistant page) reduces
 *     state-sync bugs and keeps the dashboard fast.
 *   - The bar reads `?prompt=` round-trips so deep-linking works.
 *   - Voice button is a placeholder navigation target — C-track wires
 *     real voice transcription via `multimodal/voice.rs` (PR-C18).
 *   - "Does not collapse on scroll" per Goal v3 §V step A6 — achieved
 *     via the `sticky` + `top-0` + `z-30` classes when the parent
 *     container is scrollable; on the dashboard the bar is rendered
 *     above the strip so it's persistent in the layout.
 *
 * Future PRs (tracked, NOT in this skeleton):
 *   - PR-A6.b — escalate to `/v1/ai/agent` SSE for Gold-tier complex
 *     queries (depends on C3.b dispatcher unification)
 *   - PR-C18 — real voice transcription replacing the navigation
 *   - PR-A6.c — slash-commands surface (e.g. `/zakat`, `/net-worth`,
 *     `/sukuks`) that bypass the LLM for known intents
 */
import { Icons } from "@mizan/ui/components/ui/icons";
import { type FormEvent, useState } from "react";
import { useNavigate } from "react-router-dom";

export interface AiCommandBarProps {
  /** Override the destination route. Defaults to `/assistant`. */
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
 * when it's empty. Track §23-themed so the first-time user sees the
 * promises in §23 made concrete.
 */
const SUGGESTIONS: { label: string; prompt: string }[] = [
  { label: "What's my Zakat?", prompt: "What's my Zakat this year?" },
  { label: "Show net worth this week", prompt: "Show me my net worth this week" },
  { label: "Bonds by maturity", prompt: "Show my Sukuks and bonds by maturity year" },
  { label: "Cash flow this quarter", prompt: "What did my cash flow look like this quarter?" },
];

export function AiCommandBar({
  toRoute = "/assistant",
  placeholder = "Ask Mizan anything — Zakat, net worth, holdings, plans…",
  initialValue = "",
  ariaLabel = "Ask Mizan",
  pinned = true,
}: AiCommandBarProps) {
  const [value, setValue] = useState(initialValue);
  const navigate = useNavigate();

  const submit = (prompt: string) => {
    const trimmed = prompt.trim();
    if (!trimmed) return;
    const params = new URLSearchParams({
      intent: "command",
      prompt: trimmed,
    });
    navigate(`${toRoute}?${params.toString()}`);
  };

  const handleSubmit = (e: FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    submit(value);
  };

  const handleVoiceClick = () => {
    // Voice navigation surfaces the existing assistant page with the
    // voice flag; PR-C18 wires real transcription. Until then this
    // routes the user to the chat where they can dictate via the
    // composer's existing mic affordance.
    navigate(`${toRoute}?voice=1`);
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
          aria-label="Send to Mizan"
          disabled={value.trim().length === 0}
          className="bg-primary text-primary-foreground hover:bg-primary/90 disabled:bg-muted disabled:text-muted-foreground flex h-8 w-8 shrink-0 items-center justify-center rounded-full transition-colors"
          data-testid="ai-command-bar-submit"
        >
          <Icons.ArrowUp className="h-4 w-4" />
        </button>
      </form>

      {value.trim().length === 0 && (
        <div
          className="flex flex-wrap gap-1.5"
          aria-label="Suggested prompts"
          data-testid="ai-command-bar-suggestions"
        >
          {SUGGESTIONS.map((s) => (
            <button
              key={s.label}
              type="button"
              onClick={() => submit(s.prompt)}
              className="bg-muted text-muted-foreground hover:bg-muted/80 hover:text-foreground rounded-full px-2.5 py-0.5 text-xs transition-colors"
            >
              {s.label}
            </button>
          ))}
        </div>
      )}
    </section>
  );
}

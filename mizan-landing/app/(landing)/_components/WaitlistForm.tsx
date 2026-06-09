"use client";

import { useEffect, useState } from "react";
import { motion, AnimatePresence, useReducedMotion } from "framer-motion";
import { Check, Copy, Loader2, MessageSquareWarning } from "lucide-react";

import { Button } from "@/app/(landing)/_primitives/Button";
import { cn } from "@/lib/cn";
import { COUNTRIES, type Country, type WaitlistResponse } from "@/lib/schemas";

type Status = "idle" | "submitting" | "success" | "error";

declare global {
  interface Window {
    plausible?: (event: string, opts?: { props?: Record<string, unknown> }) => void;
  }
}

export function WaitlistForm() {
  const reduce = useReducedMotion();
  const [status, setStatus] = useState<Status>("idle");
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<WaitlistResponse | null>(null);
  const [copied, setCopied] = useState(false);
  const [refFromUrl, setRefFromUrl] = useState<string | undefined>();

  useEffect(() => {
    // Pull `?ref=` from the URL on mount so referred signups carry
    // attribution. Cookie persistence is not used in v1 — keeping
    // surface area tight pre-launch.
    const params = new URLSearchParams(window.location.search);
    const ref = params.get("ref");
    if (ref && ref.length === 8) setRefFromUrl(ref);
  }, []);

  async function onSubmit(e: React.FormEvent<HTMLFormElement>) {
    e.preventDefault();
    setStatus("submitting");
    setError(null);

    const formData = new FormData(e.currentTarget);
    const payload = {
      email: String(formData.get("email") ?? ""),
      country: String(formData.get("country") ?? "Other"),
      painPoint: String(formData.get("painPoint") ?? "") || undefined,
      ref: refFromUrl,
    };

    try {
      const res = await fetch("/api/waitlist", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(payload),
      });

      if (!res.ok) {
        const body = await res.json().catch(() => ({}));
        throw new Error(body?.error ?? `Submit failed (${res.status})`);
      }

      const data = (await res.json()) as WaitlistResponse;
      setResult(data);
      setStatus("success");
      window.plausible?.("waitlist_signup", {
        props: {
          country: payload.country,
          hasReferrer: Boolean(refFromUrl),
        },
      });
    } catch (err) {
      setError(err instanceof Error ? err.message : "Something went wrong");
      setStatus("error");
    }
  }

  return (
    <AnimatePresence mode="wait" initial={false}>
      {status === "success" && result ? (
        <motion.div
          key="success"
          initial={reduce ? false : { opacity: 0, scale: 0.96 }}
          animate={{ opacity: 1, scale: 1 }}
          transition={{ duration: 0.25, ease: [0.25, 0.46, 0.45, 0.94] }}
        >
          <ConfirmationCard result={result} copied={copied} setCopied={setCopied} />
        </motion.div>
      ) : (
        <motion.form
          key="form"
          onSubmit={onSubmit}
          initial={reduce ? false : { opacity: 0, scale: 0.98 }}
          animate={{ opacity: 1, scale: 1 }}
          transition={{ duration: 0.2, ease: [0.25, 0.46, 0.45, 0.94] }}
          className="space-y-4"
          aria-busy={status === "submitting"}
        >
          <div className="grid gap-3 md:grid-cols-[1fr_auto]">
            <Field
              name="email"
              type="email"
              required
              autoComplete="email"
              placeholder="you@example.com"
              ariaLabel="Email address"
              inputMode="email"
            />
            <Select
              name="country"
              required
              defaultValue="UK"
              ariaLabel="Country"
            />
          </div>
          <Field
            name="painPoint"
            placeholder="What's your biggest wealth-tracking headache? (optional)"
            ariaLabel="Pain point"
            maxLength={280}
          />
          <Button
            type="submit"
            variant="primary"
            size="lg"
            className="w-full"
            disabled={status === "submitting"}
          >
            {status === "submitting" ? (
              <>
                <Loader2 className="h-4 w-4 animate-spin" />
                Reserving…
              </>
            ) : (
              <>Reserve my seat →</>
            )}
          </Button>
          {error ? (
            <p
              className="t-caption text-destructive flex items-center gap-2"
              role="alert"
              aria-live="polite"
            >
              <MessageSquareWarning className="h-4 w-4" />
              {error}
            </p>
          ) : null}
          <p className="t-micro text-foreground/55">
            No newsletter. One email when we launch. Easy unsubscribe.
          </p>
        </motion.form>
      )}
    </AnimatePresence>
  );
}

function Field({
  name,
  type = "text",
  required = false,
  autoComplete,
  placeholder,
  ariaLabel,
  inputMode,
  maxLength,
}: {
  name: string;
  type?: string;
  required?: boolean;
  autoComplete?: string;
  placeholder?: string;
  ariaLabel: string;
  inputMode?: React.InputHTMLAttributes<HTMLInputElement>["inputMode"];
  maxLength?: number;
}) {
  return (
    <input
      name={name}
      type={type}
      required={required}
      autoComplete={autoComplete}
      placeholder={placeholder}
      aria-label={ariaLabel}
      inputMode={inputMode}
      maxLength={maxLength}
      className={cn(
        "h-14 w-full rounded-xl border border-depth-border bg-depth-elevated px-4",
        "text-[16px] text-foreground placeholder:text-foreground/40",
        "focus:border-gold-primary/60 focus:bg-depth-elevated focus:outline-none focus:ring-2 focus:ring-gold-primary/40",
        "transition-colors duration-150",
      )}
    />
  );
}

function Select({
  name,
  required = false,
  defaultValue,
  ariaLabel,
}: {
  name: string;
  required?: boolean;
  defaultValue?: Country;
  ariaLabel: string;
}) {
  return (
    <select
      name={name}
      required={required}
      defaultValue={defaultValue}
      aria-label={ariaLabel}
      className={cn(
        "h-14 rounded-xl border border-depth-border bg-depth-elevated px-4",
        "text-[16px] text-foreground appearance-none cursor-pointer",
        "focus:border-gold-primary/60 focus:outline-none focus:ring-2 focus:ring-gold-primary/40",
        "transition-colors duration-150",
        // Custom caret via background image — single inline SVG, no extra HTTP.
        "bg-[url('data:image/svg+xml;utf8,<svg%20xmlns=%22http://www.w3.org/2000/svg%22%20width=%2212%22%20height=%2212%22%20viewBox=%220%200%2024%2024%22%20fill=%22none%22%20stroke=%22%23ccc%22%20stroke-width=%222%22%20stroke-linecap=%22round%22%20stroke-linejoin=%22round%22><polyline%20points=%226%209%2012%2015%2018%209%22/></svg>')] bg-[length:12px_12px] bg-no-repeat bg-[right_1rem_center] pr-10",
      )}
    >
      {COUNTRIES.map((c) => (
        <option key={c} value={c} className="bg-depth-elevated">
          {c}
        </option>
      ))}
    </select>
  );
}

function ConfirmationCard({
  result,
  copied,
  setCopied,
}: {
  result: WaitlistResponse;
  copied: boolean;
  setCopied: (v: boolean) => void;
}) {
  const inviteUrl = `https://getmizan.net/i/${result.refCode}`;

  async function copy() {
    try {
      await navigator.clipboard.writeText(inviteUrl);
      setCopied(true);
      window.plausible?.("referral_share", { props: { channel: "copy" } });
      setTimeout(() => setCopied(false), 2000);
    } catch {
      // ignored
    }
  }

  async function shareNative() {
    try {
      if (navigator.share) {
        await navigator.share({
          title: "Mizan",
          text: "I just reserved my seat for Mizan — AI-native wealth, built for the Muslim affluent.",
          url: inviteUrl,
        });
        window.plausible?.("referral_share", { props: { channel: "native" } });
      }
    } catch {
      // user cancelled — ignore
    }
  }

  const shareText = encodeURIComponent(
    "Just reserved my seat for Mizan — AI-native wealth tracking, Zakat across all four schools, audit-grade provenance. ",
  );

  return (
    <div className="space-y-6 text-center">
      <div className="space-y-2">
        <p className="t-micro text-gold-primary">YOU&apos;RE IN</p>
        <h3 className="font-serif text-2xl font-bold text-gold-cream md:text-3xl">
          Welcome, founding member{" "}
          <span className="tabular-nums text-gold-primary">
            #{result.position}
          </span>
        </h3>
        <p className="t-body text-foreground/75">
          When we launch in August, you&apos;ll get the founding price for life.
        </p>
      </div>

      <div className="space-y-3 rounded-xl border border-depth-border bg-depth-page p-4">
        <p className="t-micro text-foreground/55">Your invite link</p>
        <div className="flex items-center justify-between gap-3 rounded-lg bg-depth-elevated px-3 py-2">
          <span className="truncate t-body font-mono text-foreground/85">
            {inviteUrl}
          </span>
          <button
            type="button"
            onClick={copy}
            aria-label="Copy invite link"
            className="inline-flex h-9 items-center gap-1.5 rounded-md border border-depth-border bg-depth-card px-3 t-caption text-foreground hover:bg-depth-elevated transition-colors"
          >
            {copied ? (
              <>
                <Check className="h-4 w-4 text-success" /> Copied
              </>
            ) : (
              <>
                <Copy className="h-4 w-4" /> Copy
              </>
            )}
          </button>
        </div>
        <p className="t-micro text-foreground/55">
          Share if a friend would want this. No leaderboard, no position bumps — we don&apos;t play those games.
        </p>
      </div>

      <div className="flex flex-wrap items-center justify-center gap-2">
        {typeof navigator !== "undefined" && "share" in navigator ? (
          <Button variant="ghost" size="sm" onClick={shareNative}>
            Share via…
          </Button>
        ) : null}
        <a
          href={`https://twitter.com/intent/tweet?text=${shareText}&url=${encodeURIComponent(
            inviteUrl,
          )}`}
          target="_blank"
          rel="noopener noreferrer"
          className="inline-flex h-9 items-center gap-1.5 rounded-md border border-depth-border bg-depth-card px-3 t-caption text-foreground hover:bg-depth-elevated transition-colors"
          onClick={() =>
            window.plausible?.("referral_share", { props: { channel: "x" } })
          }
        >
          Share on X
        </a>
        <a
          href={`https://wa.me/?text=${shareText}${encodeURIComponent(inviteUrl)}`}
          target="_blank"
          rel="noopener noreferrer"
          className="inline-flex h-9 items-center gap-1.5 rounded-md border border-depth-border bg-depth-card px-3 t-caption text-foreground hover:bg-depth-elevated transition-colors"
          onClick={() =>
            window.plausible?.("referral_share", { props: { channel: "whatsapp" } })
          }
        >
          WhatsApp
        </a>
        <a
          href={`mailto:?subject=Mizan%20waitlist&body=${shareText}${encodeURIComponent(inviteUrl)}`}
          className="inline-flex h-9 items-center gap-1.5 rounded-md border border-depth-border bg-depth-card px-3 t-caption text-foreground hover:bg-depth-elevated transition-colors"
          onClick={() =>
            window.plausible?.("referral_share", { props: { channel: "email" } })
          }
        >
          Email
        </a>
      </div>
    </div>
  );
}

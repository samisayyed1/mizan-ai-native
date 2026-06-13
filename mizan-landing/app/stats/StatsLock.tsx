"use client";

import { useState } from "react";
import { Lock } from "lucide-react";

import { Wordmark } from "@/app/(landing)/_primitives/Wordmark";

export function StatsLock() {
  const [key, setKey] = useState("");

  function submit(e: React.FormEvent) {
    e.preventDefault();
    if (!key.trim()) return;
    window.location.href = `/stats?key=${encodeURIComponent(key.trim())}`;
  }

  return (
    <main className="flex min-h-screen flex-col items-center justify-center gap-6 bg-depth-page px-6 text-center">
      <Wordmark size="sm" />
      <span className="inline-flex h-12 w-12 items-center justify-center rounded-2xl border border-depth-border bg-depth-card text-gold-primary">
        <Lock className="h-5 w-5" />
      </span>
      <div className="space-y-1">
        <h1 className="font-serif text-2xl font-bold text-gold-cream">
          Insider access
        </h1>
        <p className="t-caption text-foreground/55">
          This page is private. Enter your access key to continue.
        </p>
      </div>
      <form onSubmit={submit} className="flex w-full max-w-xs flex-col gap-3">
        <input
          type="password"
          value={key}
          onChange={(e) => setKey(e.target.value)}
          placeholder="Access key"
          aria-label="Access key"
          className="h-12 rounded-xl border border-depth-border bg-depth-elevated px-4 text-[16px] text-foreground placeholder:text-foreground/40 focus:border-gold-primary/60 focus:outline-none focus:ring-2 focus:ring-gold-primary/40"
        />
        <button
          type="submit"
          className="h-12 rounded-xl bg-gold-primary font-semibold text-depth-page transition-colors hover:bg-gold-cream"
        >
          Unlock
        </button>
      </form>
    </main>
  );
}

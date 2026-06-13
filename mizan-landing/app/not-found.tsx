import Link from "next/link";

import { Wordmark } from "./(landing)/_primitives/Wordmark";

export default function NotFound() {
  return (
    <main className="flex min-h-screen flex-col items-center justify-center gap-6 bg-depth-page px-6 text-center">
      <Wordmark size="sm" />
      <p className="t-micro text-gold-deep">404</p>
      <h1 className="font-serif text-4xl font-bold text-gold-cream md:text-5xl">
        Nothing here yet.
      </h1>
      <p className="max-w-md text-base text-foreground/75">
        That page doesn&apos;t exist. Head back to the waitlist — Mizan launches August 2026.
      </p>
      <Link
        href="/"
        className="t-caption rounded-xl border border-depth-border bg-depth-card px-5 py-3 text-foreground transition-colors hover:bg-depth-elevated"
      >
        Back to home →
      </Link>
    </main>
  );
}

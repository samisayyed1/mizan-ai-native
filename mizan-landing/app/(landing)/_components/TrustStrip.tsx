import { FileCheck2, Lock, Scale, ShieldCheck } from "lucide-react";

import { Container } from "@/app/(landing)/_primitives/Container";
import { RevealOnScroll } from "./RevealOnScroll";

/**
 * Trust strip — sits between Hero and Problem. Surfaces the four
 * concrete claims that flip "marketing fluff" → "real product":
 * encryption, audit ledger, all four madhāhib, AAOIFI screening.
 *
 * Pattern researched from Stripe / Vercel: trust signals belong
 * immediately after hero, above the fold on standard laptop widths.
 */
const TRUST = [
  {
    icon: ShieldCheck,
    title: "AES-256-GCM encryption",
    sub: "Per-provider token encryption",
  },
  {
    icon: FileCheck2,
    title: "Hash-chained ledger",
    sub: "Every figure verifiable",
  },
  {
    icon: Scale,
    title: "All four madhāhib",
    sub: "Hanafi · Shafi'i · Maliki · Hanbali",
  },
  {
    icon: Lock,
    title: "AAOIFI-graded screening",
    sub: "Compliance per ADR 0012",
  },
] as const;

export function TrustStrip() {
  return (
    <section
      aria-label="Trust signals"
      className="border-y border-depth-border bg-depth-container py-8 md:py-10"
    >
      <Container>
        <RevealOnScroll
          stagger
          className="grid grid-cols-2 gap-6 md:grid-cols-4 md:gap-8"
        >
          {TRUST.map(({ icon: Icon, title, sub }) => (
            <div key={title} className="flex items-start gap-3">
              <span
                aria-hidden="true"
                className="flex h-10 w-10 shrink-0 items-center justify-center rounded-full border border-depth-border bg-depth-card"
              >
                <Icon className="h-4 w-4 text-gold-primary" />
              </span>
              <div className="min-w-0 space-y-1">
                <p className="t-body-bold text-foreground/95">{title}</p>
                <p className="t-micro text-gold-deep">{sub}</p>
              </div>
            </div>
          ))}
        </RevealOnScroll>
      </Container>
    </section>
  );
}

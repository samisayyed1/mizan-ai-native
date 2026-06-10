import { FileCheck2, Globe2, Layers, ShieldCheck } from "lucide-react";

import { Container } from "@/app/(landing)/_primitives/Container";
import { RevealOnScroll } from "./RevealOnScroll";

/**
 * Trust strip — sits between Hero and Problem. Four concrete claims
 * that flip "marketing fluff" → "real product".
 *
 * Pattern researched from Stripe / Vercel: trust signals belong
 * immediately after hero, above the fold on standard laptop widths.
 */
const TRUST = [
  {
    icon: Layers,
    title: "12 asset classes",
    sub: "Equities, crypto, sukuks, gold, property & more",
  },
  {
    icon: Globe2,
    title: "9 jurisdictions",
    sub: "US · UK · EU · GCC · SG · MY · IN · CA · AU",
  },
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

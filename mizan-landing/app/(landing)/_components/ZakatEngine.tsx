import { Check } from "lucide-react";

import { Container } from "@/app/(landing)/_primitives/Container";
import { Eyebrow } from "@/app/(landing)/_primitives/Eyebrow";
import { RevealOnScroll } from "./RevealOnScroll";

const CLAIMS = [
  "All four schools with full edge-case handling",
  "Hawl tracking per asset cohort in lunar years",
  "One-tap pay to verified charities · receipt in 60 seconds",
] as const;

/**
 * Zakat engine — the only section with any gold tint anywhere on the
 * page. Subtle ellipse gradient from top, ~8% opacity. Keeps the gold
 * sacred to the moat moment.
 */
export function ZakatEngine() {
  return (
    <section
      className="relative isolate overflow-hidden py-20 md:py-32 bg-depth-page"
    >
      <div
        aria-hidden="true"
        className="absolute inset-0 pointer-events-none"
        style={{
          background:
            "radial-gradient(ellipse 80% 60% at 50% 0%, hsl(31 49% 64% / 0.08), transparent 70%)",
        }}
      />
      <Container className="relative">
        <RevealOnScroll className="mx-auto max-w-3xl space-y-6 text-center">
          <Eyebrow tone="primary">THE ZAKAT ENGINE</Eyebrow>
          <h2
            className="font-serif font-bold tracking-tight text-gold-cream"
            style={{ fontSize: "clamp(28px, 4vw, 44px)", lineHeight: 1.1 }}
          >
            The first Zakat that&apos;s actually correct.
          </h2>
          <div className="space-y-5 text-left md:text-center">
            <p className="t-body text-foreground/85 text-base md:text-lg leading-relaxed">
              Hanafi, Shafi&apos;i, Maliki, Hanbali. All four schools live, all four edge cases handled in <code className="font-mono text-gold-cream/90 text-[0.95em]">mizan-zakat</code> (Rust). Real Hawl tracking per asset cohort in lunar years. Nisab in current silver or gold spot from MetalpriceAPI. Locked-retirement disagreement (Hanbali&apos;s two-views ruling) surfaced, not hidden. Investment property routed through Maliki&apos;s intent-based classification when you select that school. Long-term mortgages deducted when your school allows it.
            </p>
            <p className="t-body text-foreground/85 text-base md:text-lg leading-relaxed">
              Every assessment writes a <code className="font-mono text-gold-cream/90 text-[0.95em]">ZakatComputed</code> entry into the hash-chained Truth Ledger so the number is verifiable a year later. Pay through Stripe to Islamic Relief, Zakat Foundation, HHRD, or partnership mosques with auto-generated receipts (Hijri + Gregorian dated) and a yearly tax-record export.
            </p>
            <p className="font-serif text-foreground/95 text-lg md:text-xl italic">
              Build a Zakat calculation that would survive a fatwa committee. Then let your phone do it in two seconds.
            </p>
          </div>
          <ul className="mx-auto flex flex-col gap-3 pt-2 md:flex-row md:flex-wrap md:justify-center md:gap-x-6 md:gap-y-3">
            {CLAIMS.map((claim) => (
              <li
                key={claim}
                className="flex items-start gap-2 t-caption text-foreground/85 text-left text-sm"
              >
                <Check className="mt-0.5 h-4 w-4 shrink-0 text-gold-primary" />
                <span>{claim}</span>
              </li>
            ))}
          </ul>
        </RevealOnScroll>
      </Container>
    </section>
  );
}

import { ArrowRight } from "lucide-react";

import { Badge } from "@/app/(landing)/_primitives/Badge";
import { Button } from "@/app/(landing)/_primitives/Button";
import { Container } from "@/app/(landing)/_primitives/Container";
import { Eyebrow } from "@/app/(landing)/_primitives/Eyebrow";
import { Wordmark } from "@/app/(landing)/_primitives/Wordmark";
import { RevealOnScroll } from "./RevealOnScroll";
import { ScrollIndicator } from "./ScrollIndicator";

/**
 * Hero — single 100vh focal viewport. One motion element (the gold
 * orb), one CTA, one ambient noise texture. Sticky header materialises
 * after first scroll.
 */
export function Hero() {
  return (
    <header className="relative isolate overflow-hidden bg-depth-page min-h-[680px] min-h-[100svh] flex flex-col">
      {/* Orb — gold radial gradient, subtle drift. The blur is
          intentionally heavy so the orb reads as ambient light, not
          a UI element. */}
      <div
        aria-hidden="true"
        className="absolute -top-32 right-[-12%] h-[420px] w-[420px] md:h-[700px] md:w-[700px] rounded-full opacity-30 blur-3xl pointer-events-none animate-orb"
        style={{
          background:
            "radial-gradient(circle at center, hsl(31 49% 64% / 0.85) 0%, transparent 60%)",
        }}
      />
      <div
        aria-hidden="true"
        className="absolute inset-0 noise-texture opacity-[0.04] pointer-events-none"
      />

      {/* Sticky top bar */}
      <Container as="header" className="relative z-10 flex h-16 items-center justify-between pt-4">
        <Wordmark size="sm" />
        <div className="flex items-center gap-3">
          {/* Badge is desktop-only — on phones the header is just
              wordmark + one CTA, so it never feels cramped. */}
          <Badge
            className="hidden border-gold-primary/25 bg-gold-primary/[0.06] sm:inline-flex"
            icon={
              <span className="relative grid h-2 w-2 place-items-center">
                <span className="absolute h-2 w-2 rounded-full bg-success/70 animate-ping" />
                <span className="relative h-1.5 w-1.5 rounded-full bg-success" />
              </span>
            }
          >
            <span className="text-foreground/90">Private beta · Aug 2026</span>
          </Badge>
          <Button variant="primary" size="sm" href="#waitlist">
            Reserve my spot
          </Button>
        </div>
      </Container>

      {/* Hero copy */}
      <Container className="relative z-10 flex flex-1 flex-col items-center justify-center text-center py-12">
        <RevealOnScroll immediate className="space-y-6 max-w-3xl">
          <Eyebrow>AI-NATIVE WEALTH PLATFORM</Eyebrow>
          <h1
            className="font-serif font-bold tracking-tight"
            style={{
              fontSize: "clamp(40px, 6vw, 72px)",
              lineHeight: 1.12,
            }}
          >
            <span className="block text-foreground/95">Know your net worth.</span>
            {/* pb so the descender never clips against the clip-box. */}
            <span className="block hero-headline-sweep pb-[0.12em]">
              Down to the cent.
            </span>
          </h1>
          <p className="text-foreground/85 mx-auto max-w-2xl text-lg md:text-xl leading-relaxed">
            Right now, your net worth is a guess.{" "}
            <span className="text-gold-cream">Mizan turns it into a fact.</span>
          </p>
          <p className="text-foreground/70 mx-auto max-w-2xl text-base leading-relaxed">
            Bank Accounts, Stocks, ETFs, Mutual Funds, REITs, Cryptos, Properties, Gold, Pension Funds, Sukuks — and that spreadsheet you keep updating — all in one audit-grade ledger, accurate to the cent and current to the second.
          </p>
          <div className="flex flex-col items-center justify-center gap-3 pt-2 sm:flex-row">
            <Button variant="primary" size="lg" href="#waitlist" className="cta-glow">
              Reserve my spot <ArrowRight className="h-4 w-4" />
            </Button>
            <Button variant="ghost" size="lg" href="#product">
              See how it works
            </Button>
          </div>
          <p className="t-caption text-foreground/55">
            No more scattered logins. No more stale numbers. No more guesswork.
          </p>
          <p className="t-micro text-foreground/40">
            Backed by angel investors &amp; fintech professionals
          </p>
        </RevealOnScroll>
      </Container>

      <div className="relative z-10 flex justify-center pb-8">
        <ScrollIndicator />
      </div>

      <style>{`
        @keyframes orb-drift {
          0%, 100% { transform: translate(0, 0); }
          25%      { transform: translate(-24px, 18px); }
          50%      { transform: translate(12px, 32px); }
          75%      { transform: translate(28px, 4px); }
        }

        /* Headline gradient — static gold-cream fill by default. The
           animated sweep is layered on at desktop only (see media query
           below) so phones never run a continuous repaint. */
        .hero-headline-sweep {
          background: linear-gradient(
            100deg,
            hsl(40 67% 87%) 0%,
            hsl(40 67% 87%) 35%,
            hsl(45 95% 78%) 50%,
            hsl(40 67% 87%) 65%,
            hsl(40 67% 87%) 100%
          );
          background-size: 250% 100%;
          background-position: 50% 0;
          -webkit-background-clip: text;
          background-clip: text;
          -webkit-text-fill-color: transparent;
          color: transparent;
          line-height: 1.18;
          padding-bottom: 0.08em;
        }
        @keyframes hero-sweep {
          0%   { background-position: 100% 0; }
          100% { background-position: -150% 0; }
        }
        @keyframes cta-pulse {
          0%, 100% { box-shadow: 0 0 0 0 hsl(31 49% 64% / 0.0), 0 8px 32px -8px hsl(31 49% 64% / 0.25); }
          50%      { box-shadow: 0 0 0 6px hsl(31 49% 64% / 0.08), 0 12px 40px -8px hsl(31 49% 64% / 0.45); }
        }

        /* Continuous animations run on pointer-capable, larger screens
           only — keeps phones smooth + saves battery. */
        @media (min-width: 768px) {
          .animate-orb {
            animation: orb-drift 20s ease-in-out infinite;
            will-change: transform;
          }
          .hero-headline-sweep {
            animation: hero-sweep 6s linear infinite;
          }
          .cta-glow {
            animation: cta-pulse 2.6s ease-in-out infinite;
          }
          .urgency-pill { transition: transform 150ms ease-out; }
          .urgency-pill:hover { transform: translateY(-1px); }
        }

        @media (prefers-reduced-motion: reduce) {
          .animate-orb,
          .hero-headline-sweep,
          .cta-glow,
          .urgency-pill { animation: none !important; transition: none !important; }
          .hero-headline-sweep { color: hsl(40 67% 87%); -webkit-text-fill-color: hsl(40 67% 87%); }
        }
      `}</style>
    </header>
  );
}

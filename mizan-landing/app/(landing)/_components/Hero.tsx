import { ArrowRight } from "lucide-react";

import { Badge } from "@/app/(landing)/_primitives/Badge";
import { Button } from "@/app/(landing)/_primitives/Button";
import { Container } from "@/app/(landing)/_primitives/Container";
import { Eyebrow } from "@/app/(landing)/_primitives/Eyebrow";
import { Wordmark } from "@/app/(landing)/_primitives/Wordmark";
import { RevealOnScroll } from "./RevealOnScroll";
import { ScrollIndicator } from "./ScrollIndicator";
import { SocialProofCounter } from "./SocialProofCounter";

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
        className="absolute -top-32 right-[-12%] h-[700px] w-[700px] rounded-full opacity-30 blur-3xl pointer-events-none animate-orb"
        style={{
          background:
            "radial-gradient(circle at center, hsl(31 49% 64% / 0.85) 0%, transparent 60%)",
        }}
      />
      <div
        aria-hidden="true"
        className="absolute inset-0 noise-texture opacity-[0.04] pointer-events-none"
      />

      {/* Sticky top bar (transparent until 120px scroll). The
          scroll-tracked variant lives in a tiny client component
          inline below. */}
      <Container as="header" className="relative z-10 flex h-16 items-center justify-between pt-4">
        <Wordmark size="sm" />
        <div className="flex items-center gap-3">
          <Badge>📍 Coming August 2026</Badge>
          <Button variant="ghost" size="sm" href="#waitlist">
            Reserve your seat
          </Button>
        </div>
      </Container>

      {/* Hero copy */}
      <Container className="relative z-10 flex flex-1 flex-col items-center justify-center text-center py-16">
        <RevealOnScroll className="space-y-6 max-w-3xl">
          <Eyebrow>PRIVATE WEALTH · BUILT FOR THE MUSLIM AFFLUENT</Eyebrow>
          <h1
            className="font-serif font-bold tracking-tight"
            style={{
              fontSize: "clamp(40px, 6vw, 72px)",
              lineHeight: 1.05,
            }}
          >
            <span className="block text-gold-cream">Your wealth, accounted for.</span>
            <span className="block text-foreground/90">Down to the dirham.</span>
          </h1>
          <p className="text-foreground/75 mx-auto max-w-xl text-base md:text-lg leading-relaxed">
            Twelve asset classes. Nine jurisdictions. Zakat calculated correctly across all four schools. One audit-grade ledger you can prove.
          </p>
          <div className="flex flex-col items-center justify-center gap-3 pt-2 sm:flex-row">
            <Button variant="primary" size="lg" href="#waitlist">
              Reserve your seat <ArrowRight className="h-4 w-4" />
            </Button>
            <Button variant="ghost" size="lg" href="#product">
              See how it works
            </Button>
          </div>
          <SocialProofCounter />
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
        .animate-orb {
          animation: orb-drift 20s ease-in-out infinite;
          will-change: transform;
        }
        @media (prefers-reduced-motion: reduce) {
          .animate-orb { animation: none; }
        }
      `}</style>
    </header>
  );
}

import { Check } from "lucide-react";

import { Badge } from "@/app/(landing)/_primitives/Badge";
import { Container } from "@/app/(landing)/_primitives/Container";
import { Eyebrow } from "@/app/(landing)/_primitives/Eyebrow";
import { Section } from "@/app/(landing)/_primitives/Section";
import { AppShowcase } from "./AppShowcase";
import { RevealOnScroll } from "./RevealOnScroll";

const PILLS = [
  "Tracks every asset",
  "Intelligent insights",
  "Goals & forecasts",
] as const;

export function Product() {
  return (
    <Section id="product" background="page">
      <Container>
        <div className="grid items-center gap-10 md:gap-16 md:grid-cols-[1.4fr_1fr]">
          <RevealOnScroll className="order-2 md:order-1">
            <AppShowcase />
          </RevealOnScroll>
          <RevealOnScroll className="order-1 md:order-2 space-y-5">
            <Eyebrow>YOUR CHIEF FINANCIAL OFFICER</Eyebrow>
            <h2 className="t-title-lg text-foreground/95 md:text-3xl md:leading-tight">
              Not a tracker.{" "}
              <span className="text-gold-cream">A CFO.</span>
            </h2>
            <p className="t-body text-foreground/80 text-base leading-relaxed">
              Bank accounts. Stocks, ETFs, mutual funds, REITs. Crypto. Property. Gold. Pensions. Sukuks — Mizan connects to all of it, across every major institution and exchange worldwide. Add an account in seconds; Mizan keeps it live, converted to your base currency, automatically.
            </p>
            <p className="t-body text-foreground/80 text-base leading-relaxed">
              Then it goes to work like a private CFO: surfacing insights you&apos;d have missed, flagging risk before it costs you, modelling the goals you&apos;re saving toward, and answering — in plain language — what your numbers actually mean. Every figure timestamped, every conversion traceable, every action written to an immutable, audit-grade ledger.
            </p>
            <div className="flex flex-wrap gap-2 pt-2">
              {PILLS.map((label) => (
                <Badge
                  key={label}
                  icon={<Check className="h-3.5 w-3.5 text-gold-primary" />}
                >
                  {label}
                </Badge>
              ))}
            </div>
          </RevealOnScroll>
        </div>
      </Container>
    </Section>
  );
}

import { Check } from "lucide-react";

import { Badge } from "@/app/(landing)/_primitives/Badge";
import { Container } from "@/app/(landing)/_primitives/Container";
import { Eyebrow } from "@/app/(landing)/_primitives/Eyebrow";
import { Section } from "@/app/(landing)/_primitives/Section";
import { DashboardMockup } from "./DashboardMockup";
import { RevealOnScroll } from "./RevealOnScroll";

const PILLS = [
  "12 asset classes",
  "9 jurisdictions",
  "Audit-grade provenance",
] as const;

export function Product() {
  return (
    <Section id="product" background="page">
      <Container>
        <div className="grid items-center gap-10 md:gap-16 md:grid-cols-[1.4fr_1fr]">
          <RevealOnScroll className="order-2 md:order-1">
            <DashboardMockup />
          </RevealOnScroll>
          <RevealOnScroll className="order-1 md:order-2 space-y-5">
            <Eyebrow>WHAT IT TRACKS</Eyebrow>
            <h2 className="t-title-lg text-foreground/95 md:text-3xl md:leading-tight">
              Twelve classes.{" "}
              <span className="text-gold-cream">One ledger.</span>
            </h2>
            <p className="t-body text-foreground/80 text-base leading-relaxed">
              Mizan connects to every account you have. Plaid for US banks, SnapTrade for global brokerages. Crypto via CCXT and direct chain reading. Sukuks priced through Bondevalue. Gold and silver spot from MetalpriceAPI — the same silver price that determines your Nisab threshold.
            </p>
            <p className="t-body text-foreground/80 text-base leading-relaxed">
              Every figure timestamped. Every conversion traceable. Every action logged in an immutable hash-chained Truth Ledger.
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

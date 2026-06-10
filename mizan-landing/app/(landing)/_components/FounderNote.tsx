import { Container } from "@/app/(landing)/_primitives/Container";
import { Eyebrow } from "@/app/(landing)/_primitives/Eyebrow";
import { Section } from "@/app/(landing)/_primitives/Section";
import { Wordmark } from "@/app/(landing)/_primitives/Wordmark";
import { RevealOnScroll } from "./RevealOnScroll";

export function FounderNote() {
  return (
    <Section background="page">
      <Container>
        <RevealOnScroll className="mx-auto max-w-2xl space-y-6 text-center">
          <Eyebrow tone="primary">FOUNDER&apos;S NOTE</Eyebrow>
          <h3 className="t-title-lg text-foreground/95 md:text-3xl">
            Why we built Mizan
          </h3>
          <div className="space-y-5 text-left text-foreground/85">
            <p className="text-base leading-relaxed">
              It started as our own problem. The wealth we were watching — across our families, our friends, our own balance sheets — had quietly outgrown every tool meant to track it. Brokerages in three countries. Crypto on four exchanges. Property across borders. Gold receipts in a drawer. Seven apps doing three things each, glued together by a spreadsheet someone updates at 11pm and still doesn&apos;t fully trust.
            </p>
            <p className="text-base leading-relaxed">
              We tried to manage it the normal way. It didn&apos;t work. Once wealth spans enough accounts, currencies and jurisdictions, no dashboard built for one country and one asset class can keep up. So we stopped building a better tracker and built something else entirely: an <span className="text-gold-cream">AI-native</span> system where the intelligence sits at the centre, not bolted on the side.
            </p>
            <p className="text-base leading-relaxed">
              The engineering is real, not a wrapper. A Rust-native financial truth engine. An immutable, hash-chained audit ledger so every figure is provable a year later. An AI layer with bounded, audited write access through a Truth-Ledger-aware dispatcher — it can act, but never silently. Per-provider AES-256-GCM encryption on every sync token. It runs as a native desktop app, not a browser tab.
            </p>
            <p className="text-base leading-relaxed">
              Mizan is for anyone whose wealth has become too complex for a spreadsheet — and yes, it does Zakat properly too, across all four schools, because that&apos;s a bar nothing else clears. If that&apos;s you, join the waitlist. We read every reply personally.
            </p>
          </div>
          <div className="flex justify-center pt-8 opacity-60">
            <Wordmark size="sm" />
          </div>
        </RevealOnScroll>
      </Container>
    </Section>
  );
}

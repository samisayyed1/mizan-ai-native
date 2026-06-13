import { ChevronDown } from "lucide-react";

import { Container } from "@/app/(landing)/_primitives/Container";
import { Eyebrow } from "@/app/(landing)/_primitives/Eyebrow";
import { Section } from "@/app/(landing)/_primitives/Section";
import { RevealOnScroll } from "./RevealOnScroll";

/**
 * FAQ — visible accordion (native <details>, so it's crawlable +
 * keyboard-accessible with zero JS) PLUS a matching FAQPage JSON-LD
 * block for Google rich results. The data lives in one array so the
 * visible answers and the structured data can never drift apart.
 *
 * Answers are kept as plain strings (no markup) so they're valid for
 * schema.org without escaping surprises; the visible render uses the
 * same strings.
 */
const FAQ_ITEMS: { q: string; a: string }[] = [
  {
    q: "What is Mizan?",
    a: "Mizan is an AI-native personal wealth platform that brings every account you own — banks, brokerages, crypto, property, gold, pensions, and sukuks — into one audit-grade ledger, accurate to the cent and current to the second.",
  },
  {
    q: "When does Mizan launch?",
    a: "Mizan launches in August 2026. Join the waitlist for priority access and founding-member pricing locked in for life.",
  },
  {
    q: "What can Mizan track?",
    a: "Bank accounts, stocks, ETFs, mutual funds, REITs, crypto, property, gold, pension funds, and sukuks — across multiple currencies and jurisdictions, all in one place.",
  },
  {
    q: "Is my financial data secure?",
    a: "Yes. Mizan uses AES-256-GCM encryption on every connection token, writes every figure to an immutable hash-chained audit ledger, and runs as a native app with bounded, audited AI access. Read more on our Security page.",
  },
  {
    q: "Does Mizan give financial advice?",
    a: "Mizan helps you see and understand your wealth and can suggest actions, but it never executes trades, moves money, or replaces a professional advisor. You always stay in control of your decisions.",
  },
  {
    q: "Does Mizan calculate Zakat?",
    a: "Yes — Mizan calculates Zakat correctly across all four schools of thought, a bar most apps simply don't clear.",
  },
  {
    q: "How much will Mizan cost?",
    a: "Pricing is announced at launch. Founding members who join the waitlist lock in founding-member pricing for life.",
  },
  {
    q: "Which countries will Mizan support?",
    a: "Mizan is built for global wealth across multiple jurisdictions. We open markets country by country, and waitlist members get first access as each one opens.",
  },
];

const faqJsonLd = {
  "@context": "https://schema.org",
  "@type": "FAQPage",
  mainEntity: FAQ_ITEMS.map((item) => ({
    "@type": "Question",
    name: item.q,
    acceptedAnswer: { "@type": "Answer", text: item.a },
  })),
} as const;

export function FAQ() {
  return (
    <Section id="faq" background="page" topBorder>
      <Container>
        <script
          type="application/ld+json"
          dangerouslySetInnerHTML={{ __html: JSON.stringify(faqJsonLd) }}
        />
        <RevealOnScroll className="mx-auto max-w-2xl space-y-3 text-center">
          <Eyebrow>FAQ</Eyebrow>
          <h2 className="t-title-lg text-foreground/95 md:text-3xl md:leading-tight">
            Questions, answered.
          </h2>
        </RevealOnScroll>

        <RevealOnScroll className="mx-auto mt-10 max-w-2xl divide-y divide-depth-border overflow-hidden rounded-2xl border border-depth-border bg-depth-card">
          {FAQ_ITEMS.map(({ q, a }) => (
            <details key={q} className="group">
              <summary className="flex cursor-pointer list-none items-center justify-between gap-4 px-5 py-4 t-body-bold text-sm text-foreground/90 transition-colors hover:bg-depth-elevated [&::-webkit-details-marker]:hidden">
                {q}
                <ChevronDown
                  aria-hidden="true"
                  className="h-4 w-4 shrink-0 text-gold-primary transition-transform duration-200 group-open:rotate-180"
                />
              </summary>
              <div className="px-5 pb-5 -mt-1">
                <p className="text-[15px] leading-relaxed text-foreground/70">{a}</p>
              </div>
            </details>
          ))}
        </RevealOnScroll>
      </Container>
    </Section>
  );
}

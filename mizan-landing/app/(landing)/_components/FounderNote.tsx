import { Container } from "@/app/(landing)/_primitives/Container";
import { Section } from "@/app/(landing)/_primitives/Section";
import { Wordmark } from "@/app/(landing)/_primitives/Wordmark";
import { RevealOnScroll } from "./RevealOnScroll";

export function FounderNote() {
  return (
    <Section background="page">
      <Container>
        <RevealOnScroll className="mx-auto max-w-3xl">
          <div className="grid items-start gap-8 md:grid-cols-[120px_1fr] md:gap-12">
            {/* Founder avatar — monogram fallback, swap to /sami.webp
                once the user supplies the photo. */}
            <div
              aria-hidden="true"
              className="mx-auto flex h-24 w-24 shrink-0 items-center justify-center rounded-full border border-gold-primary/30 bg-depth-card md:mx-0"
            >
              <span className="font-serif italic text-3xl text-gold-cream">
                S
              </span>
            </div>
            <div className="space-y-5 text-foreground/85">
              <h3 className="t-title text-foreground/95">
                Why I&apos;m building Mizan
              </h3>
              <p className="text-base leading-relaxed">
                I&apos;m Sami. I&apos;m 18. I&apos;m building Mizan because nothing exists for the Muslim wealth I see in my own family, my own community, my own balance sheet. Sukuks at Emaar, CPF in Singapore, properties in Hyderabad, gold for Zakat, multiple currencies, multiple schools of thought — and seven apps trying to track three things each.
              </p>
              <p className="text-base leading-relaxed">
                The architecture is real. Tauri 2 desktop, Rust-native financial truth engine, hash-chained immutable audit ledger, AI agent with bounded write access through a Truth-Ledger-aware dispatcher, AAOIFI-graded Sharia screening per ADR 0012, per-provider AES-256-GCM encryption on every sync token. The Singapore Sharia-aware millionaire&apos;s Ramadan scenario passes end-to-end on the Playwright suite. Production launches August 2026.
              </p>
              <p className="text-base leading-relaxed">
                If your wealth is too complex for spreadsheets and your faith is too important for an app that doesn&apos;t try — join the waitlist. I&apos;ll write back personally.
              </p>
              <p className="font-serif italic text-xl text-gold-cream">
                — Sami
              </p>
            </div>
          </div>
          <div className="mt-16 flex justify-center opacity-60">
            <Wordmark size="sm" />
          </div>
        </RevealOnScroll>
      </Container>
    </Section>
  );
}

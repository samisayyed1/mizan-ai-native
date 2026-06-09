import { Container } from "@/app/(landing)/_primitives/Container";
import { Eyebrow } from "@/app/(landing)/_primitives/Eyebrow";
import { Section } from "@/app/(landing)/_primitives/Section";
import { RevealOnScroll } from "./RevealOnScroll";
import { WaitlistForm } from "./WaitlistForm";

export function FoundingOffer() {
  return (
    <Section id="waitlist" background="container" topBorder bottomBorder>
      <Container>
        <RevealOnScroll className="mx-auto max-w-2xl">
          <div className="rounded-2xl border border-gold-primary/25 bg-depth-card p-8 md:p-12 space-y-7">
            <div className="space-y-3 text-center">
              <Eyebrow tone="primary">FOUNDING MEMBERS</Eyebrow>
              <h2 className="t-title-lg text-foreground/95 md:text-3xl">
                1,000 seats. <span className="text-gold-cream tabular-nums">847</span> taken.
              </h2>
              <p className="t-body text-foreground/75 text-base leading-relaxed">
                Founding members get Gold tier free for the first year (worth $300). Forever-locked pricing after that. Direct line to the founder for feature requests. First access to new sync providers as they open — Setu (India), Lean (Gulf), Basiq (Australia), Tink (EU), SGFinDex (Singapore).
              </p>
            </div>
            <WaitlistForm />
          </div>
        </RevealOnScroll>
      </Container>
    </Section>
  );
}

import { Calculator, Layers, ShieldQuestion } from "lucide-react";

import { Container } from "@/app/(landing)/_primitives/Container";
import { Eyebrow } from "@/app/(landing)/_primitives/Eyebrow";
import { Section } from "@/app/(landing)/_primitives/Section";
import { RevealItem, RevealOnScroll } from "./RevealOnScroll";

const PROBLEMS = [
  {
    icon: Layers,
    title: "Spreadsheets break across borders.",
    body: "Nine tax regimes, twelve asset classes, one fragile sheet. Reconciliation eats your weekend.",
  },
  {
    icon: Calculator,
    title: "Wealth apps ignore Zakat.",
    body: "Or worse — hardcode one school and call it done. Multi-jurisdiction Hawl tracking? Nowhere.",
  },
  {
    icon: ShieldQuestion,
    title: "Robo-advisors aren't Sharia-aware.",
    body: "Sukuk, gold, equities — screened by what standard? Most apps can't tell you. Yours can't tell you.",
  },
] as const;

export function Problem() {
  return (
    <Section background="page">
      <Container>
        <RevealOnScroll className="mx-auto max-w-2xl space-y-3 text-center">
          <Eyebrow>THE GAP</Eyebrow>
          <h2 className="t-title-lg text-foreground/95 md:text-3xl md:leading-tight">
            Wealth apps weren&apos;t built for you.
          </h2>
          <p className="t-body text-foreground/70 text-base md:text-lg leading-relaxed">
            You have eight of them. None of them talk to each other. And none of them understand your portfolio.
          </p>
        </RevealOnScroll>

        <RevealOnScroll
          stagger
          className="mt-12 grid gap-4 md:grid-cols-3 md:gap-6"
        >
          {PROBLEMS.map(({ icon: Icon, title, body }) => (
            <RevealItem key={title}>
              <article className="h-full rounded-2xl border border-depth-border bg-depth-card p-6 md:p-8 transition-[transform,border-color] duration-150 hover:border-gold-primary/30 hover:-translate-y-0.5">
                <span
                  aria-hidden="true"
                  className="mb-5 inline-flex h-10 w-10 items-center justify-center rounded-xl bg-depth-elevated"
                >
                  <Icon className="h-5 w-5 text-gold-primary" />
                </span>
                <h3 className="t-body-bold text-foreground/95 text-base">{title}</h3>
                <p className="mt-2 t-body text-foreground/65 text-sm leading-relaxed">{body}</p>
              </article>
            </RevealItem>
          ))}
        </RevealOnScroll>
      </Container>
    </Section>
  );
}

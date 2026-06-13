import { Sparkles, TrendingUp, Lock } from "lucide-react";

import { Container } from "@/app/(landing)/_primitives/Container";
import { Eyebrow } from "@/app/(landing)/_primitives/Eyebrow";
import { Section } from "@/app/(landing)/_primitives/Section";
import { RevealOnScroll } from "./RevealOnScroll";
import { WaitlistForm } from "./WaitlistForm";

const BENEFITS = [
  {
    icon: Sparkles,
    title: "Priority access",
    body: "Be among the first through the door when we launch.",
  },
  {
    icon: Lock,
    title: "Founding-member pricing",
    body: "Locked in for life — the rate you join at never goes up.",
  },
  {
    icon: TrendingUp,
    title: "Early influence",
    body: "Shape the features we build next. We read every reply.",
  },
] as const;

export function FoundingOffer() {
  return (
    <Section id="waitlist" background="container" topBorder bottomBorder>
      <Container>
        <RevealOnScroll className="mx-auto max-w-2xl space-y-8">
          <div className="space-y-3 text-center">
            <Eyebrow tone="primary">BE FIRST IN LINE</Eyebrow>
            <h2 className="t-title-lg text-foreground/95 md:text-3xl">
              Replace every tracker, spreadsheet, and sticky note.
            </h2>
            <p className="t-body text-foreground/75 text-base leading-relaxed">
              We&apos;re putting the finishing touches on it now. Join the waitlist today and you&apos;ll get:
            </p>
          </div>

          {/* Benefits */}
          <ul className="grid gap-3 sm:grid-cols-3">
            {BENEFITS.map(({ icon: Icon, title, body }) => (
              <li
                key={title}
                className="rounded-2xl border border-depth-border bg-depth-card p-5 text-center sm:text-left"
              >
                <span
                  aria-hidden="true"
                  className="mb-3 inline-flex h-9 w-9 items-center justify-center rounded-xl bg-gold-primary/10 text-gold-primary"
                >
                  <Icon className="h-4 w-4" />
                </span>
                <p className="t-body-bold text-foreground/95 text-sm">{title}</p>
                <p className="mt-1 t-caption text-foreground/60 leading-relaxed">
                  {body}
                </p>
              </li>
            ))}
          </ul>

          {/* Form card */}
          <div className="rounded-2xl border border-gold-primary/25 bg-depth-card p-6 md:p-8 space-y-4">
            <div className="space-y-1.5 text-center">
              <h3 className="t-title text-foreground/95">Join the waitlist</h3>
              <p className="t-caption text-foreground/60 leading-relaxed">
                Drop your email below. We&apos;ll only reach out with launch updates — no noise, no spam, no filler.
              </p>
            </div>
            <WaitlistForm />
          </div>

          <p className="text-center font-serif text-lg italic text-gold-cream md:text-xl">
            Your wealth deserves more than a best guess. It deserves Mizan.
          </p>
        </RevealOnScroll>
      </Container>
    </Section>
  );
}

import { Sparkles } from "lucide-react";

import { Container } from "@/app/(landing)/_primitives/Container";
import { Eyebrow } from "@/app/(landing)/_primitives/Eyebrow";
import { Section } from "@/app/(landing)/_primitives/Section";
import { RevealItem, RevealOnScroll } from "./RevealOnScroll";

/**
 * Real prompts a member can type. Shown as input-style cards (a gold
 * caret + a sparkle) rather than fabricated answers — it reads as
 * "ask this and Mizan answers" without inventing numbers.
 */
const PROMPTS = [
  "Rebalance my entire portfolio using my available cash.",
  "What's my real exposure to U.S. tech right now — across stocks, ETFs, and funds?",
  "Am I overweight on emerging markets after this quarter's moves?",
  "If I sell this property, how does my asset mix shift?",
] as const;

export function AILayer() {
  return (
    <Section background="page" topBorder>
      <Container>
        <RevealOnScroll className="mx-auto max-w-2xl space-y-3 text-center">
          <Eyebrow>BUILT AI-NATIVE</Eyebrow>
          <h2 className="t-title-lg text-foreground/95 md:text-3xl md:leading-tight">
            Built AI-native, not AI-bolted-on.
          </h2>
          <p className="t-body text-foreground/75 text-base md:text-lg leading-relaxed">
            Mizan isn&apos;t a tracker with a chatbot stapled to the side. It&apos;s an AI-native wealth platform — so you can simply <em>ask</em>, and get answers grounded in your actual portfolio.
          </p>
        </RevealOnScroll>

        <RevealOnScroll
          stagger
          className="mx-auto mt-12 grid max-w-3xl gap-3 sm:grid-cols-2"
        >
          {PROMPTS.map((prompt, i) => (
            <RevealItem key={i}>
              <article className="group flex h-full items-start gap-3 rounded-2xl border border-depth-border bg-depth-card p-5 transition-[border-color,transform] duration-150 hover:-translate-y-0.5 hover:border-gold-primary/30">
                <span
                  aria-hidden="true"
                  className="mt-0.5 inline-flex h-7 w-7 shrink-0 items-center justify-center rounded-lg bg-gold-primary/10 text-gold-primary"
                >
                  <Sparkles className="h-4 w-4" />
                </span>
                <p className="t-body text-left text-sm leading-relaxed text-foreground/90 md:text-base">
                  &ldquo;{prompt}&rdquo;
                </p>
              </article>
            </RevealItem>
          ))}
        </RevealOnScroll>

        <RevealOnScroll className="mx-auto mt-10 max-w-2xl text-center">
          <p className="t-body text-foreground/70 text-base leading-relaxed">
            No more rebalancing one account at a time. No more guessing your true sector or geographic exposure. Mizan sees <span className="text-gold-cream">everything you own</span> — and helps you act on it.
          </p>
        </RevealOnScroll>
      </Container>
    </Section>
  );
}

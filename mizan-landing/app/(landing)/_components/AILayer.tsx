import { Container } from "@/app/(landing)/_primitives/Container";
import { Eyebrow } from "@/app/(landing)/_primitives/Eyebrow";
import { Section } from "@/app/(landing)/_primitives/Section";
import { RevealItem, RevealOnScroll } from "./RevealOnScroll";

const CONVERSATIONS = [
  {
    user: "What's my Zakat this year?",
    ai: "$2,847 across all eligible holdings. Breakdown: $1,890 from Sukuks (full face value), $612 from cash above Nisab through Hawl, $345 from your SPUS gold-backed holdings. Your Bukit Batok primary residence and CPF-OA locked balance excluded per your configured Shafi'i school. Pay now → routes through Stripe to Islamic Relief, Zakat Foundation, HHRD, or a partnership mosque. Receipt back in 60 seconds.",
  },
  {
    user: "My Emaar Sukuk matures in 47 days — what should I reinvest into?",
    ai: "Pulling current Sharia-compliant new issues priced through Bondevalue. Three matches with similar profile ($200K minimum, BBB+ rating, 5–7 year duration): Aldar 2031 at 5.2%, Dubai Islamic Bank Tier 1 at 5.0%, Investcorp perpetual at 5.4% (illiquid). Want me to draft the broker instruction to your Stan Chart Sukuk desk?",
  },
  {
    user: "Set a reminder when USDSGD moves more than 1%.",
    ai: "Done. Heads-up: at your current USD-denominated equity exposure ($367K), a 1% USDSGD move shifts your base-currency net worth by ~$3,670. Want me to also flag if your unhedged FX exposure ever exceeds 25% of net worth?",
  },
] as const;

export function AILayer() {
  return (
    <Section background="page" topBorder>
      <Container>
        <RevealOnScroll className="mx-auto max-w-2xl space-y-3 text-center">
          <Eyebrow>THE INTELLIGENCE</Eyebrow>
          <h2 className="t-title-lg text-foreground/95 md:text-3xl md:leading-tight">
            Talk to your wealth.
          </h2>
          <p className="t-body text-foreground/75 text-base md:text-lg leading-relaxed">
            Mizan is AI-native. The agent <em>is</em> the operating system, not a chat icon stapled on. Anthropic Claude with bounded write access through a Truth-Ledger-aware dispatcher. Ask anything. Tell it anything. It remembers.
          </p>
        </RevealOnScroll>

        <RevealOnScroll
          stagger
          className="mx-auto mt-12 max-w-3xl space-y-4"
        >
          {CONVERSATIONS.map(({ user, ai }, i) => (
            <RevealItem key={i}>
              <article className="rounded-2xl border border-depth-border bg-depth-card p-5 md:p-6 space-y-4">
                <div className="flex justify-end">
                  <p className="max-w-[85%] rounded-2xl rounded-br-md bg-gold-primary/[0.14] border border-gold-primary/20 px-4 py-2.5 t-body text-foreground text-sm md:text-base">
                    {user}
                  </p>
                </div>
                <div className="flex">
                  <p className="max-w-[92%] rounded-2xl rounded-bl-md bg-depth-elevated border border-depth-border px-4 py-3 t-body text-foreground/85 text-sm md:text-base leading-relaxed">
                    {ai}
                  </p>
                </div>
              </article>
            </RevealItem>
          ))}
        </RevealOnScroll>
      </Container>
    </Section>
  );
}

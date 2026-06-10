import type { ReactNode } from "react";

import { Container } from "@/app/(landing)/_primitives/Container";
import { Eyebrow } from "@/app/(landing)/_primitives/Eyebrow";

/**
 * Shared shell for the legal / info pages (Privacy, Terms, Security).
 * Premium prose styling is applied via arbitrary-variant Tailwind
 * selectors so it stays self-contained — no global CSS, no typography
 * plugin. Headings are serif gold-cream, body is foreground/75, links
 * are gold, list markers are gold-deep.
 */
export function LegalArticle({
  eyebrow,
  title,
  intro,
  updated,
  children,
}: {
  eyebrow: string;
  title: string;
  intro: string;
  updated: string;
  children: ReactNode;
}) {
  return (
    <Container className="py-16 md:py-24">
      <div className="mx-auto max-w-2xl">
        <header className="mb-12 border-b border-depth-border pb-10">
          <Eyebrow tone="primary">{eyebrow}</Eyebrow>
          <h1 className="mt-3 font-serif text-4xl font-bold leading-tight text-gold-cream md:text-5xl">
            {title}
          </h1>
          <p className="mt-4 text-base leading-relaxed text-foreground/75">
            {intro}
          </p>
          <p className="mt-5 t-micro text-gold-deep">Last updated · {updated}</p>
        </header>

        <article
          className="
            [&_h2]:mb-4 [&_h2]:mt-12 [&_h2]:font-serif [&_h2]:text-2xl [&_h2]:font-bold [&_h2]:text-gold-cream first:[&_h2]:mt-0
            [&_h3]:mb-2 [&_h3]:mt-8 [&_h3]:text-base [&_h3]:font-semibold [&_h3]:text-foreground/95
            [&_p]:mb-4 [&_p]:text-[15px] [&_p]:leading-relaxed [&_p]:text-foreground/75
            [&_ul]:mb-4 [&_ul]:mt-1 [&_ul]:space-y-2 [&_ul]:pl-5
            [&_li]:list-disc [&_li]:text-[15px] [&_li]:leading-relaxed [&_li]:text-foreground/75 [&_li]:marker:text-gold-deep
            [&_a]:text-gold-primary [&_a]:underline [&_a]:underline-offset-2 hover:[&_a]:text-gold-cream
            [&_strong]:font-semibold [&_strong]:text-foreground/90
          "
        >
          {children}
        </article>

        <div className="mt-16 rounded-2xl border border-depth-border bg-depth-card p-6 text-center">
          <p className="text-sm text-foreground/75">
            Questions about this page? Email{" "}
            <a
              href="mailto:info@getmizan.net"
              className="text-gold-primary underline underline-offset-2 hover:text-gold-cream"
            >
              info@getmizan.net
            </a>{" "}
            — we read every message.
          </p>
        </div>
      </div>
    </Container>
  );
}

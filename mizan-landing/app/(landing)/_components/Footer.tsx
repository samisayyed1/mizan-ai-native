import Link from "next/link";

import { Container } from "@/app/(landing)/_primitives/Container";
import { Wordmark } from "@/app/(landing)/_primitives/Wordmark";

const POLICY_LINKS = [
  { href: "/privacy", label: "Privacy" },
  { href: "/terms", label: "Terms" },
  { href: "/security", label: "Security" },
] as const;

export function Footer() {
  return (
    <footer className="border-t border-depth-border bg-depth-page">
      <Container className="py-10 md:py-12">
        {/* Top: brand + nav + contact */}
        <div className="flex flex-col gap-8 md:flex-row md:items-start md:justify-between">
          <div className="space-y-2">
            <Wordmark size="sm" />
            <p className="t-micro text-gold-deep">
              Your AI-native Chief Financial Officer
            </p>
          </div>
          <div className="flex flex-col gap-4 md:items-end">
            <nav aria-label="Policies" className="flex gap-6">
              {POLICY_LINKS.map(({ href, label }) => (
                <Link
                  key={href}
                  href={href}
                  className="t-caption text-foreground/70 transition-colors hover:text-foreground"
                >
                  {label}
                </Link>
              ))}
            </nav>
            <a
              href="mailto:info@getmizan.net"
              className="t-caption text-foreground/70 transition-colors hover:text-foreground"
            >
              info@getmizan.net
            </a>
          </div>
        </div>

        {/* Bottom bar: endorsement + copyright on one aligned line */}
        <div className="mt-10 flex flex-col gap-3 border-t border-depth-border pt-6 sm:flex-row sm:items-center sm:justify-between">
          <p className="t-micro inline-flex items-center gap-2 text-foreground/55">
            <span
              aria-hidden="true"
              className="inline-block h-1 w-1 shrink-0 rounded-full bg-gold-primary"
            />
            Backed by angel investors &amp; fintech professionals
          </p>
          <p className="t-micro text-foreground/40">
            © 2026 Mizan · Made in Singapore
          </p>
        </div>
      </Container>
    </footer>
  );
}

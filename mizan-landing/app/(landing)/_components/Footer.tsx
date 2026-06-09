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
        <div className="grid gap-8 md:grid-cols-3 md:items-start">
          <div className="space-y-2">
            <Wordmark size="sm" />
            <p className="t-micro text-gold-deep">
              AI-native personal wealth
            </p>
            <p className="t-micro text-foreground/40">
              © 2026 Maeve Models Ltd · Built in London
            </p>
          </div>
          <nav
            aria-label="Policies"
            className="flex gap-6 md:justify-center"
          >
            {POLICY_LINKS.map(({ href, label }) => (
              <Link
                key={href}
                href={href}
                className="t-caption text-foreground/70 hover:text-foreground transition-colors"
              >
                {label}
              </Link>
            ))}
          </nav>
          <div className="flex flex-col gap-2 md:items-end">
            <a
              href="https://twitter.com/Sami_Sayyed1"
              target="_blank"
              rel="noopener noreferrer"
              className="t-caption text-foreground/70 hover:text-foreground transition-colors"
            >
              X · @Sami_Sayyed1
            </a>
            <a
              href="mailto:hello@getmizan.net"
              className="t-caption text-foreground/70 hover:text-foreground transition-colors"
            >
              hello@getmizan.net
            </a>
          </div>
        </div>
      </Container>
    </footer>
  );
}

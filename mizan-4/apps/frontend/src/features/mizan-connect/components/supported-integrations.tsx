/**
/** Mizan Connect — Plaid-supported institution examples. */

import { useState } from "react";

interface SupportedIntegration {
  /** Display name shown to the user. */
  name: string;
  /** Country / region context — keeps the grid scannable for users
   *  who only care about their geography. */
  region: string;
  /** Domain we feed to clearbit's logo CDN. */
  domain: string;
  /** Optional short tagline; mostly for less-obvious providers. */
  tagline?: string;
}

const SUPPORTED_INTEGRATIONS: readonly SupportedIntegration[] = [
  { name: "Interactive Brokers", region: "Global", domain: "interactivebrokers.com" },
  { name: "Moomoo", region: "Asia / US", domain: "moomoo.com" },
  { name: "Charles Schwab", region: "US", domain: "schwab.com" },
  { name: "Fidelity", region: "US", domain: "fidelity.com" },
  { name: "Robinhood", region: "US", domain: "robinhood.com" },
  { name: "Vanguard", region: "US / UK", domain: "vanguard.com" },
  { name: "Wealthsimple", region: "Canada", domain: "wealthsimple.com" },
  { name: "Questrade", region: "Canada", domain: "questrade.com" },
];

export function SupportedIntegrations() {
  return (
    <section aria-labelledby="supported-integrations-heading" className="space-y-3">
      <div className="flex items-center gap-3">
        <h2
          id="supported-integrations-heading"
          className="text-muted-foreground text-xs font-medium uppercase tracking-wider"
        >
          Connect the institutions you already use
        </h2>
        <div className="bg-border h-px flex-1" />
      </div>

      <ul
        className="grid grid-cols-2 gap-2 sm:grid-cols-3 lg:grid-cols-4"
        // Decorative wrapper — each <li> below carries the semantic
        // content. The h2 above is the section's accessible name.
      >
        {SUPPORTED_INTEGRATIONS.map((integration) => (
          <li key={integration.domain} className="contents">
            <IntegrationCard integration={integration} />
          </li>
        ))}
      </ul>

      <p className="text-muted-foreground/80 text-center text-xs">
        …and many more. Plaid Link shows supported banks, cards, and investment platforms when you
        click <span className="font-medium">Connect with Plaid</span>.
      </p>
    </section>
  );
}

function IntegrationCard({ integration }: { integration: SupportedIntegration }) {
  // Clearbit's logo CDN occasionally 404s for less-mainstream domains
  // or rate-limits a session — fall back to a colored letter avatar so
  // the grid never shows a broken-image icon mid-demo. Same pattern
  // as broker-account-card.tsx and broker-sync-state-card.tsx.
  const [logoFailed, setLogoFailed] = useState(false);

  return (
    <div className="border-border bg-card flex min-w-0 items-center gap-3 rounded-lg border p-3">
      <div className="bg-muted text-muted-foreground flex h-9 w-9 shrink-0 items-center justify-center overflow-hidden rounded-md">
        {logoFailed ? (
          <span className="text-xs font-semibold uppercase tabular-nums">
            {integration.name.slice(0, 2)}
          </span>
        ) : (
          <img
            src={`https://logo.clearbit.com/${integration.domain}`}
            alt=""
            className="h-6 w-6"
            onError={() => setLogoFailed(true)}
            referrerPolicy="no-referrer"
            loading="lazy"
          />
        )}
      </div>
      <div className="min-w-0">
        <p className="truncate text-sm font-medium leading-tight">{integration.name}</p>
        <p className="text-muted-foreground truncate text-[11px]">
          {integration.tagline ?? integration.region}
        </p>
      </div>
    </div>
  );
}

export default SupportedIntegrations;

import type { Metadata } from "next";

import { LegalArticle } from "../_components/LegalArticle";

export const metadata: Metadata = {
  title: "Security",
  description:
    "How Mizan protects your wealth data: AES-256-GCM encryption, a hash-chained immutable audit ledger, a native desktop app, and bounded, audited AI access.",
  alternates: { canonical: "/security" },
  openGraph: {
    title: "Security — Mizan",
    description:
      "AES-256-GCM encryption, a hash-chained audit ledger, and security built into the architecture — not bolted on.",
    url: "/security",
  },
};

export default function SecurityPage() {
  return (
    <LegalArticle
      eyebrow="TRUST"
      title="Security"
      intro="Mizan holds the full picture of your wealth, so security isn't a feature — it's the foundation. Here's how we protect it."
      updated="10 June 2026"
    >
      <h2>Security by architecture, not afterthought</h2>
      <p>
        Most apps add security around the edges. Mizan is built the other way:
        the protections below are part of the core engine, not a layer on top.
      </p>

      <h2>Encryption everywhere</h2>
      <ul>
        <li>
          <strong>AES-256-GCM</strong> encryption is applied per provider to
          every connection token — the keys that link your accounts are never
          stored in plain text.
        </li>
        <li>
          <strong>TLS</strong> protects all data in transit, and data is
          encrypted at rest by our infrastructure providers.
        </li>
      </ul>

      <h2>A ledger you can prove</h2>
      <p>
        Every figure in Mizan is written to an <strong>immutable,
        hash-chained audit ledger</strong>. Each entry is cryptographically
        linked to the one before it, so your numbers are tamper-evident and
        verifiable a year later — not a snapshot that silently changes.
      </p>

      <h2>A native app, with bounded AI</h2>
      <p>
        Mizan runs as a native desktop application backed by a Rust-native
        financial-truth engine — not a browser tab holding your credentials. The
        AI layer can read everything it needs, but it can only write back to your
        ledger through an audited, bounded path. It can <em>suggest</em>; it
        cannot silently act.
      </p>

      <h2>We see less than you&apos;d think</h2>
      <p>
        Account connections are designed for read access wherever possible, and
        sensitive bank credentials are handled by regulated aggregators rather
        than stored by us. We practise data minimisation — we keep what&apos;s
        needed to give you an accurate picture, and no more.
      </p>

      <h2>The waitlist, today</h2>
      <p>
        Right now, before launch, the only data we hold is your waitlist email
        and country. It lives in a secured database with row-level security and
        is never sold. See our{" "}
        <a href="/privacy">Privacy Policy</a> for the full detail.
      </p>

      <h2>Responsible disclosure</h2>
      <p>
        If you believe you&apos;ve found a security vulnerability, please tell us
        first at{" "}
        <a href="mailto:info@getmizan.net">info@getmizan.net</a>. We welcome
        good-faith research and will work with you to verify and fix any issue
        before it&apos;s disclosed publicly.
      </p>

      <h2>Before launch</h2>
      <p>
        A full security overview will accompany the product at launch. The
        architecture described here is real and already running through our
        end-to-end test suite.
      </p>
    </LegalArticle>
  );
}

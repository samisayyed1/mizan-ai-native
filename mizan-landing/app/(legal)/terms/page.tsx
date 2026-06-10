import type { Metadata } from "next";

import { LegalArticle } from "../_components/LegalArticle";

export const metadata: Metadata = {
  title: "Terms of Service",
  description:
    "The terms for using getmizan.net and joining the Mizan waitlist. Mizan provides information and insight, not financial, tax, legal, or religious advice.",
  alternates: { canonical: "/terms" },
  openGraph: {
    title: "Terms of Service — Mizan",
    description:
      "The terms for using getmizan.net and joining the Mizan waitlist.",
    url: "/terms",
  },
};

export default function TermsPage() {
  return (
    <LegalArticle
      eyebrow="LEGAL"
      title="Terms of Service"
      intro="The plain-English agreement for using this website and joining the Mizan waitlist before launch."
      updated="10 June 2026"
    >
      <h2>Agreement</h2>
      <p>
        By using <a href="https://getmizan.net">getmizan.net</a> or joining the
        waitlist, you agree to these terms. If you don&apos;t agree, please
        don&apos;t use the site. Mizan is operated from Singapore.
      </p>

      <h2>What Mizan is</h2>
      <p>
        Mizan is an AI-native personal wealth platform, currently in development
        and scheduled to launch in August 2026. This website is a pre-launch
        page where you can register interest. Features described here represent
        our current plans and may evolve before launch.
      </p>

      <h2>The waitlist</h2>
      <p>
        Joining the waitlist registers your interest — it doesn&apos;t create an
        account or guarantee access. Founding-member benefits (priority access,
        founding pricing, early influence) are our genuine intention for early
        members and will be confirmed in the product terms at launch.
      </p>

      <h2>Not financial advice</h2>
      <p>
        This is important. Mizan helps you <strong>see and understand</strong>{" "}
        your wealth — it organises your accounts, surfaces insights, and can{" "}
        <em>suggest</em> actions. It does not provide investment, tax, legal, or
        religious advice, and it does not execute trades or move money on its
        own. Any suggestion is informational; you decide, and you remain
        responsible for your own financial decisions. For advice specific to
        your circumstances, consult a qualified professional.
      </p>

      <h2>Acceptable use</h2>
      <p>You agree not to:</p>
      <ul>
        <li>Submit false information or someone else&apos;s details.</li>
        <li>Attempt to disrupt, probe, or overload the service.</li>
        <li>Use the site for any unlawful purpose.</li>
      </ul>

      <h2>Intellectual property</h2>
      <p>
        The Mizan name, logo, copy, design, and software are owned by us. You may
        not copy or reuse them without permission.
      </p>

      <h2>&ldquo;As is&rdquo; and liability</h2>
      <p>
        This pre-launch website is provided on an &ldquo;as is&rdquo; basis,
        without warranties of any kind. To the fullest extent permitted by law,
        we are not liable for any indirect or consequential loss arising from
        your use of the site. Nothing here limits liability that cannot be
        limited by law.
      </p>

      <h2>Governing law</h2>
      <p>
        These terms are governed by the laws of Singapore, and the courts of
        Singapore have exclusive jurisdiction over any dispute.
      </p>

      <h2>Changes</h2>
      <p>
        We may update these terms; the date above shows the latest version.
        Continued use of the site means you accept the current terms.
      </p>
    </LegalArticle>
  );
}

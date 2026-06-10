import type { Metadata } from "next";

import { LegalArticle } from "../_components/LegalArticle";

export const metadata: Metadata = {
  title: "Privacy Policy",
  description:
    "How Mizan collects, uses, and protects your data. For the waitlist we store only your email and country, never sell your data, and you can unsubscribe or delete anytime.",
  alternates: { canonical: "/privacy" },
  openGraph: {
    title: "Privacy Policy — Mizan",
    description:
      "How Mizan collects, uses, and protects your data. We store only what we need and never sell it.",
    url: "/privacy",
  },
};

export default function PrivacyPage() {
  return (
    <LegalArticle
      eyebrow="LEGAL"
      title="Privacy Policy"
      intro="Your wealth is private. So is your data with us. This policy explains exactly what we collect, why, and the control you have over it."
      updated="10 June 2026"
    >
      <h2>Who we are</h2>
      <p>
        Mizan (&ldquo;we&rdquo;, &ldquo;us&rdquo;) operates the website{" "}
        <a href="https://getmizan.net">getmizan.net</a> and is building an
        AI-native personal wealth platform. We are based in Singapore. For any
        privacy question, contact{" "}
        <a href="mailto:info@getmizan.net">info@getmizan.net</a>.
      </p>

      <h2>What we collect</h2>
      <p>
        At this pre-launch stage, the only personal data we collect is what you
        give us when you join the waitlist:
      </p>
      <ul>
        <li>
          <strong>Email address</strong> — so we can send you launch updates.
        </li>
        <li>
          <strong>Country</strong> — so we know which markets to open first.
        </li>
      </ul>
      <p>
        We also process limited technical data automatically: your IP address is
        used transiently to rate-limit the signup form and prevent abuse, and we
        measure aggregate, anonymous traffic with a privacy-first analytics tool
        (Plausible) that uses <strong>no cookies</strong> and does not track you
        across sites.
      </p>

      <h2>How we use it</h2>
      <ul>
        <li>To send you one confirmation email and occasional launch updates.</li>
        <li>To gauge interest and prioritise the markets we launch in.</li>
        <li>To protect the service from spam and abuse.</li>
      </ul>
      <p>
        We rely on your <strong>consent</strong> (joining the waitlist) and our
        legitimate interest in operating and securing the service. We do not use
        your data for automated decisions that affect you.
      </p>

      <h2>Who we share it with</h2>
      <p>
        We never sell your data. We share it only with the processors that run
        our infrastructure, each bound to protect it:
      </p>
      <ul>
        <li>
          <strong>Supabase</strong> — secure database that stores your signup.
        </li>
        <li>
          <strong>Resend</strong> — sends your confirmation and launch emails.
        </li>
        <li>
          <strong>Netlify</strong> — hosts the website.
        </li>
        <li>
          <strong>Plausible</strong> — cookieless, aggregate analytics.
        </li>
      </ul>

      <h2>How long we keep it</h2>
      <p>
        We keep your waitlist details until launch and a reasonable period
        after, or until you ask us to delete them — whichever comes first. Every
        email we send includes a one-click unsubscribe.
      </p>

      <h2>Your rights</h2>
      <p>
        You can ask us to access, correct, export, or delete your data, and you
        can withdraw consent at any time. Just email{" "}
        <a href="mailto:info@getmizan.net">info@getmizan.net</a> and we&apos;ll
        action it promptly. Depending on where you live, you may have additional
        rights under laws such as the GDPR or Singapore&apos;s PDPA.
      </p>

      <h2>International transfers</h2>
      <p>
        Our processors may store data in regions outside your own. Where they do,
        they apply recognised safeguards to keep your data protected to the
        standard described here.
      </p>

      <h2>Children</h2>
      <p>
        Mizan is intended for adults managing their own wealth. It is not
        directed at anyone under 18, and we do not knowingly collect their data.
      </p>

      <h2>Changes</h2>
      <p>
        If we change this policy, we&apos;ll update the date above and, for
        material changes, notify waitlist members by email.
      </p>
    </LegalArticle>
  );
}

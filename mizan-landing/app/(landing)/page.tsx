/**
 * Mizan waitlist landing — composition.
 *
 * Section order is the user journey: hook (Hero) → ground (Trust) →
 * problem → product → AI moat → Zakat moat → conversion (Founding
 * offer) → human (Founder) → close (Footer).
 */
import { AILayer } from "./_components/AILayer";
import { Footer } from "./_components/Footer";
import { FounderNote } from "./_components/FounderNote";
import { FoundingOffer } from "./_components/FoundingOffer";
import { Hero } from "./_components/Hero";
import { Problem } from "./_components/Problem";
import { Product } from "./_components/Product";
import { TrustStrip } from "./_components/TrustStrip";

// NOTE: The FAQ section is intentionally removed for now but its
// component is preserved at ./_components/FAQ.tsx. To restore it, import
// { FAQ } and drop <FAQ /> back between <AILayer /> and <FoundingOffer />.

export default function LandingPage() {
  return (
    <main id="main">
      <Hero />
      <TrustStrip />
      <Problem />
      <Product />
      <AILayer />
      <FoundingOffer />
      <FounderNote />
      <Footer />
    </main>
  );
}

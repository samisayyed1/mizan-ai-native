import Link from "next/link";
import { ArrowLeft } from "lucide-react";

import { Container } from "@/app/(landing)/_primitives/Container";
import { Wordmark } from "@/app/(landing)/_primitives/Wordmark";
import { Footer } from "@/app/(landing)/_components/Footer";

export default function LegalLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <div className="flex min-h-screen flex-col bg-depth-page">
      <header className="border-b border-depth-border">
        <Container className="flex h-16 items-center justify-between">
          <Link href="/" aria-label="Mizan home">
            <Wordmark size="sm" />
          </Link>
          <Link
            href="/"
            className="inline-flex items-center gap-1.5 t-caption text-foreground/70 transition-colors hover:text-foreground"
          >
            <ArrowLeft className="h-3.5 w-3.5" /> Back to home
          </Link>
        </Container>
      </header>

      <main id="main" className="flex-1">
        {children}
      </main>

      <Footer />
    </div>
  );
}

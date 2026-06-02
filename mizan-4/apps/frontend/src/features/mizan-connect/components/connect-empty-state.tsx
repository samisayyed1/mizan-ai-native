import { ExternalLink } from "@/components/external-link";
import { Button } from "@mizan/ui/components/ui/button";
import { Icons } from "@mizan/ui/components/ui/icons";
import { Link } from "react-router-dom";
import { useCreateBrokerLoginPortal } from "../hooks";
import { useMizanConnect } from "../providers/mizan-connect-provider";
import { ConnectFlowDiagram } from "./connect-flow-diagram";
import { SupportedIntegrations } from "./supported-integrations";

const features = [
  {
    icon: Icons.CloudSync2,
    title: "Plaid Sync",
    description: "Banks, cards, investments, and liabilities.",
    color: "orange",
  },
  {
    icon: Icons.Devices,
    title: "Device Sync",
    description: "Sync across devices with E2E encryption.",
    color: "green",
  },
  {
    icon: Icons.UserSwitch,
    title: "Household View",
    description: "Share accounts with family members.",
    color: "blue",
  },
];

const colorClasses = {
  orange: {
    bg: "bg-orange-100 dark:bg-orange-900/30",
    icon: "text-orange-600 dark:text-orange-400",
  },
  blue: {
    bg: "bg-blue-100 dark:bg-blue-900/30",
    icon: "text-blue-600 dark:text-blue-400",
  },
  green: {
    bg: "bg-green-100 dark:bg-green-900/30",
    icon: "text-green-600 dark:text-green-400",
  },
};

export function ConnectEmptyState() {
  const { isConnected } = useMizanConnect();
  const [portal, isPolling] = useCreateBrokerLoginPortal();

  const handleConnect = () => {
    portal.mutate(undefined);
  };
  const isBusy = portal.isPending || isPolling;

  return (
    <div className="flex min-h-[calc(100vh-12rem)] flex-col items-center justify-center px-4 py-6">
      <div className="w-full max-w-3xl space-y-8 sm:space-y-12">
        {/* Header with Logo */}
        <header className="text-center">
          <img alt="Mizan" className="mx-auto mb-4 h-16 w-16" src="/logo-vantage.png" />
          <div className="bg-secondary text-secondary-foreground mb-3 inline-flex items-center gap-1.5 rounded-full px-2.5 py-0.5 text-xs font-medium">
            <Icons.Sparkles className="h-3 w-3" />
            Optional
          </div>
          <h1 className="mb-2 text-xl font-semibold tracking-tight">Mizan Connect</h1>
          <p className="text-muted-foreground text-sm">
            Connect once. Mizan quietly keeps your complete wealth picture alive.
          </p>
        </header>

        {/* Hero Diagram - constrained width */}
        <section className="mx-auto max-w-2xl">
          <ConnectFlowDiagram />
        </section>

        {/* Features - responsive grid */}
        <section className="grid grid-cols-1 gap-4 sm:grid-cols-3 sm:gap-6">
          {features.map((feature) => {
            const colors = colorClasses[feature.color as keyof typeof colorClasses];
            return (
              <div key={feature.title} className="flex items-center gap-3">
                <div className={`shrink-0 rounded-xl p-2.5 ${colors.bg}`}>
                  <feature.icon className={`h-5 w-5 ${colors.icon}`} />
                </div>
                <div className="min-w-0">
                  <h3 className="text-sm font-medium">{feature.title}</h3>
                  <p className="text-muted-foreground text-xs">{feature.description}</p>
                </div>
              </div>
            );
          })}
        </section>

        {/* Supported integrations — names + logos so the user can see
            their broker is on the list before they click Connect. */}
        <SupportedIntegrations />

        {/* CTA */}
        <footer className="flex flex-col items-center gap-4">
          <div className="flex w-full flex-col gap-3 sm:w-auto sm:flex-row sm:items-center sm:justify-center">
            {isConnected ? (
              <Button
                onClick={handleConnect}
                disabled={isBusy}
                className="from-primary to-primary/90 bg-linear-to-r w-full sm:w-auto"
              >
                {isBusy ? (
                  <>
                    <Icons.Spinner className="mr-1.5 h-4 w-4 animate-spin" />
                    {isPolling ? "Waiting for Plaid..." : "Opening Plaid..."}
                  </>
                ) : (
                  <>
                    Connect with Plaid
                    <Icons.ExternalLink className="ml-1.5 h-4 w-4" />
                  </>
                )}
              </Button>
            ) : (
              <Button variant="outline" asChild className="w-full sm:w-auto">
                <Link to="/settings/connect">
                  <Icons.User className="mr-1.5 h-4 w-4" />
                  Sign in to Mizan Connect first
                </Link>
              </Button>
            )}
          </div>
          <p className="text-muted-foreground/70 text-xs">
            Plaid handles authentication. Mizan Connect stores access tokens only in the encrypted
            server-side boundary.
          </p>
          <ExternalLink
            href="https://mizan.app"
            className="text-muted-foreground hover:text-foreground inline-flex items-center gap-1 text-xs transition-colors"
          >
            Learn more about Connect
            <Icons.ExternalLink className="h-3 w-3" />
          </ExternalLink>
        </footer>
      </div>
    </div>
  );
}

import { ApplicationShell } from "@mizan/ui";
import { Icons } from "@mizan/ui/components/ui/icons";
import { Separator } from "@mizan/ui/components/ui/separator";
import { Outlet, useLocation, useNavigate } from "react-router-dom";
import { SidebarNav } from "./sidebar-nav";

const settingsSections = [
  {
    title: "Preferences",
    items: [
      {
        title: "General",
        href: "general",
        subtitle: "Currency, timezone, and app behavior",
        icon: <Icons.Settings2 className="size-5" />,
      },
      {
        title: "Appearance",
        href: "appearance",
        subtitle: "Theme, font, and menu bar",
        icon: <Icons.Monitor className="size-5" />,
      },
      // Privacy entry deliberately removed for now. The previous version
      // pointed to href: "general" — meaning clicking "Privacy" sent the
      // user to the General page, which has no privacy controls. Dead
      // nav. When dedicated privacy controls land (balance masking
      // toggle, local-first opt-outs) we can re-add this entry with a
      // real route. Until then, balance privacy is exposed via the
      // dashboard's eye-toggle and that's reachable from anywhere.
    ],
  },
  {
    title: "Wealth",
    items: [
      {
        title: "Portfolios",
        href: "accounts",
        subtitle: "Local portfolios and account organization",
        icon: <Icons.CreditCard className="size-5" />,
      },
      {
        title: "Zakat",
        href: "/zakat",
        subtitle: "Estimated zakat assumptions and calculation",
        icon: <Icons.Coins className="size-5" />,
      },
      {
        title: "Reports",
        href: "/reports",
        subtitle: "AI-native summaries and computed snapshots",
        icon: <Icons.FileText className="size-5" />,
      },
    ],
  },
  {
    title: "Connections",
    items: [
      {
        title: "Mizan Connect",
        href: "connect",
        subtitle: "Subscription and encrypted cloud session",
        icon: <Icons.CloudSync2 className="text-primary size-6" />,
      },
      {
        title: "Plaid Sync",
        href: "/connect",
        subtitle: "Gold live accounts, liabilities, transactions, and holdings",
        icon: <Icons.Link className="size-5" />,
      },
    ],
  },
  {
    title: "AI",
    items: [
      {
        title: "Settings",
        href: "ai-providers",
        subtitle: "Mizan AI plus optional bring-your-own providers",
        icon: <Icons.SparklesOutline className="size-5" />,
      },
    ],
  },
  {
    title: "Advanced",
    items: [
      {
        title: "Data Classification",
        href: "taxonomies",
        subtitle: "Asset classification hierarchies",
        icon: <Icons.Blocks className="size-5" />,
      },
      {
        title: "Backup & Export",
        href: "exports",
        subtitle: "Local backup files and data exports",
        icon: <Icons.Download className="size-5" />,
      },
      {
        title: "Diagnostics",
        href: "about",
        subtitle: "Version, build, and runtime diagnostics",
        icon: <Icons.InfoCircle className="size-5" />,
      },
    ],
  },
];

export default function SettingsLayout() {
  const location = useLocation();
  const navigate = useNavigate();

  const sections = settingsSections;

  // Check if we're on the main settings page (mobile) or a specific setting page
  const isMainSettingsPage =
    location.pathname === "/settings" || location.pathname === "/settings/";

  // Mobile-first: show list view on main page, detail view on specific pages
  return (
    <ApplicationShell className="settings-root app-shell h-screen overflow-x-hidden">
      {/* Mobile Layout */}
      <div className="w-full lg:hidden">
        {isMainSettingsPage ? (
          // Mobile Settings List View (carded list with dividers)
          <div className="scan-hide-target w-full max-w-full overflow-x-hidden">
            <div className="bg-background/95 supports-backdrop-filter:bg-background/60 pt-safe sticky top-0 z-10 border-b backdrop-blur">
              <div className="flex min-h-[60px] items-center justify-center px-4">
                <h1 className="text-lg font-semibold">Settings</h1>
              </div>
            </div>
            <div className="space-y-6 p-3 pb-[calc(var(--mobile-nav-ui-height)+max(var(--mobile-nav-gap),env(safe-area-inset-bottom)))] lg:p-4 lg:pb-4">
              {sections.map((section) => (
                <div key={section.title} className="space-y-3">
                  <div className="text-muted-foreground px-2 text-xs font-semibold uppercase tracking-widest">
                    {section.title}
                  </div>
                  <div className="divide-border bg-card divide-y overflow-hidden rounded-2xl border shadow-sm">
                    {section.items.map((item) => (
                      <button
                        type="button"
                        key={item.href}
                        onClick={() =>
                          navigate(item.href.startsWith("/") ? item.href : `/settings/${item.href}`)
                        }
                        className="hover:bg-muted/40 flex w-full items-center justify-between gap-3 px-4 py-4 text-left transition-colors active:opacity-90"
                        aria-label={item.title}
                      >
                        <div className="flex min-w-0 flex-1 items-center gap-3">
                          <div className="text-muted-foreground shrink-0">{item.icon}</div>
                          <div className="min-w-0">
                            <div className="text-foreground truncate text-base font-medium">
                              {item.title}
                            </div>
                            {item?.subtitle && (
                              <div className="text-muted-foreground truncate text-sm">
                                {item.subtitle}
                              </div>
                            )}
                          </div>
                        </div>
                        <Icons.ChevronRight className="text-muted-foreground h-4 w-4 shrink-0" />
                      </button>
                    ))}
                  </div>
                </div>
              ))}
            </div>
          </div>
        ) : (
          <div className="scan-hide-target pt-safe w-full max-w-full overflow-x-hidden">
            <div className="w-full max-w-full overflow-x-hidden scroll-smooth">
              <div className="p-2 pb-[calc(var(--mobile-nav-ui-height)+max(var(--mobile-nav-gap),env(safe-area-inset-bottom)))] lg:p-4 lg:pb-4">
                <Outlet />
              </div>
            </div>
          </div>
        )}
      </div>

      {/* Desktop Layout */}
      <div className="hidden lg:flex lg:w-full lg:justify-start">
        <div className="flex w-full max-w-6xl flex-col px-2 py-8">
          <div className="space-y-0.5">
            <h2 className="text-2xl font-bold tracking-tight">Settings</h2>
          </div>
          <Separator className="my-6" />
          <div className="flex gap-10">
            <aside className="hidden w-[240px] shrink-0 lg:sticky lg:top-24 lg:flex lg:flex-col lg:self-start">
              <div className="space-y-6">
                {sections.map((section) => (
                  <div key={section.title} className="space-y-2">
                    <div className="text-muted-foreground pl-2 text-sm font-light uppercase tracking-widest">
                      {section.title}
                    </div>
                    <SidebarNav items={section.items} />
                  </div>
                ))}
              </div>
            </aside>
            <div className="mb-8 min-w-0 flex-1">
              <Outlet />
            </div>
          </div>
        </div>
      </div>
    </ApplicationShell>
  );
}

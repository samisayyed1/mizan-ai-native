import { getDynamicNavItems, subscribeToNavigationUpdates } from "@/addons/addons-runtime-context";
import { Icons } from "@mizan/ui/components/ui/icons";
import { useEffect, useState } from "react";

/**
 * Navigation entry shape.
 *
 * Most entries are plain `<Link>` destinations (`href`). Items that should
 * trigger an in-app action instead of navigation (Assistant intent) set
 * `action` — the rendering layer (`app-sidebar.tsx`, `mobile-navbar.tsx`)
 * detects it and renders a button instead of a Link.
 */
export interface NavLink {
  title: string;
  href: string;
  icon?: React.ReactNode;
  keywords?: string[];
  label?: string; // Optional descriptive label for launcher/search
  /** Discriminator for special nav behaviors. Today: opens Assistant asset ingest. */
  action?: "openAssistantAddAsset";
}

export interface NavigationProps {
  primary: NavLink[];
  secondary?: NavLink[];
  addons?: NavLink[];
}

/**
 * The simplified top-level nav: Home / Portfolio / Add / Assistant. Older
 * surfaces (Insights, Holdings, News, Activities, Goals,
 * Connect) stay reachable through Settings → Advanced but don't crowd the
 * primary nav anymore — they were the #1 source of "I can't find anything"
 * feedback on first-time users.
 */
const staticNavigation: NavigationProps = {
  primary: [
    {
      icon: <Icons.Dashboard className="size-6" />,
      title: "Home",
      href: "/",
      keywords: ["home", "dashboard", "overview", "summary"],
      label: "Home",
    },
    {
      icon: <Icons.Briefcase className="size-6" />,
      title: "Portfolio",
      href: "/portfolio",
      keywords: ["portfolio", "portfolios", "holdings", "assets", "positions"],
      label: "Portfolio",
    },
    {
      icon: <Icons.Target className="size-6" />,
      title: "Goals",
      href: "/goals",
      keywords: ["goals", "retirement", "fire", "planning", "savings"],
      label: "Goals",
    },
    {
      icon: <Icons.Plus className="size-6" />,
      title: "Add",
      // `/add` is a sentinel — the renderer triggers `action` instead of
      // navigating. Kept as a non-empty string so search/launcher logic still
      // has a stable key.
      href: "/add",
      keywords: ["add", "create", "new", "stock", "bank", "property", "gold", "liability"],
      label: "Add asset",
      action: "openAssistantAddAsset",
    },
    {
      icon: <Icons.Sparkles className="size-6" />,
      title: "Assistant",
      href: "/assistant",
      keywords: ["ai", "assistant", "chat", "help", "ask"],
      label: "AI Assistant",
    },
  ],
  secondary: [
    {
      icon: <Icons.Settings className="size-6" />,
      title: "Settings",
      href: "/settings",
      keywords: ["preferences", "config", "configuration", "connect", "sync", "broker"],
    },
  ],
};

export function useNavigation() {
  const [dynamicItems, setDynamicItems] = useState<NavigationProps["addons"]>([]);

  // Subscribe to navigation updates from addons
  useEffect(() => {
    const updateDynamicItems = () => {
      const itemsFromRuntime = getDynamicNavItems();
      setDynamicItems(itemsFromRuntime);
    };

    // Initial load
    updateDynamicItems();

    // Subscribe to updates
    const unsubscribe = subscribeToNavigationUpdates(updateDynamicItems);

    return () => {
      unsubscribe();
    };
  }, []);

  // Combine static navigation items with addons grouped separately.
  // Hide desktop-only features (FIRE Planner) in web mode.
  const primary = staticNavigation.primary;
  const navigation: NavigationProps = {
    primary,
    secondary: staticNavigation.secondary,
    addons: dynamicItems,
  };

  return navigation;
}

export function isPathActive(pathname: string, href: string): boolean {
  if (!href) {
    return false;
  }

  const ensureLeadingSlash = href.startsWith("/") ? href : `/${href}`;
  const normalize = (value: string) => {
    if (value.length > 1 && value.endsWith("/")) {
      return value.slice(0, -1);
    }
    return value;
  };

  const normalizedHref = normalize(ensureLeadingSlash);
  const normalizedPath = normalize(pathname);

  if (normalizedHref === "/") {
    // Home matches just `/` and `/dashboard` (the legacy alias). Net Worth
    // moved off the main nav in M2 and lives under Portfolio / Settings now.
    return normalizedPath === "/" || normalizedPath === "/dashboard";
  }

  // Portfolio tab also lights up when drilled into a specific portfolio
  // (`/accounts/:id`) — that's the same primary destination from the user's
  // perspective.
  if (normalizedHref === "/portfolio") {
    return (
      normalizedPath === "/portfolio" ||
      normalizedPath.startsWith("/portfolio/") ||
      normalizedPath.startsWith("/accounts/")
    );
  }

  // The Add sentinel never matches a real path — it always opens Assistant.
  if (normalizedHref === "/add") {
    return false;
  }

  return normalizedPath === normalizedHref || normalizedPath.startsWith(`${normalizedHref}/`);
}

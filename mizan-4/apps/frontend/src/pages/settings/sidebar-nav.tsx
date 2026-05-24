import { NavLink, useLocation } from "react-router-dom";
import { ReactNode } from "react";

import { cn } from "@/lib/utils";
import { buttonVariants } from "@mizan/ui/components/ui/button-variants";

interface SidebarNavProps extends React.HTMLAttributes<HTMLElement> {
  items: {
    href: string;
    title: string;
    icon?: ReactNode;
  }[];
}

export function SidebarNav({ className, items, ...props }: SidebarNavProps) {
  const location = useLocation();

  return (
    <nav className={cn("flex flex-col space-y-1", className)} {...props}>
      {items.map((item) => {
        // Force absolute resolution. Relative `to="connect"` is correct in
        // theory, but defensive against React Router's relative-path edge
        // cases when there's a same-named route at a different ancestor
        // (we have <Route path="connect"> at both AppLayout and SettingsLayout).
        const target = item.href.startsWith("/") ? item.href : `/settings/${item.href}`;
        const isActive = location.pathname === target;
        return (
          <NavLink
            key={item.href}
            to={target}
            className={cn(
              buttonVariants({ variant: "ghost" }),
              "h-9 justify-start rounded-md px-2 [&_svg]:size-4",
              isActive ? "bg-muted hover:bg-muted" : "hover:bg-muted/50",
            )}
          >
            {item.icon && <span className="mr-1.5 hidden lg:inline-block">{item.icon}</span>}
            {item.title}
          </NavLink>
        );
      })}
    </nav>
  );
}

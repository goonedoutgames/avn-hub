import { NavLink, Outlet } from "react-router-dom";
import { Library, Link2, Settings } from "lucide-react";
import { GlobalTaskBar } from "@/components/GlobalTaskBar";
import { MigrationBanner } from "@/components/LibraryMigrationCard";
import { cn } from "@/lib/utils";
import logo from "@/assets/avn-hub-logo.webp";

const nav = [
  { to: "/", label: "Library", icon: Library },
  { to: "/match", label: "Match", icon: Link2 },
  { to: "/settings", label: "Settings", icon: Settings },
];

function NavItem({
  to,
  label,
  icon: Icon,
  end,
  mobile,
}: {
  to: string;
  label: string;
  icon: typeof Library;
  end?: boolean;
  mobile?: boolean;
}) {
  return (
    <NavLink
      to={to}
      end={end}
      className={({ isActive }) =>
        cn(
          "flex items-center transition-colors",
          mobile
            ? cn(
                "flex-1 flex-col gap-1 py-2 text-[10px]",
                isActive
                  ? "text-[var(--color-primary)]"
                  : "text-[var(--color-muted-foreground)]",
              )
            : cn(
                "gap-2 rounded-md px-3 py-2 text-sm",
                isActive
                  ? "bg-[var(--color-primary)] text-[var(--color-primary-foreground)]"
                  : "text-[var(--color-muted-foreground)] hover:bg-[var(--color-accent)] hover:text-[var(--color-foreground)]",
              ),
        )
      }
    >
      <Icon className={mobile ? "h-5 w-5" : "h-4 w-4"} />
      {label}
    </NavLink>
  );
}

export function Layout() {
  return (
    <div className="min-h-screen">
      <header className="border-b border-[var(--color-border)] bg-[var(--color-card)]">
        <div className="mx-auto flex max-w-7xl items-center justify-between gap-3 px-4 py-3 md:py-4">
          <div className="flex min-w-0 items-center gap-3">
            <img
              src={logo}
              alt="AVN Hub"
              className="h-10 w-10 shrink-0 rounded-lg object-cover md:h-12 md:w-12"
            />
            <div className="min-w-0">
              <h1 className="truncate font-semibold leading-none">AVN Hub</h1>
              <p className="hidden text-xs text-[var(--color-muted-foreground)] sm:block">
                Library & Metadata Manager
              </p>
            </div>
          </div>
          <nav className="hidden gap-1 md:flex">
            {nav.map(({ to, label, icon }) => (
              <NavItem key={to} to={to} label={label} icon={icon} end={to === "/"} />
            ))}
          </nav>
        </div>
      </header>

      <main className="mx-auto max-w-7xl px-4 py-6 pb-24 md:py-8 md:pb-28">
        <MigrationBanner />
        <Outlet />
      </main>

      <nav
        aria-label="Main navigation"
        className="fixed inset-x-0 bottom-0 z-40 border-t border-[var(--color-border)] bg-[var(--color-card)]/95 backdrop-blur-sm md:hidden"
      >
        <div className="mx-auto flex max-w-7xl">
          {nav.map(({ to, label, icon }) => (
            <NavItem
              key={to}
              to={to}
              label={label}
              icon={icon}
              end={to === "/"}
              mobile
            />
          ))}
        </div>
      </nav>

      <GlobalTaskBar />
    </div>
  );
}

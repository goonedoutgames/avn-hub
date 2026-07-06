import { NavLink, Outlet } from "react-router-dom";
import { Library, Link2, Settings } from "lucide-react";
import { GlobalTaskBar } from "@/components/GlobalTaskBar";
import { cn } from "@/lib/utils";
import logo from "@/assets/avn-hub-logo.webp";

const nav = [
  { to: "/", label: "Library", icon: Library },
  { to: "/match", label: "Match", icon: Link2 },
  { to: "/settings", label: "Settings", icon: Settings },
];

export function Layout() {
  return (
    <div className="min-h-screen">
      <header className="border-b border-[var(--color-border)] bg-[var(--color-card)]">
        <div className="mx-auto flex max-w-7xl items-center justify-between px-4 py-4">
          <div className="flex items-center gap-3">
            <img
              src={logo}
              alt="AVN Hub"
              className="min-h-24 min-w-24 max-h-24 max-w-24 rounded-lg object-cover"
            />
            <div>
              <h1 className="font-semibold leading-none">AVN Hub</h1>
              <p className="text-xs text-[var(--color-muted-foreground)]">
                Library & Metadata Manager
              </p>
            </div>
          </div>
          <nav className="flex gap-1">
            {nav.map(({ to, label, icon: Icon }) => (
              <NavLink
                key={to}
                to={to}
                end={to === "/"}
                className={({ isActive }) =>
                  cn(
                    "flex items-center gap-2 rounded-md px-3 py-2 text-sm transition-colors",
                    isActive
                      ? "bg-[var(--color-primary)] text-[var(--color-primary-foreground)]"
                      : "text-[var(--color-muted-foreground)] hover:bg-[var(--color-accent)] hover:text-[var(--color-foreground)]",
                  )
                }
              >
                <Icon className="h-4 w-4" />
                {label}
              </NavLink>
            ))}
          </nav>
        </div>
      </header>
      <main className="mx-auto max-w-7xl px-4 py-8 pb-28">
        <Outlet />
      </main>
      <GlobalTaskBar />
    </div>
  );
}

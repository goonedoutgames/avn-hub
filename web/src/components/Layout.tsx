import { useEffect, useState } from "react";
import { NavLink, Outlet } from "react-router-dom";
import { Menu, X } from "lucide-react";
import { useAuth } from "@/context/AuthContext";
import logo from "@/assets/avn-hub-logo.webp";

const links = [
  { to: "/", label: "Library" },
  { to: "/browse", label: "Browse" },
  { to: "/settings", label: "Settings" },
];

export function Layout() {
  const { configured, logout } = useAuth();
  const [open, setOpen] = useState(false);

  useEffect(() => {
    const mq = window.matchMedia("(min-width: 768px)");
    const onChange = () => {
      if (mq.matches) setOpen(false);
    };
    onChange();
    mq.addEventListener("change", onChange);
    return () => mq.removeEventListener("change", onChange);
  }, []);

  return (
    <div className="min-h-screen">
      <header className="sticky top-0 z-20 border-b border-[var(--border)] bg-[color-mix(in_srgb,var(--bg)_88%,transparent)] backdrop-blur-md">
        <div className="mx-auto flex max-w-[1500px] items-center gap-3 px-4 py-3 sm:gap-6">
          <NavLink
            to="/"
            className="flex items-center gap-3 text-[var(--text)]"
            onClick={() => setOpen(false)}
          >
            <img src={logo} alt="AVN Hub" className="h-9 w-9 rounded-lg object-cover" />
            <span className="text-lg font-semibold tracking-wide">AVN Hub</span>
          </NavLink>

          <nav className="ml-auto hidden items-center gap-1 md:flex">
            {links.map((l) => (
              <NavLink
                key={l.to}
                to={l.to}
                end={l.to === "/"}
                className={({ isActive }) =>
                  `rounded-lg px-3 py-1.5 text-sm ${
                    isActive
                      ? "bg-[var(--bg-soft)] text-[var(--text)]"
                      : "text-[var(--muted)] hover:text-[var(--text)]"
                  }`
                }
              >
                {l.label}
              </NavLink>
            ))}
            {configured && (
              <button type="button" className="btn btn-sm ml-2" onClick={() => void logout()}>
                Log out
              </button>
            )}
          </nav>

          <div className="ml-auto md:hidden">
            <button
              type="button"
              className="btn btn-sm"
              aria-label={open ? "Close menu" : "Open menu"}
              aria-expanded={open}
              onClick={() => setOpen((v) => !v)}
            >
              {open ? <X className="h-4 w-4" /> : <Menu className="h-4 w-4" />}
            </button>
          </div>
        </div>

        {open && (
          <div className="border-t border-[var(--border)] px-4 py-3 md:hidden">
            <nav className="flex flex-col gap-1">
              {links.map((l) => (
                <NavLink
                  key={l.to}
                  to={l.to}
                  end={l.to === "/"}
                  onClick={() => setOpen(false)}
                  className={({ isActive }) =>
                    `rounded-lg px-3 py-2.5 text-sm ${
                      isActive
                        ? "bg-[var(--bg-soft)] text-[var(--text)]"
                        : "text-[var(--muted)]"
                    }`
                  }
                >
                  {l.label}
                </NavLink>
              ))}
              {configured && (
                <button
                  type="button"
                  className="btn mt-2 w-full"
                  onClick={() => {
                    setOpen(false);
                    void logout();
                  }}
                >
                  Log out
                </button>
              )}
            </nav>
          </div>
        )}
      </header>
      <main className="mx-auto max-w-[1500px] px-3 py-5 sm:px-4 sm:py-6">
        <Outlet />
      </main>
    </div>
  );
}

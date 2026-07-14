import { useState, type ReactNode } from "react";
import { MoreHorizontal } from "lucide-react";
import { Sheet } from "@/components/ui/sheet";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

export interface ActionMenuItem {
  key: string;
  label: string;
  icon?: ReactNode;
  onClick: () => void;
  disabled?: boolean;
  variant?: "default" | "destructive" | "secondary" | "outline";
  hidden?: boolean;
}

interface MobileActionMenuProps {
  label?: string;
  items: ActionMenuItem[];
  className?: string;
}

export function MobileActionMenu({
  label = "Actions",
  items,
  className,
}: MobileActionMenuProps) {
  const [open, setOpen] = useState(false);
  const visible = items.filter((item) => !item.hidden);

  if (visible.length === 0) return null;

  return (
    <>
      <Button
        type="button"
        variant="secondary"
        size="sm"
        className={cn("md:hidden", className)}
        onClick={() => setOpen(true)}
      >
        <MoreHorizontal className="h-4 w-4" />
        {label}
      </Button>
      <Sheet
        open={open}
        onOpenChange={setOpen}
        side="right"
        title={label}
        description="Choose an action"
      >
        <div className="flex flex-col gap-2">
          {visible.map((item) => (
            <Button
              key={item.key}
              type="button"
              variant={item.variant ?? "outline"}
              className="w-full justify-start"
              disabled={item.disabled}
              onClick={() => {
                setOpen(false);
                item.onClick();
              }}
            >
              {item.icon}
              {item.label}
            </Button>
          ))}
        </div>
      </Sheet>
    </>
  );
}

interface ResponsiveActionsProps {
  menuLabel?: string;
  menuItems: ActionMenuItem[];
  children: ReactNode;
}

/** Inline buttons on md+, collapsed menu on small screens. */
export function ResponsiveActions({
  menuLabel,
  menuItems,
  children,
}: ResponsiveActionsProps) {
  return (
    <>
      <div className="hidden flex-wrap gap-2 md:flex">{children}</div>
      <MobileActionMenu label={menuLabel} items={menuItems} />
    </>
  );
}

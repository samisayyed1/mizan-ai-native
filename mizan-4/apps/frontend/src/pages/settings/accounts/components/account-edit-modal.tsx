import { Dialog, DialogContent } from "@mizan/ui/components/ui/dialog";
import { useIsMobileViewport } from "@/hooks/use-platform";
import { useSettingsContext } from "@/lib/settings-provider";
import type { Account } from "@/lib/types";
import { AccountForm } from "./account-form";

export interface AccountEditModalProps {
  account?: Account;
  open?: boolean;
  onClose?: () => void;
}

export function AccountEditModal({ account, open, onClose }: AccountEditModalProps) {
  const { settings } = useSettingsContext();

  const defaultValues = {
    id: account?.id ?? undefined,
    name: account?.name ?? "",
    balance: account?.balance ?? 0,
    accountType: (account?.accountType ?? "SECURITIES") as "SECURITIES" | "CASH" | "CRYPTOCURRENCY",
    group: account?.group ?? undefined,
    currency: account?.currency ?? settings?.baseCurrency ?? "USD",
    isDefault: account?.isDefault ?? false,
    isActive: account?.id ? account?.isActive : true,
    isArchived: account?.isArchived ?? false,
    trackingMode: account?.trackingMode,
    meta: account?.meta,
  };

  return (
    <Dialog open={open} onOpenChange={onClose} useIsMobile={useIsMobileViewport}>
      {/* Scrollable form body + sticky footer.
          The previous implementation put `overflow-y-auto` on the
          DialogContent itself, which scrolled the Save / Cancel buttons
          off-screen on tall forms (long account groups, alert banners,
          etc.) — users couldn't see the action they needed to take.
          Switching the modal to a flex column with the fields body
          scrolling internally keeps the footer always visible at the
          bottom of the dialog regardless of form length. */}
      <DialogContent className="flex max-h-[90vh] flex-col gap-0 overflow-hidden p-0 sm:max-w-[625px]">
        <AccountForm defaultValues={defaultValues} onSuccess={onClose} />
      </DialogContent>
    </Dialog>
  );
}

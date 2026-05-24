import { ActionPalette, type ActionPaletteGroup } from "@/components/action-palette";
import {
  useRecalculatePortfolioMutation,
  useUpdatePortfolioMutation,
} from "@/hooks/use-calculate-portfolio";
import { useRunHealthChecks } from "@/hooks/use-health";
import { Button } from "@mizan/ui/components/ui/button";
import { Icons } from "@mizan/ui/components/ui/icons";
import { useMemo, useState } from "react";
import { useNavigate } from "react-router-dom";

interface DashboardActionsProps {
  onAddAsset?: () => void;
  onAddLiability?: () => void;
}

export function DashboardActions({ onAddAsset, onAddLiability }: DashboardActionsProps) {
  const navigate = useNavigate();
  const [open, setOpen] = useState(false);

  const updatePortfolioMutation = useUpdatePortfolioMutation();
  const recalculatePortfolioMutation = useRecalculatePortfolioMutation();
  const runHealthChecksMutation = useRunHealthChecks({ navigate });

  const groups = useMemo((): ActionPaletteGroup[] => {
    const primaryActions =
      onAddAsset && onAddLiability
        ? [
            {
              icon: Icons.Plus,
              label: "Add Asset",
              onClick: onAddAsset,
            },
            {
              icon: Icons.Plus,
              label: "Add Liability",
              onClick: onAddLiability,
            },
          ]
        : [
            {
              icon: Icons.Plus,
              label: "Record Transaction",
              onClick: () => navigate("/activities/manage"),
            },
          ];

    return [
      {
        items: [
          ...primaryActions,
          {
            icon: Icons.Refresh,
            label: "Update Prices",
            onClick: () => updatePortfolioMutation.mutate(),
          },
          {
            icon: Icons.History,
            label: "Rebuild Full History",
            onClick: () => recalculatePortfolioMutation.mutate(),
          },
          {
            icon: Icons.ShieldCheck,
            label: "Verify Data",
            onClick: () => runHealthChecksMutation.mutate(),
          },
        ],
      },
    ];
  }, [
    navigate,
    onAddAsset,
    onAddLiability,
    updatePortfolioMutation,
    recalculatePortfolioMutation,
    runHealthChecksMutation,
  ]);

  return (
    <ActionPalette
      open={open}
      onOpenChange={setOpen}
      groups={groups}
      trigger={
        <Button variant="secondary" size="icon-xs" className="bg-secondary/50 rounded-full">
          <Icons.DotsThreeVertical className="size-5" weight="fill" />
        </Button>
      }
    />
  );
}

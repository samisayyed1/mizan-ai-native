import {
  backupDatabase,
  backupDatabaseToPath,
  getAccounts,
  getActivities,
  getGoals,
  getHistoricalValuations,
  getHoldings,
  getSettings,
  isWeb,
  logger,
  openFileSaveDialog,
  openFolderDialog,
} from "@/adapters";
import { getPlatform as getRuntimePlatform } from "@/hooks/use-platform";
import { formatData } from "@/lib/export-utils";
import { QueryKeys } from "@/lib/query-keys";
import {
  Account,
  AccountValuation,
  ActivityDetails,
  ExportDataType,
  ExportedFileFormat,
  Goal,
  Holding,
} from "@/lib/types";
import { QueryObserverResult, useMutation, useQuery } from "@tanstack/react-query";
import { toast } from "@mizan/ui/components/ui/use-toast";

interface ExportParams {
  format: ExportedFileFormat;
  data: ExportDataType;
}

interface SQLiteBackupResult {
  mode: "sqlite";
  target: "local" | "server";
  value: string;
}

type ExportMutationResult = SQLiteBackupResult | boolean | null;

export function useExportData() {
  const { refetch: fetchAccounts } = useQuery<Account[], Error>({
    queryKey: [QueryKeys.ACCOUNTS],
    queryFn: () => getAccounts(),
    enabled: false,
  });
  const { refetch: fetchActivities } = useQuery<ActivityDetails[], Error>({
    queryKey: [QueryKeys.ACTIVITIES],
    queryFn: () => getActivities(),
    enabled: false,
  });
  const { refetch: fetchGoals } = useQuery<Goal[], Error>({
    queryKey: [QueryKeys.GOALS],
    queryFn: getGoals,
    enabled: false,
  });
  const { refetch: fetchPortfolioHistory } = useQuery<AccountValuation[], Error>({
    queryKey: [QueryKeys.HISTORY_VALUATION],
    queryFn: () => getHistoricalValuations("TOTAL"),
    enabled: false,
  });

  const {
    mutateAsync: exportDataMutation,
    isPending: isExporting,
    variables: mutationVariables,
  } = useMutation<ExportMutationResult, Error, ExportParams>({
    mutationFn: async (params: ExportParams) => {
      const { format, data: desiredData } = params;

      // Excel — produces ONE .xlsx workbook with a tab per data section.
      // The builder is dynamic-imported so users who never visit the
      // export screen never pay the ExcelJS bundle cost.
      if (format === "Excel") {
        const [accountsRes, activitiesRes, goalsRes, historyRes, holdings, settings] =
          await Promise.all([
            fetchAccounts(),
            fetchActivities(),
            fetchGoals(),
            fetchPortfolioHistory(),
            getHoldings("TOTAL").catch((err: unknown) => {
              logger.warn(`getHoldings failed during Excel export: ${String(err)}`);
              return [] as Holding[];
            }),
            getSettings().catch((err: unknown) => {
              logger.warn(`getSettings failed during Excel export: ${String(err)}`);
              return null;
            }),
          ]);

        const { buildPortfolioWorkbook, defaultPortfolioFileName } =
          await import("@/lib/xlsx-portfolio-export");
        const generatedAt = new Date();
        const bytes = await buildPortfolioWorkbook({
          generatedAt,
          baseCurrency: settings?.baseCurrency ?? "USD",
          accounts: accountsRes.data ?? [],
          activities: activitiesRes.data ?? [],
          holdings,
          goals: goalsRes.data ?? [],
          portfolioHistory: historyRes.data ?? [],
        });

        return openFileSaveDialog(bytes, defaultPortfolioFileName(generatedAt));
      }

      if (format === "SQLite") {
        if (isWeb) {
          const { filename } = await backupDatabase();
          return { mode: "sqlite", target: "server" as const, value: filename };
        }

        const runtimePlatform = await getRuntimePlatform();
        if (runtimePlatform.is_desktop) {
          // Open folder dialog to let user choose backup location
          const selectedDir = await openFolderDialog();

          if (!selectedDir) {
            // User cancelled the dialog, return null to indicate cancellation
            return null;
          }

          // Create backup in selected directory
          const backupPath = await backupDatabaseToPath(selectedDir);
          return { mode: "sqlite", target: "local" as const, value: backupPath };
        }

        // Mobile: create backup and let user pick destination file.
        const { filename, data } = await backupDatabase();
        const saved = await openFileSaveDialog(data, filename);
        if (!saved) {
          return null;
        }
        return { mode: "sqlite", target: "local" as const, value: filename };
      } else {
        let exportedData: string | undefined;
        // Initialise so TS can prove definite assignment. Every switch
        // arm below that sets exportedData also sets fileName, so the
        // `if (exportedData)` guard guarantees fileName is the real
        // value at the openFileSaveDialog call site.
        let fileName = "";
        let datasetLabel: string | null = null;

        const currentDate = new Date().toISOString().split("T")[0];
        switch (desiredData) {
          case "accounts":
            exportedData = await fetchAndFormatData(fetchAccounts, format);
            fileName = `accounts_${currentDate}.${format.toLowerCase()}`;
            datasetLabel = "accounts";
            break;
          case "activities":
            exportedData = await fetchAndFormatData(fetchActivities, format);
            fileName = `activities_${currentDate}.${format.toLowerCase()}`;
            datasetLabel = "activities";
            break;
          case "goals":
            exportedData = await fetchAndFormatData(fetchGoals, format);
            fileName = `goals_${currentDate}.${format.toLowerCase()}`;
            datasetLabel = "goals";
            break;
          case "portfolio-history":
            exportedData = await fetchAndFormatData(fetchPortfolioHistory, format);
            fileName = `portfolio-history_${currentDate}.${format.toLowerCase()}`;
            datasetLabel = "portfolio history records";
            break;
        }

        if (exportedData) {
          return openFileSaveDialog(exportedData, fileName);
        }

        if (datasetLabel) {
          toast({
            title: "Nothing to export.",
            description: `No ${datasetLabel} available to export right now.`,
          });
        }

        return null;
      }
    },
    onSuccess: (result) => {
      if (!result) {
        // User cancelled the operation, don't show any message
        return;
      }

      if (result && typeof result === "object" && "mode" in result && result.mode === "sqlite") {
        const description =
          result.target === "server"
            ? `Backup created on the server as ${result.value}`
            : `Backup saved as ${result.value}`;

        toast({
          title: "Database backup completed successfully.",
          description,
          variant: "success",
        });
      } else {
        // Regular export success
        toast({
          title: "Export completed",
          description: "File saved successfully. Check your download location.",
          variant: "success",
        });
      }
    },
    onError: (e) => {
      logger.error(`Error while exporting: ${String(e)}`);
      toast({
        title: "Something went wrong.",
        variant: "destructive",
      });
    },
  });

  const exportData = async (params: ExportParams) => {
    try {
      await exportDataMutation(params);
    } catch (error) {
      logger.error(`Error while exporting: ${String(error)}`);
    }
  };

  return {
    exportData,
    isExporting,
    exportingFormat: isExporting ? mutationVariables?.format : null,
    exportingData: isExporting ? mutationVariables?.data : null,
  };
}

async function fetchAndFormatData(
  queryFn: () => Promise<QueryObserverResult<unknown[], Error>>,
  format: ExportedFileFormat,
): Promise<string | undefined> {
  const response = await queryFn();

  // Handle empty data gracefully - export empty file instead of error
  const data = response.data ?? [];
  return formatData(data, format);
}

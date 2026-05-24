import { z } from "zod";

export const mizanAssetClassSchema = z.enum([
  "cash",
  "stocks_etfs_sukuk",
  "precious_metals",
  "property",
  "crypto",
  "collectibles",
  "liability",
  "other",
]);

export const mizanActionSourceSchema = z.enum([
  "conversational_ingest",
  "csv_import",
  "plaid_sync",
]);

export const addAssetActionSchema = z.object({
  action: z.literal("ADD_ASSET"),
  asset_class: mizanAssetClassSchema,
  source: mizanActionSourceSchema,
  confidence: z.number().min(0).max(1),
  requires_review: z.boolean(),
  payload: z.record(z.unknown()),
});

export const updateAssetActionSchema = z.object({
  action: z.literal("UPDATE_ASSET"),
  asset_id: z.string().min(1),
  confidence: z.number().min(0).max(1),
  requires_review: z.boolean(),
  payload: z.record(z.unknown()),
});

export const importTransactionsActionSchema = z.object({
  action: z.literal("IMPORT_TRANSACTIONS"),
  source: z.literal("csv_import"),
  confidence: z.number().min(0).max(1),
  requires_review: z.boolean(),
  payload: z.record(z.unknown()),
});

export const connectPlaidActionSchema = z.object({
  action: z.literal("CONNECT_PLAID"),
  source: z.literal("plaid_sync"),
  confidence: z.number().min(0).max(1),
  requires_review: z.literal(true),
  payload: z.record(z.unknown()).default({}),
});

export const calculateZakatActionSchema = z.object({
  action: z.literal("CALCULATE_ZAKAT"),
  confidence: z.number().min(0).max(1),
  requires_review: z.boolean().default(false),
  payload: z.record(z.unknown()).default({}),
});

export const generateWealthSummaryActionSchema = z.object({
  action: z.literal("GENERATE_WEALTH_SUMMARY"),
  confidence: z.number().min(0).max(1),
  requires_review: z.boolean().default(false),
  payload: z.record(z.unknown()).default({}),
});

export const mizanAiActionSchema = z.discriminatedUnion("action", [
  addAssetActionSchema,
  updateAssetActionSchema,
  importTransactionsActionSchema,
  connectPlaidActionSchema,
  calculateZakatActionSchema,
  generateWealthSummaryActionSchema,
]);

export type MizanAiAction = z.infer<typeof mizanAiActionSchema>;
export type AddAssetAction = z.infer<typeof addAssetActionSchema>;

export function parseMizanAiAction(input: unknown): MizanAiAction {
  const action = mizanAiActionSchema.parse(input);
  if (action.confidence < 0.75 && !action.requires_review) {
    return { ...action, requires_review: true } as MizanAiAction;
  }
  return action;
}

export function isStateChangingAction(action: MizanAiAction): boolean {
  return ["ADD_ASSET", "UPDATE_ASSET", "IMPORT_TRANSACTIONS", "CONNECT_PLAID"].includes(
    action.action,
  );
}

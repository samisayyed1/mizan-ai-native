// Shared types for the Notify track. Lives next to ../types.ts so
// both the tauri + web adapters can import from one place and the
// shape matches the cloud-side `mizan-core::notifications` model
// exactly (camelCase via `serde(rename_all = "camelCase")`).

/**
 * Wire-format kinds emitted by the Rust insights crate.
 *
 * Source of truth: `crates/core/src/notifications/model.rs` —
 * `pub enum NotificationKind` with `#[serde(rename_all = "snake_case")]`.
 *
 * **Keep this union in sync with the Rust enum.** If a variant is
 * added there and missed here, any consumer that does a `Record<K, V>`
 * lookup (KindIcon, severity styles, etc.) will return `undefined` at
 * runtime and crash the whole tree at the first row that uses it. The
 * defensive fallback in `KindIcon` keeps things visually graceful, but
 * the TypeScript contract is still where this stays honest.
 */
export type NotificationKind =
  | "big_move"
  | "allocation_drift"
  | "goal_milestone"
  | "cash_drag"
  | "dividend_posted"
  | "new_ath"
  | "net_worth_dip"
  | "sync_failure"
  | "ai_digest"
  | "bond_maturity_approaching"
  | "fx_moved_materially"
  | "sharia_status_changed"
  | "zakat_hawl_approaching"
  | "concentration_risk"
  | "cash_drag_opportunity"
  | "tax_optimization_window";

export type NotificationSeverity = "info" | "success" | "warning" | "critical";

export interface Notification {
  id: string;
  kind: NotificationKind;
  severity: NotificationSeverity;
  title: string;
  body: string;
  /** Optional `mizan://...` deep link the row clicks through to. */
  deepLink: string | null;
  /** Stringified JSON the rule emitted. Type-narrow per `kind`. */
  payloadJson: string;
  dedupeKey: string;
  createdAtMs: number;
  readAtMs: number | null;
  dismissedAtMs: number | null;
}

export interface NotificationsPage {
  items: Notification[];
  /** Total unread count (NOT capped by the page `limit`). */
  unreadCount: number;
}

export interface NotificationAdapter {
  list(limit?: number): Promise<NotificationsPage>;
  unreadCount(): Promise<number>;
  markRead(id: string): Promise<void>;
  dismiss(id: string): Promise<void>;
  markAllRead(): Promise<number>;
}

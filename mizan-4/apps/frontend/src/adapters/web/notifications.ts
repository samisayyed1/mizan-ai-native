// Web stub for the notifications adapter.
//
// The web build doesn't have a local SQLite DB to read notifications
// from, and the cloud doesn't currently push notifications down
// (that's a future track). Return empty results so the bell UI
// renders the "All caught up" empty state and no error toasts fire.

import type {
  NotificationAdapter,
  NotificationsPage,
} from "../types-notifications";

export const listNotifications = async (_limit?: number): Promise<NotificationsPage> => ({
  items: [],
  unreadCount: 0,
});

export const notificationsUnreadCount = async (): Promise<number> => 0;
export const markNotificationRead = async (_id: string): Promise<void> => {
  /* web stub — no local DB to write to (see file header) */
};
export const dismissNotification = async (_id: string): Promise<void> => {
  /* web stub — no local DB to write to (see file header) */
};
export const markAllNotificationsRead = async (): Promise<number> => 0;

export type { Notification, NotificationsPage } from "../types-notifications";

export const notificationAdapter: NotificationAdapter = {
  list: listNotifications,
  unreadCount: notificationsUnreadCount,
  markRead: markNotificationRead,
  dismiss: dismissNotification,
  markAllRead: markAllNotificationsRead,
};

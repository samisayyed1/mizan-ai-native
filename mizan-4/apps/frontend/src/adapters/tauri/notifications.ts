// Desktop adapter for the personalized AI wealth-notification center.
// Mirrors the Rust commands in apps/tauri/src/commands/notifications.rs.

import type {
  NotificationAdapter,
  NotificationsPage,
} from "../types-notifications";
import { invoke, logger } from "./core";

export const listNotifications = async (limit?: number): Promise<NotificationsPage> => {
  try {
    return await invoke<NotificationsPage>("list_notifications", { limit });
  } catch (err) {
    logger.error("Error loading notifications");
    throw err;
  }
};

export const notificationsUnreadCount = async (): Promise<number> => {
  try {
    return await invoke<number>("notifications_unread_count");
  } catch (_err) {
    logger.error("Error fetching unread notification count");
    // Bell-badge call site treats this as a soft failure — return 0
    // rather than break the chrome on a transient IPC blip.
    return 0;
  }
};

export const markNotificationRead = async (id: string): Promise<void> => {
  try {
    await invoke("mark_notification_read", { id });
  } catch (err) {
    logger.error("Error marking notification read");
    throw err;
  }
};

export const dismissNotification = async (id: string): Promise<void> => {
  try {
    await invoke("dismiss_notification", { id });
  } catch (err) {
    logger.error("Error dismissing notification");
    throw err;
  }
};

export const markAllNotificationsRead = async (): Promise<number> => {
  try {
    return await invoke<number>("mark_all_notifications_read");
  } catch (err) {
    logger.error("Error marking all notifications read");
    throw err;
  }
};

/** Re-export the domain types so consumers don't have to import twice. */
export type { Notification, NotificationsPage } from "../types-notifications";

export const notificationAdapter: NotificationAdapter = {
  list: listNotifications,
  unreadCount: notificationsUnreadCount,
  markRead: markNotificationRead,
  dismiss: dismissNotification,
  markAllRead: markAllNotificationsRead,
};

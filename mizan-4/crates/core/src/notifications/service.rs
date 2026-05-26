//! Notification persistence trait + in-memory implementation.
//!
//! The real persistence path is `mizan-storage-sqlite::notifications::
//! SqliteNotificationService`. This in-memory impl is used by unit
//! tests of the insights engine + scheduler so we don't need a temp
//! DB for every test.

use std::sync::RwLock;

use async_trait::async_trait;

use super::model::{Notification, NotificationsPage};
use crate::Result;

#[async_trait]
pub trait NotificationService: Send + Sync {
    /// Insert a notification if its `dedupe_key` is unseen.
    /// Returns `Ok(true)` if a new row was created, `Ok(false)` if
    /// the dedupe_key already existed. Idempotent reruns of the
    /// insights engine on the same day are first-class — the caller
    /// fires-and-forgets without checking.
    async fn emit(&self, notification: Notification) -> Result<bool>;

    /// Bulk emit — convenience wrapper. Returns the count of rows
    /// actually written (i.e. excluding dedupe-hits).
    async fn emit_batch(&self, notifications: Vec<Notification>) -> Result<usize> {
        let mut new_count = 0usize;
        for n in notifications {
            if self.emit(n).await? {
                new_count += 1;
            }
        }
        Ok(new_count)
    }

    /// Bell-panel list: not-dismissed rows, newest first. `limit`
    /// caps the page size; the unread count is total (uncapped).
    async fn list_active(&self, limit: usize) -> Result<NotificationsPage>;

    /// Mark one row read (does nothing if id is unknown).
    async fn mark_read(&self, id: &str) -> Result<()>;

    /// Soft-delete one row (does nothing if id is unknown).
    async fn dismiss(&self, id: &str) -> Result<()>;

    /// Mark all currently-unread rows as read in one shot. Used by
    /// the "Mark all read" action in the bell panel.
    async fn mark_all_read(&self) -> Result<usize>;

    /// Unread count — fast path for the bell badge that doesn't
    /// need to hydrate full rows.
    async fn unread_count(&self) -> Result<i64>;
}

pub struct InMemoryNotificationService {
    rows: RwLock<Vec<Notification>>,
}

impl InMemoryNotificationService {
    pub fn new() -> Self {
        Self {
            rows: RwLock::new(Vec::new()),
        }
    }
}

impl Default for InMemoryNotificationService {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl NotificationService for InMemoryNotificationService {
    async fn emit(&self, notification: Notification) -> Result<bool> {
        let mut rows = self.rows.write().expect("notifications poisoned");
        if rows.iter().any(|r| r.dedupe_key == notification.dedupe_key) {
            return Ok(false);
        }
        rows.push(notification);
        Ok(true)
    }

    async fn list_active(&self, limit: usize) -> Result<NotificationsPage> {
        let rows = self.rows.read().expect("notifications poisoned");
        let mut active: Vec<Notification> = rows
            .iter()
            .filter(|r| r.dismissed_at_ms.is_none())
            .cloned()
            .collect();
        active.sort_by(|a, b| b.created_at_ms.cmp(&a.created_at_ms));
        let unread_count = active.iter().filter(|r| r.read_at_ms.is_none()).count() as i64;
        active.truncate(limit);
        Ok(NotificationsPage {
            items: active,
            unread_count,
        })
    }

    async fn mark_read(&self, id: &str) -> Result<()> {
        let mut rows = self.rows.write().expect("notifications poisoned");
        if let Some(row) = rows.iter_mut().find(|r| r.id == id) {
            if row.read_at_ms.is_none() {
                row.read_at_ms = Some(chrono::Utc::now().timestamp_millis());
            }
        }
        Ok(())
    }

    async fn dismiss(&self, id: &str) -> Result<()> {
        let mut rows = self.rows.write().expect("notifications poisoned");
        if let Some(row) = rows.iter_mut().find(|r| r.id == id) {
            if row.dismissed_at_ms.is_none() {
                row.dismissed_at_ms = Some(chrono::Utc::now().timestamp_millis());
            }
        }
        Ok(())
    }

    async fn mark_all_read(&self) -> Result<usize> {
        let mut rows = self.rows.write().expect("notifications poisoned");
        let now = chrono::Utc::now().timestamp_millis();
        let mut n = 0;
        for row in rows.iter_mut() {
            if row.read_at_ms.is_none() && row.dismissed_at_ms.is_none() {
                row.read_at_ms = Some(now);
                n += 1;
            }
        }
        Ok(n)
    }

    async fn unread_count(&self) -> Result<i64> {
        let rows = self.rows.read().expect("notifications poisoned");
        Ok(rows
            .iter()
            .filter(|r| r.read_at_ms.is_none() && r.dismissed_at_ms.is_none())
            .count() as i64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notifications::model::{NotificationKind, NotificationSeverity};

    fn fixture(dedupe: &str) -> Notification {
        Notification {
            id: uuid::Uuid::new_v4().to_string(),
            kind: NotificationKind::BigMove,
            severity: NotificationSeverity::Info,
            title: "t".into(),
            body: "b".into(),
            deep_link: None,
            payload_json: "{}".into(),
            dedupe_key: dedupe.into(),
            created_at_ms: chrono::Utc::now().timestamp_millis(),
            read_at_ms: None,
            dismissed_at_ms: None,
        }
    }

    #[tokio::test]
    async fn dedupe_key_blocks_second_insert() {
        let svc = InMemoryNotificationService::new();
        assert!(svc.emit(fixture("k1")).await.unwrap());
        assert!(!svc.emit(fixture("k1")).await.unwrap());
        assert_eq!(svc.unread_count().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn dismiss_hides_from_list_but_unread_excludes_too() {
        let svc = InMemoryNotificationService::new();
        svc.emit(fixture("k1")).await.unwrap();
        let page = svc.list_active(10).await.unwrap();
        let id = page.items[0].id.clone();
        svc.dismiss(&id).await.unwrap();
        let page2 = svc.list_active(10).await.unwrap();
        assert!(page2.items.is_empty());
        assert_eq!(svc.unread_count().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn mark_all_read_only_counts_currently_unread() {
        let svc = InMemoryNotificationService::new();
        svc.emit(fixture("k1")).await.unwrap();
        svc.emit(fixture("k2")).await.unwrap();
        let n = svc.mark_all_read().await.unwrap();
        assert_eq!(n, 2);
        let n2 = svc.mark_all_read().await.unwrap();
        assert_eq!(n2, 0, "second run is a no-op");
    }
}

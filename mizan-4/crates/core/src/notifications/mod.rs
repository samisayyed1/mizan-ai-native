//! Personalized AI wealth-notification engine (Notify track).
//!
//! Pipeline:
//!   1. The insights engine (deterministic rules, no LLM) scans the
//!      user's current snapshot + recent history and emits a typed
//!      `Insight` set.
//!   2. `Insight::into_notification()` lowers each one to a typed
//!      `Notification` (title + body + severity + dedupe_key +
//!      payload_json + optional deep_link).
//!   3. `NotificationService::emit_batch` writes them through the
//!      idempotency UNIQUE on `dedupe_key`, so reruns are safe.
//!   4. The Tauri scheduler picks freshly-emitted rows, fires OS
//!      notifications for `severity >= Warning`, and emits a
//!      `notifications:new` event to the frontend so the bell badge
//!      refreshes.
//!
//! The "AI-native" layer sits *on top* of this:
//!   - If managed AI is enabled (Silver/Gold), the scheduler asks
//!     Mizan Connect for a 2-sentence personalised digest summarising
//!     the day's insights and emits a single `AiDigest` notification
//!     alongside. This is the only LLM call per user per day —
//!     everything else is deterministic.

mod model;
mod service;

pub use model::{Notification, NotificationKind, NotificationSeverity, NotificationsPage};
pub use service::{InMemoryNotificationService, NotificationService};

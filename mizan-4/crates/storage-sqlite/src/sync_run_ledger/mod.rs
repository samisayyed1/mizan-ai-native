//! SQLite-backed implementation of §A4 `SyncRunLedger`.

pub mod model;
pub mod repository;

pub use repository::SqliteSyncRunLedger;

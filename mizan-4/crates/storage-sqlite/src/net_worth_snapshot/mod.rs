//! SQLite-backed implementation of §A12 `NetWorthSnapshotService`.

pub mod repository;

pub use repository::SqliteNetWorthSnapshotService;

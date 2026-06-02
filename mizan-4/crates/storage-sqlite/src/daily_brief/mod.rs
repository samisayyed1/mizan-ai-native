//! SQLite-backed implementation of §A22 `DailyBriefService`.

pub mod repository;

pub use repository::SqliteDailyBriefService;

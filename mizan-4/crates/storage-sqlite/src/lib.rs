// AI chat repository surfaces `ChatRepositoryResult<T> = Result<T, AiError>`,
// which inherits AiError's large-by-design footprint. See the same allow
// in crates/ai/src/lib.rs for the rationale (flat sum type for
// matchability over boxing). Suppressed crate-wide to keep storage
// signatures aligned with the AI crate's contract.
#![allow(clippy::result_large_err)]

//! SQLite storage implementation for Mizan.
//!
//! This crate provides all database-related functionality using Diesel ORM with SQLite.
//! It implements the repository traits defined in `mizan-core` and contains:
//! - Database connection pooling and management
//! - Diesel migrations
//! - Repository implementations for all domain entities
//! - Database-specific model types (with Diesel derives)
//!
//! # Architecture
//!
//! This crate is the only place in the application where Diesel dependencies exist.
//! All other crates (`core`, `connect`) are database-agnostic and work with traits.
//!
//! ```text
//! core (domain)          connect (sync)
//!       │                      │
//!       └──────────┬───────────┘
//!                  │
//!                  ▼
//!          storage-sqlite (this crate)
//!                  │
//!                  ▼
//!              SQLite DB
//! ```

pub mod db;
pub mod errors;
pub mod schema;
pub mod utils;

// Repository implementations
pub mod accounts;
pub mod activities;
pub mod ai_chat;
pub mod assets;
pub mod custom_provider;
pub mod fx;
pub mod goals;
pub mod health;
pub mod limits;
pub mod market_data;
pub mod news;
pub mod portfolio;
pub mod daily_brief;
pub mod net_worth_snapshot;
pub mod settings;
pub mod sync;
pub mod sync_run_ledger;
pub mod taxonomies;
pub mod truth_ledger;

// Re-export database utilities
pub use db::{
    backup_database, create_pool, get_connection, get_db_path, init, restore_database,
    restore_database_safe, run_migrations, DbConnection, DbPool, DbTransactionExecutor,
    WriteHandle,
};

// Re-export storage errors and conversion helpers
pub use errors::{IntoCore, StorageError};

// Re-export from mizan-core for convenience
pub use mizan_core::errors::{DatabaseError, Error, Result};

// Re-export SQLite utilities
pub use utils::{chunk_for_sqlite, SQLITE_MAX_PARAMS_CHUNK};

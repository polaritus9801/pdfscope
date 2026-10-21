//! SQLite persistence.
//!
//! The database is an index over the filesystem, never the source of truth. That is why this crate
//! can be deleted, rebuilt, and tested against an in-memory database without ceremony.
//!
//! Two rules hold here:
//!
//! - **Migrations are append-only.** A shipped migration is never edited; a change is a new file.
//! - **Queries are written at runtime, not as compile-time macros.** `sqlx::query` keeps
//!   `cargo check` working on a machine with no database and no `DATABASE_URL`, which matters more
//!   for a desktop app than the extra type safety of `query!`.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

use std::path::Path;
use std::str::FromStr;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::{SqlitePool, migrate::MigrateError};
use thiserror::Error;

/// Errors produced by the persistence layer.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum StoreError {
    /// The database rejected a statement or could not be reached.
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    /// A migration was missing or could not be applied.
    #[error("migration failed: {0}")]
    Migration(#[from] MigrateError),
}

/// A handle to the library database.
#[derive(Debug, Clone)]
pub struct Database {
    pool: SqlitePool,
}

impl Database {
    /// Opens (creating if absent) the database at `path` and applies pending migrations.
    pub async fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(SqliteJournalMode::Wal);

        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(options)
            .await?;

        let database = Self { pool };
        database.migrate().await?;
        Ok(database)
    }

    /// Opens a throwaway database in memory, for tests and for a dry run.
    ///
    /// A single connection is used because an in-memory database exists only for the connection
    /// that created it; a pool with more connections would hand out empty databases.
    pub async fn in_memory() -> Result<Self, StoreError> {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")?.foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;

        let database = Self { pool };
        database.migrate().await?;
        Ok(database)
    }

    /// Applies every migration that has not run yet.
    pub async fn migrate(&self) -> Result<(), StoreError> {
        sqlx::migrate!("./migrations").run(&self.pool).await?;
        Ok(())
    }

    /// The underlying pool, for callers in this workspace that need a transaction.
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Number of books in the library.
    pub async fn book_count(&self) -> Result<i64, StoreError> {
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM books")
            .fetch_one(&self.pool)
            .await?;
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn in_memory() -> Database {
        Database::in_memory().await.unwrap()
    }

    #[tokio::test]
    async fn migrates_an_empty_database() {
        let database = in_memory().await;
        assert_eq!(database.book_count().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn migrations_are_idempotent() {
        let database = in_memory().await;
        database.migrate().await.unwrap();
        database.migrate().await.unwrap();
    }

    #[tokio::test]
    async fn stores_and_counts_a_book() {
        let database = in_memory().await;

        sqlx::query(
            "INSERT INTO books (id, path, title, page_count, added_at) VALUES (?, ?, ?, ?, ?)",
        )
        .bind("9f3c")
        .bind("/tmp/book.pdf")
        .bind("A Book")
        .bind(404_i64)
        .bind(0_i64)
        .execute(database.pool())
        .await
        .unwrap();

        assert_eq!(database.book_count().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn rejects_a_negative_page_count() {
        let database = in_memory().await;

        let result = sqlx::query(
            "INSERT INTO books (id, path, title, page_count, added_at) VALUES (?, ?, ?, ?, ?)",
        )
        .bind("bad")
        .bind("/tmp/book.pdf")
        .bind("A Book")
        .bind(-1_i64)
        .bind(0_i64)
        .execute(database.pool())
        .await;

        assert!(
            result.is_err(),
            "schema should reject a negative page count"
        );
    }

    #[tokio::test]
    async fn reopens_a_file_database() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("library.db");

        {
            let database = Database::open(&path).await.unwrap();
            assert_eq!(database.book_count().await.unwrap(), 0);
        }

        let reopened = Database::open(&path).await.unwrap();
        assert_eq!(reopened.book_count().await.unwrap(), 0);
    }
}

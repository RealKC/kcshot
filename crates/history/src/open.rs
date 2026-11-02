use diesel::{SqliteConnection, prelude::*};
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};

const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

#[derive(thiserror::Error, Debug)]
pub enum OpenHistoryError {
    #[error("Failed to get xdg directory for state: {0}")]
    Xdg(#[from] xdg::BaseDirectoriesError),
    #[error("Encountered an IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Failed to open a connection: {0}")]
    Connection(#[from] ConnectionError),
    #[error("Failed to run migrations: {0}")]
    Migration(#[from] Box<dyn std::error::Error + Send + Sync>),
    #[error("Paths are not UTF-8")]
    PathsAreNotUtf8,
}

pub fn open() -> Result<SqliteConnection, OpenHistoryError> {
    let path = if cfg!(feature = "xdg-paths") {
        xdg::BaseDirectories::with_prefix("kcshot").place_state_file("history.db")?
    } else {
        std::env::current_dir()?.join("history.db")
    };
    let path = path.to_str().ok_or(OpenHistoryError::PathsAreNotUtf8)?;

    let mut connection = SqliteConnection::establish(path)?;
    connection.run_pending_migrations(MIGRATIONS)?;

    Ok(connection)
}

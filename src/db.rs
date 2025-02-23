use diesel::{SqliteConnection, prelude::*};
use diesel_migrations::{EmbeddedMigrations, MigrationHarness};

use self::models::Screenshot;

pub mod models;
pub mod schema;

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
        xdg::BaseDirectories::with_prefix("kcshot")?.place_state_file("history.db")?
    } else {
        std::env::current_dir()?.join("history.db")
    };
    let path = path.to_str().ok_or(OpenHistoryError::PathsAreNotUtf8)?;

    let mut connection = SqliteConnection::establish(path)?;
    connection.run_pending_migrations(MIGRATIONS)?;

    Ok(connection)
}

pub fn add_screenshot_to_history(
    conn: &mut SqliteConnection,
    path_: Option<String>,
    time_: String,
    url_: Option<String>,
) -> QueryResult<()> {
    use schema::screenshots::{self, dsl::*};

    diesel::insert_into(screenshots::table)
        .values((path.eq(path_), time.eq(time_), url.eq(url_)))
        .execute(conn)
        .map(|_| ())
}

pub fn fetch_screenshots(
    conn: &mut SqliteConnection,
    start_at: i64,
    count: i64,
) -> QueryResult<Vec<Screenshot>> {
    use schema::screenshots::dsl::*;

    screenshots
        .limit(count)
        .offset(start_at)
        .order(id.desc())
        .load::<Screenshot>(conn)
}

pub fn number_of_history_itms(conn: &mut SqliteConnection) -> QueryResult<i64> {
    use schema::screenshots::dsl::*;

    screenshots.count().get_result(conn)
}

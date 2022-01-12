use diesel::{migration::RunMigrationsError, prelude::*, SqliteConnection};

use self::models::Screenshot;

pub mod models;
pub mod schema;

diesel_migrations::embed_migrations!();

#[derive(thiserror::Error, Debug)]
pub enum OpenHistoryError {
    #[error("Failed to get xdg directory for state: {0}")]
    Xdg(#[from] xdg::BaseDirectoriesError),
    #[error("Encountered an IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Failed to open a connection: {0}")]
    Connection(#[from] ConnectionError),
    #[error("Failed to run migrations: {0}")]
    Migration(#[from] RunMigrationsError),
    #[error("Paths are not UTF-8")]
    PathsAreNotUtf8,
}

pub fn open() -> Result<SqliteConnection, OpenHistoryError> {
    #[cfg(feature = "xdg")]
    let path = xdg::BaseDirectories::with_prefix("kcshot")?.place_state_file("history.db")?;
    #[cfg(not(feature = "xdg"))]
    let path = std::env::current_dir()?.join("history.db");
    let path = path.to_str().ok_or(OpenHistoryError::PathsAreNotUtf8)?;

    let connection = SqliteConnection::establish(path)?;
    embedded_migrations::run_with_output(&connection, &mut std::io::stderr())?;

    Ok(connection)
}

pub fn add_screenshot_to_history(
    conn: &SqliteConnection,
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
    conn: &SqliteConnection,
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

pub fn number_of_history_itms(conn: &SqliteConnection) -> QueryResult<i64> {
    use schema::screenshots::dsl::*;

    screenshots.count().get_result(conn)
}

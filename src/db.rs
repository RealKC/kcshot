use diesel::{prelude::*, result::ConnectionResult, SqliteConnection};

use self::models::Screenshot;

pub mod models;
pub mod schema;

pub fn open(path: &str) -> ConnectionResult<SqliteConnection> {
    SqliteConnection::establish(path)
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

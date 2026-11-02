use diesel::{QueryResult, SqliteConnection, prelude::*};

use crate::{schema, screenshot::Screenshot};

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

pub fn get_screenshot_at_index(conn: &mut SqliteConnection, index: u32) -> QueryResult<Screenshot> {
    use schema::screenshots_ordered::dsl::*;

    screenshots_ordered
        .filter(idx.eq(index as i32))
        .limit(1)
        .first::<Screenshot>(conn)
}

pub fn number_of_history_itms(conn: &mut SqliteConnection) -> QueryResult<i64> {
    use schema::screenshots::dsl::*;

    screenshots.count().get_result(conn)
}

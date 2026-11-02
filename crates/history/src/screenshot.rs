#[derive(diesel::Queryable, Clone)]
#[diesel(table_name = screenshots_ordered)]
pub struct Screenshot {
    pub idx: Option<i32>,
    pub id: Option<i32>,
    pub path: Option<String>,
    pub time: Option<String>,
    pub url: Option<String>,
}

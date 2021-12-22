#[derive(Queryable, Clone)]
pub struct Screenshot {
    pub id: i32,
    pub path: Option<String>,
    pub time: String,
    pub url: Option<String>,
}

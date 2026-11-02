// @generated automatically by Diesel CLI.

diesel::table! {
    screenshots (id) {
        id -> Integer,
        path -> Nullable<Text>,
        time -> Text,
        url -> Nullable<Text>,
    }
}

diesel::view! {
    screenshots_ordered {
        idx -> Nullable<Integer>,
        id -> Nullable<Integer>,
        path -> Nullable<Text>,
        time -> Nullable<Text>,
        url -> Nullable<Text>,
    }
}

diesel::allow_tables_to_appear_in_same_query!(screenshots, screenshots_ordered,);

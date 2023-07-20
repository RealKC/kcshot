#![allow(unused_qualifications /*, reason = "Macro generated code" */)]

table! {
    screenshots (id) {
        id -> Integer,
        path -> Nullable<Text>,
        time -> Text,
        url -> Nullable<Text>,
    }
}

use gtk4::{gio, glib};

use crate::colour::Colour;

#[gsettings_macro::gen_settings(file = "./resources/kc.kcshot.gschema.xml", id = "kc.kcshot")]
#[gen_settings_define(
    key_name = "last-used-primary-colour",
    arg_type = "Colour",
    ret_type = "Colour"
)]
#[gen_settings_define(
    key_name = "last-used-secondary-colour",
    arg_type = "Colour",
    ret_type = "Colour"
)]
pub struct Settings;

impl Settings {
    pub fn open() -> Self {
        Self::default()
    }
}

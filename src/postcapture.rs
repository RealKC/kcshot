use std::{collections::HashMap, fmt::Write as _};

use diesel::SqliteConnection;
use gtk4::{
    gdk::{self, prelude::*},
    gdk_pixbuf::Pixbuf,
    glib,
};
use kcshot_data::settings::Settings;

use crate::{
    db,
    history::{ModelNotifier, RowData},
};

/// Trait for the post capture actions.
#[allow(
    dead_code,
    reason = "I need to figure out the post capture actions situation still"
)]
pub trait PostCaptureAction {
    /// Returns the ID of the action, this is used for the settings.
    fn id(&self) -> String;

    /// The name of the post capture action.
    fn name(&self) -> String;

    /// Short description of the post capture action.
    fn description(&self) -> String;

    /// Gets called when executing the post capture action.
    fn handle(
        &self,
        model_notifier: &ModelNotifier,
        conn: &mut SqliteConnection,
        pixbuf: &mut Pixbuf,
    );
}

/// This struct represents the action of saving the pixbuf to disk.
pub struct SaveToDisk;

impl PostCaptureAction for SaveToDisk {
    fn id(&self) -> String {
        "save-to-disk".to_owned()
    }

    fn name(&self) -> String {
        "Save to disk".to_owned()
    }

    fn description(&self) -> String {
        "Saves the screenshot to the disk".to_owned()
    }

    fn handle(
        &self,
        model_notifier: &ModelNotifier,
        conn: &mut SqliteConnection,
        pixbuf: &mut Pixbuf,
    ) {
        let now = glib::DateTime::now_local()
            .unwrap()
            .format_iso8601()
            .unwrap()
            .to_string();

        let settings = Settings::open();
        let mut path = settings.saved_screenshots_path();
        if !path.ends_with('/') {
            path.push('/');
        }

        if let Err(why) = std::fs::create_dir_all(&path) {
            tracing::error!("Failed to create directory='{path}': {why}");
        }

        write!(path, "screenshot_{now}.png").expect("Writing to a string shouldn't fail");

        if let Err(why) = pixbuf.savev(&path, "png", &[]) {
            tracing::error!("Failed to save screenshot to file: {why}");
        }

        if let Err(why) = db::add_screenshot_to_history(conn, Some(path.clone()), now.clone(), None)
        {
            tracing::error!("Failed to add screenshot to history: {why}");
            return;
        }
        if let Err(why) =
            model_notifier.blocking_send(RowData::new_from_components(Some(path), now, None))
        {
            tracing::error!("Failed to notify the history model that a new item was added: {why}");
        }
    }
}

/// This struct represents the action of copying the picture to the users clipboard.
pub struct CopyToClipboard;

impl PostCaptureAction for CopyToClipboard {
    fn id(&self) -> String {
        "copy-to-clipboard".to_owned()
    }

    fn name(&self) -> String {
        "Copy to clipboard".to_owned()
    }

    fn description(&self) -> String {
        "Copies the picture to the clipboard".to_owned()
    }

    fn handle(
        &self,
        _model_notifier: &ModelNotifier,
        _conn: &mut SqliteConnection,
        pixbuf: &mut Pixbuf,
    ) {
        let Some(display) = gdk::Display::default() else {
            tracing::error!("Failed to fetch gdk::Display, bailing...");
            return;
        };
        let clipboard = display.clipboard();

        clipboard.set_texture(&gdk::Texture::for_pixbuf(pixbuf));
    }
}

/// Executes the post capture actions in the order they are defined in the settings.
pub fn run_postcapture_actions(
    model_notifier: &ModelNotifier,
    conn: &mut SqliteConnection,
    pixbuf: &mut Pixbuf,
) {
    for action in get_actions_from_settings() {
        action.handle(model_notifier, conn, pixbuf);
    }
}

fn get_actions_from_settings() -> Vec<&'static dyn PostCaptureAction> {
    let action_names = Settings::open().post_capture_actions();

    let action_ids_to_objects: HashMap<String, &dyn PostCaptureAction> = get_postcapture_actions()
        .iter()
        .map(|action| (action.id(), *action))
        .collect();

    let mut actions_to_run = Vec::new();
    for postcapture_action in action_names {
        if let Some(action) = action_ids_to_objects.get(postcapture_action.as_str()) {
            actions_to_run.push(*action);
        } else {
            tracing::warn!(
                "Found post capture action `{postcapture_action}` in the settings, but not in list of available post capture actions!"
            );
        }
    }

    actions_to_run
}

/// Vector of all available post capture actions.
fn get_postcapture_actions() -> Vec<&'static dyn PostCaptureAction> {
    vec![&SaveToDisk, &CopyToClipboard]
}

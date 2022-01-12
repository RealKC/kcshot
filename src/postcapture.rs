use diesel::SqliteConnection;
use gtk4::{
    gdk::{self, prelude::*},
    gdk_pixbuf::Pixbuf,
    gio,
};

use crate::{
    db,
    historymodel::{ModelNotifier, RowData},
};

pub fn do_postcapture_actions(history_model: &HistoryModel, conn: &SqliteConnection, pixbuf: &mut Pixbuf) {
    for action in get_postcapture_actions() {
        action.handle(history_model, conn, pixbuf)
    }
}

/// Trait for the post capture actions.
pub trait PostCaptureAction {
    /// Returns the ID of the action, this is used for the settings.
    fn id(&self) -> String;

    /// The name of the post capture action.
    fn name(&self) -> String;

    /// Short description of the post capture action.
    fn description(&self) -> String;

    /// Gets called when executing the post capture action.
    fn handle(&self, model_notifier: &ModelNotifier, conn: &SqliteConnection, pixbuf: &mut Pixbuf);
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

    fn handle(&self, model_notifier: &ModelNotifier, conn: &SqliteConnection, pixbuf: &mut Pixbuf) {
        let now = chrono::Local::now();
        let now = now.to_rfc3339();

        let settings = gio::Settings::new("kc.kcshot");
        let path = settings.string("saved-screenshots-path");
        let path = if path.ends_with('/') {
            format!("{}screenshot_{}.png", path, now)
        } else {
            format!("{}/screenshot_{}.png", path, now)
        };

        let res = pixbuf.savev(&path, "png", &[]);

        match res {
            Ok(_) => {}
            Err(why) => tracing::error!("Failed to save screenshot to file: {why}"),
        }

        if let Err(why) = db::add_screenshot_to_history(conn, Some(path.clone()), now.clone(), None)
        {
            tracing::error!("Failed to add screenshot to history: {why}");
            return;
        }
        if let Err(why) = model_notifier.send(RowData::new_from_components(Some(path), now, None)) {
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

    fn handle(&self, _model_notifier: &ModelNotifier, _conn: &SqliteConnection, pixbuf: &mut Pixbuf) {
        let display = match gdk::Display::default() {
            Some(display) => display,
            None => {
                tracing::error!("Failed to fetch gdk::Display, bailing...");
                return;
            }
        };
        let clipboard = display.clipboard();

        clipboard.set_texture(&gdk::Texture::for_pixbuf(&pixbuf));
    }
}

/// Vector of all available post capture actions.
pub fn get_postcapture_actions() -> Vec<&'static dyn PostCaptureAction> {
    vec![&SaveToDisk, &CopyToClipboard]
}

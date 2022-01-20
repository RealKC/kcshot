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

pub trait PostCaptureAction {
    fn handle(&self, model_notifier: ModelNotifier, conn: &SqliteConnection, pixbuf: Pixbuf);
}

pub fn current_action() -> &'static dyn PostCaptureAction {
    // FIXME: Eventually this should do more than just this, but we'll get there
    &SaveAndCopy
}

/// This struct represents an action that saves the screenshot to disk and then copies it into
/// the user's clipboard
struct SaveAndCopy;

impl PostCaptureAction for SaveAndCopy {
    fn handle(&self, model_notifier: ModelNotifier, conn: &SqliteConnection, pixbuf: Pixbuf) {
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

        let display = match gdk::Display::default() {
            Some(display) => display,
            None => {
                tracing::error!("Failed to fetch gdk::Display, bailing...");
                return;
            }
        };
        let clipboard = display.clipboard();

        clipboard.set_texture(&gdk::Texture::for_pixbuf(&pixbuf));

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

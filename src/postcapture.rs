use gtk4::{
    gdk::{self, prelude::*},
    gdk_pixbuf::Pixbuf,
};

pub trait PostCaptureAction {
    fn handle(&self, pixbuf: Pixbuf);
}

pub fn current_action() -> &'static dyn PostCaptureAction {
    // FIXME: Eventually this should do more than just this, but we'll get there
    &SaveAndCopy
}

/// This struct represents an action that saves the screenshot to disk and then copies it into
/// the user's clipboard
struct SaveAndCopy;

impl PostCaptureAction for SaveAndCopy {
    fn handle(&self, pixbuf: Pixbuf) {
        let now = chrono::Local::now();
        let res = pixbuf.savev(format!("screenshot_{}.png", now.to_rfc3339()), "png", &[]);

        match res {
            Ok(_) => {}
            Err(why) => tracing::error!("Failed to save screenshot to file: {}", why),
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
    }
}
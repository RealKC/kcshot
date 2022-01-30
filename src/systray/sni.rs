use std::{process::Command, thread::Builder as ThreadBuilder};

use gtk4::{
    glib::{self, Continue, MainContext, Sender},
    prelude::*,
};
use image::ImageResult;

use super::Initialised;
use crate::{editor, kcshot::KCShot};

/// Attempts to create a systray icon using the [KDE/freedesktop StatusNotifierItem spec][`kde_sni`].
/// This is done by using the [ksni][`ksni`] crate.
///
/// This will fail gracefully if we could not initialise a systray icon for any reason
///
/// [`kde_sni`]: https://www.freedesktop.org/wiki/Specifications/StatusNotifierItem/StatusNotifierItem/
/// [`ksni`]: https://crates.io/crates/ksni
pub(super) fn try_init(app: &KCShot) -> Initialised {
    let icon = match load_icon() {
        Ok(icon) => icon,
        Err(why) => {
            tracing::warn!("Failed to load image for systray icon: {why}\n\tThis is not fatal, but the systray icon will not be initialised");
            return Initialised::No;
        }
    };

    // We use channels because AFAIK gtk function calls can only be done on the main thread, and we
    // spawn a new thread to handle SNI events
    let (tx, rx) = MainContext::channel::<Message>(glib::PRIORITY_DEFAULT);

    let tray_service = ksni::TrayService::new(Tray { tx, icon });

    // We make a new thread ourselves so we can give it a more descriptive name :^)
    let res = ThreadBuilder::new()
        .name("tray icon thread".into())
        .spawn(move || {
            if let Err(why) = tray_service.run() {
                tracing::warn!("Failed to run SNI tray service. This is not fatal: {why}");
            }
        });

    if let Err(why) = res {
        tracing::warn!("Failed to spawn SNI thread. Suspicious, yet not fatal: {why}");
        return Initialised::No;
    }

    rx.attach(
        None,
        glib::clone!(@strong app => @default-return Continue(false), move |msg| {
            match msg {
                Message::OpenMainWindow => app.main_window().present(),
                Message::OpenScreenshotFolder => {
                    let res = Command::new("xdg-open")
                        .arg(&KCShot::screenshot_folder())
                        .spawn();
                    if let Err(why) = res {
                        tracing::error!("Failed to spawn xdg-open: {why}");
                    }
                }
                Message::TakeScreenshot => {
                    let window = editor::EditorWindow::new(app.upcast_ref());
                    window.set_decorated(false);
                    window.fullscreen();

                    window.show();
                },
                Message::Quit => {
                    app.quit();
                },
            }
            Continue(true)
        }),
    );

    Initialised::Yes
}

#[derive(Debug)]
enum Message {
    OpenMainWindow,
    OpenScreenshotFolder,
    TakeScreenshot,
    Quit,
}

fn load_icon() -> ImageResult<ksni::Icon> {
    const ICON_BYTES: &[u8] = include_bytes!("../../resources/logo/tray.png");
    let image = image::load_from_memory(ICON_BYTES)?.to_bgra8();

    let (width, height) = image.dimensions();
    let mut raw_image_data = image.into_raw();

    // We convert the image to ARGB as that's the required format of the image as defined in the SNI spec
    for chunk in raw_image_data.chunks_mut(4) {
        chunk.reverse();
    }

    Ok(ksni::Icon {
        width: width as i32,
        height: height as i32,
        data: raw_image_data,
    })
}

#[derive(Debug)]
struct Tray {
    tx: Sender<Message>,
    icon: ksni::Icon,
}

impl ksni::Tray for Tray {
    fn activate(&mut self, _x: i32, _y: i32) {
        if let Err(why) = self.tx.send(Message::TakeScreenshot) {
            tracing::error!("Failed to send message: {why:?}");
        }
    }

    fn id(&self) -> String {
        "kc.kcshot".into()
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        vec![self.icon.clone()]
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::*;
        vec![
            StandardItem {
                label: "Open window".into(),
                activate: Box::new(|tray: &mut Self| {
                    if let Err(why) = tray.tx.send(Message::OpenMainWindow) {
                        tracing::error!("Failed to send message: {why:?}");
                    }
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Open screenshot folder".into(),
                activate: Box::new(|tray: &mut Self| {
                    if let Err(why) = tray.tx.send(Message::OpenScreenshotFolder) {
                        tracing::error!("Failed to send message: {why:?}");
                    }
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Quit".into(),
                icon_name: "application-exit".into(),
                activate: Box::new(|tray: &mut Self| {
                    if let Err(why) = tray.tx.send(Message::Quit) {
                        tracing::error!("Failed to send message: {why:?}");
                    }
                }),
                ..Default::default()
            }
            .into(),
        ]
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip {
            icon_name: "".into(),
            icon_pixmap: vec![],
            title: "kcshot".into(),
            description: "".into(),
        }
    }
}

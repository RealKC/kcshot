use std::{process::Command, thread::Builder as ThreadBuilder};

use gtk4::{
    gdk_pixbuf::Pixbuf,
    glib::{self},
    prelude::*,
};
use kcshot_data::settings::Settings;
use ksni::TrayMethods;
use tokio::{
    runtime::Builder as RtBuilder,
    sync::mpsc::{self, Sender},
};

use super::Initialised;
use crate::{editor::EditorWindow, kcshot::KCShot};

/// Attempts to create a systray icon using the [KDE/freedesktop StatusNotifierItem spec][`kde_sni`].
/// This is done by using the [ksni][`ksni`] crate.
///
/// This will fail gracefully if we could not initialise a systray icon for any reason
///
/// [`kde_sni`]: https://www.freedesktop.org/wiki/Specifications/StatusNotifierItem/StatusNotifierItem/
/// [`ksni`]: https://crates.io/crates/ksni
pub(super) fn try_init(app: KCShot) -> Initialised {
    let icon = match load_icon() {
        Ok(icon) => icon,
        Err(why) => {
            tracing::warn!("Failed to load image for systray icon: {why}\n\tThis is not fatal, but the systray icon will not be initialised");
            return Initialised::No;
        }
    };

    // We can't invoke GTK methods from threads other than the main one, so we use channels
    // and an async task running on the main thread to invoke GTK stuff from the SNI thread
    let (tx, mut rx) = mpsc::channel(16);

    let tray_service = Tray { tx, icon };

    // We make a new thread ourselves so we can give it a more descriptive name :^)
    let res = ThreadBuilder::new()
        .name("tray icon thread".into())
        .spawn(move || {
            let rt = RtBuilder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();

            rt.block_on(async {
                if let Err(why) = tray_service.spawn().await {
                    tracing::warn!("Failed to run SNI tray service. This is not fatal: {why}");
                }

                std::future::pending::<()>().await;
            });
        });

    if let Err(why) = res {
        tracing::warn!("Failed to spawn SNI thread. Suspicious, yet not fatal: {why}");
        return Initialised::No;
    }

    glib::MainContext::default().spawn_local(async move {
        while let Some(msg) = rx.recv().await {
            match msg {
                Message::OpenMainWindow => app.main_window().present(),
                Message::OpenScreenshotFolder => {
                    let res = Command::new("xdg-open")
                        .arg(KCShot::screenshot_folder())
                        .spawn();
                    if let Err(why) = res {
                        tracing::error!("Failed to spawn xdg-open: {why}");
                    }
                }
                Message::TakeScreenshot => {
                    let editing_starts_with_cropping =
                        Settings::open().editing_starts_with_cropping();

                    EditorWindow::show(app.upcast_ref(), editing_starts_with_cropping);
                }
                Message::Quit => app.quit(),
            }
        }
    });

    Initialised::Yes
}

#[derive(Debug)]
enum Message {
    OpenMainWindow,
    OpenScreenshotFolder,
    TakeScreenshot,
    Quit,
}

fn load_icon() -> Result<ksni::Icon, glib::Error> {
    const ICON_BYTES: &[u8] = include_bytes!("../../resources/logo/tray.png");

    let image = Pixbuf::from_read(ICON_BYTES)?;

    let width = image.width();
    let height = image.height();
    let mut raw_image_data = image.pixel_bytes().unwrap().to_vec();

    // We convert the image to ARGB as that's the required format of the image as defined in the SNI spec
    for chunk in raw_image_data.chunks_mut(4) {
        // RGBA rotated right once gives us ARGB
        chunk.rotate_right(1);
    }

    Ok(ksni::Icon {
        width,
        height,
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
        if let Err(why) = self.tx.try_send(Message::TakeScreenshot) {
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
                    if let Err(why) = tray.tx.try_send(Message::OpenMainWindow) {
                        tracing::error!("Failed to send message: {why:?}");
                    }
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Open screenshot folder".into(),
                activate: Box::new(|tray: &mut Self| {
                    if let Err(why) = tray.tx.try_send(Message::OpenScreenshotFolder) {
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
                    if let Err(why) = tray.tx.try_send(Message::Quit) {
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
            icon_name: String::new(),
            icon_pixmap: vec![],
            title: "kcshot".into(),
            description: String::new(),
        }
    }

    fn watcher_offline(&self, reason: ksni::OfflineReason) -> bool {
        let why = match reason {
            ksni::OfflineReason::No => "StatusNotifierWatcher went offline without a reason or error".to_string(),
            ksni::OfflineReason::Error(err) => err.to_string(),
            _ => format!("Reason is not known because ksni was updated without the match at {}:{} being updated", file!(), column!())
        };

        tracing::info!("Watcher went offline: {why}");

        true
    }
}

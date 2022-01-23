use crate::kcshot::KCShot;

mod sni;

/// Creates a systray icon.
///
/// This will attempt to create an systray icon as by trying to use the KDE/freedesktop Status Notifier
/// Item spec.
///
/// kcshot shall continue running even if a systray icon could not be initialised as
/// desktop environments may choose to not offer support for them or for the protocols we use
/// or the user may be using a WM without one.
pub fn init(app: &KCShot) {
    if sni::try_init(app) == Initialised::Yes {
        return;
    }

    tracing::warn!("Failed to initialise a systray icon. This is not fatal and you can use kcshot anyway. No core functionality is missing.");
}

#[derive(PartialEq, Eq)]
enum Initialised {
    Yes,
    No,
}

use gtk4::glib;

glib::wrapper! {
    pub struct SettingsWindow(ObjectSubclass<underlying::SettingsWindow>)
        @extends gtk4::Widget, adw::PreferencesDialog,
        @implements gtk4::ConstraintTarget, gtk4::Buildable, gtk4::Accessible,
                    gtk4::ShortcutManager, adw::Dialog, gtk4::Root, gtk4::Native;
}

impl Default for SettingsWindow {
    fn default() -> Self {
        glib::Object::new()
    }
}

mod underlying {
    use std::cell::OnceCell;

    use adw::subclass::{dialog::AdwDialogImpl, prelude::PreferencesDialogImpl};
    use gtk4::{CompositeTemplate, glib, prelude::*, subclass::prelude::*};
    use kcshot_data::settings::Settings;

    use crate::{ext::DisposeExt, kcshot::KCShot};

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(file = "src/settings_window.blp")]
    pub struct SettingsWindow {
        #[template_child]
        screenshot_directory_row: TemplateChild<adw::PreferencesRow>,
        #[template_child]
        history_enabled_switch: TemplateChild<adw::SwitchRow>,
        #[template_child]
        capture_mouse_switch: TemplateChild<adw::SwitchRow>,
        #[template_child]
        editing_starts_by_cropping_switch: TemplateChild<adw::SwitchRow>,

        settings: OnceCell<Settings>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SettingsWindow {
        const NAME: &'static str = "KCShotSettingsWindow";
        type Type = super::SettingsWindow;
        type ParentType = adw::PreferencesDialog;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("kcshot-settings-window");

            klass.bind_template();
            klass.bind_template_callbacks();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for SettingsWindow {
        fn constructed(&self) {
            self.parent_constructed();

            let settings = self.settings.get_or_init(Settings::open);

            settings
                .bind_saved_screenshots_path(&self.screenshot_directory_row.get(), "subtitle")
                .build();

            settings
                .bind_is_history_enabled(&self.history_enabled_switch.get(), "active")
                .build();
            settings
                .bind_capture_mouse_cursor(&self.capture_mouse_switch.get(), "active")
                .build();
            settings
                .bind_editing_starts_with_cropping(
                    &self.editing_starts_by_cropping_switch.get(),
                    "active",
                )
                .build();
        }

        fn dispose(&self) {
            self.obj().dispose_children();
        }
    }

    #[gtk4::template_callbacks]
    impl SettingsWindow {
        #[template_callback]
        async fn on_screenshot_directory_clicked(&self, _: adw::ActionRow) {
            let window = KCShot::the().main_window();
            let file_dialog = gtk4::FileDialog::builder()
                .modal(true)
                .title("Choose a folder")
                .build();
            match file_dialog.select_folder_future(Some(&window)).await {
                Ok(folder) => {
                    Settings::open().set_saved_screenshots_path(
                        &folder
                            .path()
                            .and_then(|path| path.to_str().map(str::to_owned))
                            .unwrap(),
                    );
                }
                Err(e) => {
                    tracing::error!("Failed picking new screenshot folder: {e:#?}")
                }
            };
        }
    }

    impl WidgetImpl for SettingsWindow {}
    impl PreferencesDialogImpl for SettingsWindow {}
    impl AdwDialogImpl for SettingsWindow {}
}

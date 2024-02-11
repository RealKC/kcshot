use gtk4::glib;

glib::wrapper! {
    pub struct SettingsWindow(ObjectSubclass<underlying::SettingsWindow>)
        @extends gtk4::Widget, gtk4::Window;
}

impl Default for SettingsWindow {
    fn default() -> Self {
        glib::Object::new()
    }
}

mod underlying {
    use std::cell::OnceCell;

    use gtk4::{glib, prelude::*, subclass::prelude::*, CompositeTemplate};
    use kcshot_data::settings::Settings;

    use crate::ext::DisposeExt;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(file = "src/settings_window.blp")]
    pub struct SettingsWindow {
        #[template_child]
        screenshot_directory_chooser_button: TemplateChild<gtk4::Button>,
        #[template_child]
        history_enabled_switch: TemplateChild<gtk4::Switch>,
        #[template_child]
        capture_mouse_switch: TemplateChild<gtk4::Switch>,
        #[template_child]
        editing_starts_by_cropping_switch: TemplateChild<gtk4::Switch>,

        settings: OnceCell<Settings>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SettingsWindow {
        const NAME: &'static str = "KCShotSettingsWindow";
        type Type = super::SettingsWindow;
        type ParentType = gtk4::Window;

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
                .bind_saved_screenshots_path(
                    &self.screenshot_directory_chooser_button.get(),
                    "label",
                )
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
        fn on_screenshot_directory_clicked(&self, _: gtk4::Button) {
            let folder_chooser = gtk4::FileChooserDialog::new(
                Some("Choose a folder for your screenshot history"),
                Some(self.obj().as_ref()),
                gtk4::FileChooserAction::SelectFolder,
                &[
                    ("Cancel", gtk4::ResponseType::Cancel),
                    ("Apply", gtk4::ResponseType::Apply),
                ],
            );

            folder_chooser.connect_response(|this, response| {
                if response == gtk4::ResponseType::Apply {
                    let folder = this.file().unwrap();
                    Settings::open().set_saved_screenshots_path(
                        &folder
                            .path()
                            .and_then(|path| path.to_str().map(str::to_owned))
                            .unwrap(),
                    );
                }

                this.destroy();
            });
        }
    }

    impl WidgetImpl for SettingsWindow {}
    impl WindowImpl for SettingsWindow {}
}

use gtk4::glib;

use crate::kcshot::KCShot;

use crate::historymodel::HistoryModel;

glib::wrapper! {
    pub struct AppWindow(ObjectSubclass<underlying::AppWindow>)
    @extends gtk4::Widget, gtk4::Window, gtk4::ApplicationWindow;
}

impl AppWindow {
    pub fn new(app: &KCShot, history_model: &HistoryModel) -> Self {
        glib::Object::new(&[("application", app), ("history-model", history_model)])
            .expect("Failed to make an AppWindow")
    }
}

mod underlying {
    use std::{ffi::OsString, process::Command};

    use gtk4::{
        gio,
        glib::{self, clone, signal::Inhibit, ParamSpec},
        prelude::*,
        subclass::{
            application_window::ApplicationWindowImpl,
            prelude::{ObjectImpl, ObjectSubclass, WidgetImpl, WindowImpl},
        },
        SignalListItemFactory,
    };
    use once_cell::sync::{Lazy, OnceCell};

    use crate::{editor::EditorWindow, kcshot::KCShot};

    use crate::historymodel::RowData;

    #[derive(Default, Debug)]
    pub struct AppWindow {
        history_model: OnceCell<super::HistoryModel>,
        settings: OnceCell<gio::Settings>,
        window: OnceCell<gtk4::Window>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AppWindow {
        const NAME: &'static str = "AppWindow";
        type Type = super::AppWindow;
        type ParentType = gtk4::ApplicationWindow;
    }

    impl ObjectImpl for AppWindow {
        fn constructed(&self, obj: &Self::Type) {
            let hbox = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);

            let list_model = self.history_model.get().unwrap();

            let (settings_window, button_list) =
                build_button_pane(&dbg!(obj.application()).unwrap(), list_model);
            self.window
                .set(settings_window)
                .expect("self.constructed called twice");
            let left_frame = gtk4::Frame::new(None);
            left_frame.set_child(Some(&button_list));
            hbox.append(&left_frame);

            let factory = build_item_factory();

            let selection_model = gtk4::SingleSelection::new(Some(list_model));

            let stack = gtk4::Stack::new();

            let image_grid = gtk4::GridView::new(Some(&selection_model), Some(&factory));
            image_grid.set_min_columns(3);
            let right_frame = gtk4::Frame::new(None);

            let message = gtk4::Box::new(gtk4::Orientation::Vertical, 2);

            let emoji = gtk4::Label::new(Some("(´• ω •`)"));
            emoji.add_css_class("kc-label-emoji");
            let css_provider = gtk4::CssProvider::new();
            css_provider.load_from_data(
                b"
.kc-label-emoji {
    font-size: 15em;
}

.kc-history-disabled-note {
    font-size: 3em;
}",
            );
            emoji
                .style_context()
                .add_provider(&css_provider, gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION);
            message.append(&emoji);

            let note = gtk4::Label::new(Some("The history is disabled"));
            note.add_css_class("kc-history-disabled-note");
            note.style_context()
                .add_provider(&css_provider, gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION);
            message.append(&note);

            stack.add_named(&image_grid, Some("image-grid"));
            stack.add_named(&message, Some("message"));

            self.settings
                .set(gio::Settings::new("kc.kcshot"))
                .expect("self.settings should only be set once");

            let settings = self.settings.get().unwrap();
            let is_history_enabled = settings.boolean("is-history-enabled");

            if is_history_enabled {
                stack.set_visible_child_name("image-grid")
            } else {
                stack.set_visible_child_name("message")
            }

            settings.connect_changed(
                None,
                clone!(@strong stack => move |settings, key| {
                    tracing::info!("Called with key: {}", key);
                    if key == "is-history-enabled" {
                        let is_history_enabled = settings.boolean(key);
                        if is_history_enabled {
                            stack.set_visible_child_name("image-grid")
                        } else {
                            stack.set_visible_child_name("message")
                        }
                    }
                }),
            );

            right_frame.set_child(Some(&stack));

            hbox.append(&right_frame);

            obj.set_child(Some(&hbox));
        }

        fn properties() -> &'static [ParamSpec] {
            static PROPERTIES: Lazy<Vec<ParamSpec>> = Lazy::new(|| {
                vec![
                    ParamSpec::new_object(
                        "application",
                        "Application",
                        "Application",
                        KCShot::static_type(),
                        glib::ParamFlags::READWRITE | glib::ParamFlags::CONSTRUCT_ONLY,
                    ),
                    ParamSpec::new_object(
                        "history-model",
                        "History Model",
                        "History Model",
                        super::HistoryModel::static_type(),
                        glib::ParamFlags::WRITABLE | glib::ParamFlags::CONSTRUCT_ONLY,
                    ),
                ]
            });

            PROPERTIES.as_ref()
        }

        #[tracing::instrument]
        fn property(&self, obj: &Self::Type, _id: usize, pspec: &ParamSpec) -> glib::Value {
            match pspec.name() {
                "application" => obj.application().to_value(),
                name => {
                    tracing::error!("Unknown property: {}", name);
                    panic!()
                }
            }
        }

        #[tracing::instrument]
        fn set_property(
            &self,
            obj: &Self::Type,
            _id: usize,
            value: &glib::Value,
            pspec: &ParamSpec,
        ) {
            match pspec.name() {
                "application" => {
                    let application = value.get::<KCShot>().ok();
                    obj.set_application(application.as_ref());
                }
                "history-model" => {
                    let history_model = value.get::<super::HistoryModel>().unwrap();
                    self.history_model
                        .set(history_model)
                        .expect("history-model should only be set once");
                }
                name => tracing::warn!("Unknown property: {}", name),
            }
        }
    }

    impl WindowImpl for AppWindow {
        fn close_request(&self, window: &Self::Type) -> Inhibit {
            window.hide();
            Inhibit(false)
        }
    }

    fn build_item_factory() -> SignalListItemFactory {
        let factory = gtk4::SignalListItemFactory::new();

        factory.connect_setup(|_this, list_item| {
            let picture = gtk4::Picture::builder()
                .height_request(400)
                .width_request(300)
                .build();

            list_item.set_child(Some(&picture));
        });

        factory.connect_bind(|_this, list_item| {
            let object = list_item
                .item()
                .expect("The item has to exist")
                .downcast::<RowData>()
                .unwrap();

            let picture = list_item
                .child()
                .expect("The child has to exist")
                .downcast::<gtk4::Picture>()
                .expect("The child has to be a gtk4::Picture");

            if let Some(path) = object.path() {
                picture.set_filename(&path);
            }
        });

        factory
    }

    fn build_button_pane(
        application: &gtk4::Application,
        history_model: &super::HistoryModel,
    ) -> (gtk4::Window, gtk4::Box) {
        let buttons = gtk4::Box::new(gtk4::Orientation::Vertical, 0);

        let capture_button = gtk4::Button::new();
        capture_button.set_child(Some(&make_label("Capture")));
        capture_button.connect_clicked(
            glib::clone!(@weak application, @weak history_model => move |_| {
                let editor_window = EditorWindow::new(&application, &history_model);
                editor_window.set_decorated(false);
                editor_window.fullscreen();

                editor_window.show();
            }),
        );
        buttons.append(&capture_button);

        let settings_button = gtk4::Button::new();
        settings_button.set_child(Some(&make_label("Settings")));
        let settings_window = build_settings_window();
        let settings_window_ = settings_window.clone();
        settings_button.connect_clicked(move |_| settings_window_.show());
        buttons.append(&settings_button);

        let hbox = gtk4::Box::new(gtk4::Orientation::Horizontal, 1);
        let screenshots_folder_about =
            gtk4::Label::new(Some("Choose where the screenshots should be saved"));
        hbox.append(&screenshots_folder_about);
        let screenshots_folder_button = gtk4::Button::new();
        hbox.append(&screenshots_folder_button);
        screenshots_folder_button.set_child(Some(&make_label("Screenshots folder")));
        screenshots_folder_button.connect_clicked(|_| {
            let res = Command::new("xdg-open")
                .arg(&get_screenshot_folder())
                .spawn();
            if let Err(why) = res {
                tracing::error!("Failed to spawn xdg-open: {:?}", why);
            }
        });
        buttons.append(&hbox);

        let history_button = gtk4::Button::new();
        history_button.set_child(Some(&make_label("History")));
        history_button.connect_clicked(|_| tracing::error!("TODO: Implement history button"));
        buttons.append(&history_button);

        (settings_window, buttons)
    }

    fn build_settings_window() -> gtk4::Window {
        let window = gtk4::Window::new();
        window.set_title(Some("kcshot - Settings"));
        let settings = gio::Settings::new("kc.kcshot");

        let history_enabled_button = gtk4::CheckButton::with_label("Enable history");
        settings
            .bind("is-history-enabled", &history_enabled_button, "active")
            .flags(gio::SettingsBindFlags::DEFAULT)
            .build();

        let content_area = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        window.set_child(Some(&content_area));

        content_area.append(&history_enabled_button);

        let folder_chooser = gtk4::FileChooserDialog::new(
            Some("choose a folder for your screenshot history"),
            Some(&window),
            gtk4::FileChooserAction::SelectFolder,
            &[
                ("Cancel", gtk4::ResponseType::Cancel),
                ("Apply", gtk4::ResponseType::Apply),
            ],
        );
        let settings_ = settings.clone();
        folder_chooser.connect_response(move |this, response| {
            if response == gtk4::ResponseType::Apply {
                let folder = this.file().unwrap();
                settings_
                    .set(
                        "saved-screenshots-path",
                        &folder.path().unwrap().to_str().unwrap(),
                    )
                    .unwrap();
            }
            tracing::info!("what");
            this.destroy();
        });

        let folder_chooser_button = gtk4::Button::new();
        settings
            .bind("saved-screenshots-path", &folder_chooser_button, "label")
            .flags(gio::SettingsBindFlags::DEFAULT)
            .build();
        folder_chooser_button.connect_clicked(move |_| {
            folder_chooser.show();
        });

        content_area.append(&folder_chooser_button);

        window
    }

    /// FIXME: Make this function return smth other than the current working directory
    fn get_screenshot_folder() -> OsString {
        std::env::current_dir().unwrap().into_os_string()
    }

    fn make_label(text: &str) -> gtk4::Label {
        let label = gtk4::Label::new(Some(text));
        label.set_halign(gtk4::Align::Start);
        label
    }

    impl WidgetImpl for AppWindow {}
    impl ApplicationWindowImpl for AppWindow {}
}

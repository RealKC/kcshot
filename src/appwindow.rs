use gtk4::glib;

use crate::{historymodel::HistoryModel, kcshot::KCShot};

glib::wrapper! {
    pub struct AppWindow(ObjectSubclass<underlying::AppWindow>)
        @extends gtk4::Widget, gtk4::Window, gtk4::ApplicationWindow,
        @implements gtk4::Native;
}

impl AppWindow {
    pub fn new(app: &KCShot, history_model: &HistoryModel) -> Self {
        glib::Object::builder()
            .property("application", app)
            .property("history-model", history_model)
            .build()
    }
}

mod underlying {
    use std::{cell::RefCell, process::Command};

    use gtk4::{
        glib::{self, clone, ParamSpec, Properties},
        prelude::*,
        subclass::{application_window::ApplicationWindowImpl, prelude::*},
    };
    use kcshot_data::settings::Settings;
    use once_cell::unsync::OnceCell;

    use crate::{editor::EditorWindow, historymodel::RowData, kcshot::KCShot};

    #[derive(Debug, Properties)]
    #[properties(wrapper_type = super::AppWindow)]
    pub struct AppWindow {
        #[property(get, set, construct_only)]
        history_model: RefCell<super::HistoryModel>,

        settings: OnceCell<Settings>,
    }

    impl Default for AppWindow {
        fn default() -> Self {
            Self {
                history_model: Default::default(),
                settings: OnceCell::with_value(Settings::open()),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AppWindow {
        const NAME: &'static str = "KCShotAppWindow";
        type Type = super::AppWindow;
        type ParentType = gtk4::ApplicationWindow;
    }

    impl ObjectImpl for AppWindow {
        fn constructed(&self) {
            let obj = self.obj();
            let settings = self.settings.get().unwrap();

            let hbox = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);

            obj.set_hide_on_close(true);

            let list_model = obj.history_model();

            let button_list = build_button_pane(KCShot::the().upcast_ref(), &list_model, settings);
            let left_frame = gtk4::Frame::new(None);
            left_frame.set_child(Some(&button_list));
            hbox.append(&left_frame);

            let factory = build_item_factory();

            let selection_model = gtk4::SingleSelection::new(Some(list_model));

            let stack = gtk4::Stack::new();

            let message = gtk4::Box::new(gtk4::Orientation::Vertical, 2);

            let emoji = gtk4::Label::new(Some("(´• ω •`)"));
            emoji.add_css_class("kc-label-emoji");
            let css_provider = gtk4::CssProvider::new();
            css_provider.load_from_data(
                "
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

            let image_grid = gtk4::GridView::new(Some(selection_model), Some(factory));
            image_grid.set_min_columns(3);
            let history_view = gtk4::ScrolledWindow::new();
            history_view.set_child(Some(&image_grid));
            history_view.set_propagate_natural_width(true);
            history_view.set_min_content_height(600);
            stack.add_named(&history_view, Some("image-grid"));
            stack.add_named(&message, Some("message"));

            let is_history_enabled = settings.is_history_enabled();

            if is_history_enabled {
                stack.set_visible_child_name("image-grid");
            } else {
                stack.set_visible_child_name("message");
            }

            settings.connect_is_history_enabled_changed(clone!(@strong stack => move |settings| {
                if settings.is_history_enabled() {
                    stack.set_visible_child_name("image-grid");
                } else {
                    stack.set_visible_child_name("message");
                }
            }));

            let right_frame = gtk4::Frame::new(None);
            right_frame.set_child(Some(&stack));

            hbox.append(&right_frame);

            obj.set_child(Some(&hbox));
        }

        fn properties() -> &'static [ParamSpec] {
            Self::derived_properties()
        }

        fn set_property(&self, id: usize, value: &glib::Value, pspec: &ParamSpec) {
            Self::derived_set_property(self, id, value, pspec);
        }

        fn property(&self, id: usize, pspec: &ParamSpec) -> glib::Value {
            Self::derived_property(self, id, pspec)
        }
    }

    impl WindowImpl for AppWindow {}

    fn build_item_factory() -> gtk4::SignalListItemFactory {
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
                .and_downcast::<RowData>()
                .expect("The item has to exist and be a RowData");

            let picture = list_item
                .child()
                .and_downcast::<gtk4::Picture>()
                .expect("The child has to exist and it should be a gtk4::Picture");

            if let Some(path) = object.path() {
                picture.set_filename(Some(&path));
            }
        });

        factory
    }

    fn build_button_pane(
        application: &gtk4::Application,
        history_model: &super::HistoryModel,
        settings: &Settings,
    ) -> gtk4::Box {
        let buttons = gtk4::Box::new(gtk4::Orientation::Vertical, 0);

        let capture_button = gtk4::Button::new();
        capture_button.set_child(Some(&make_label("Capture")));
        capture_button.connect_clicked(
            glib::clone!(@weak application, @weak history_model => move |_| {
                let editing_starts_with_cropping = Settings::open().editing_starts_with_cropping();

                EditorWindow::show(&application, editing_starts_with_cropping);
            }),
        );
        buttons.append(&capture_button);

        let settings_button = gtk4::Button::new();
        settings_button.set_child(Some(&make_label("Settings")));
        settings_button.connect_clicked(move |_| build_settings_window().show());
        buttons.append(&settings_button);

        let screenshots_folder_button = gtk4::Button::new();
        screenshots_folder_button.set_child(Some(&make_label("Screenshots folder")));
        screenshots_folder_button.connect_clicked(|_| {
            let res = Command::new("xdg-open")
                .arg(&KCShot::screenshot_folder())
                .spawn();
            if let Err(why) = res {
                tracing::error!("Failed to spawn xdg-open: {why}");
            }
        });
        buttons.append(&screenshots_folder_button);

        let history_button = gtk4::Button::new();
        history_button.set_child(Some(&make_label("History")));
        history_button.connect_clicked(|_| tracing::error!("TODO: Implement history button"));
        settings
            .bind_is_history_enabled(&history_button, "visible")
            .build();
        buttons.append(&history_button);

        let quit_button = gtk4::Button::new();
        quit_button.set_child(Some(&make_label("Quit kcshot")));
        quit_button.connect_clicked(glib::clone!(@weak application => move |_| {
            application.quit();
        }));
        buttons.append(&quit_button);

        buttons
    }

    fn build_settings_window() -> gtk4::Window {
        let window = gtk4::Window::new();
        window.set_title(Some("kcshot - Settings"));
        let settings = Settings::open();

        let folder_chooser = gtk4::FileChooserDialog::new(
            Some("Choose a folder for your screenshot history"),
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
                settings_.set_saved_screenshots_path(
                    &folder
                        .path()
                        .and_then(|path| path.to_str().map(str::to_owned))
                        .unwrap(),
                );
            }
            this.destroy();
        });

        let folder_chooser_about = gtk4::Label::new(Some("Screenshot directory"));
        let folder_chooser_button = gtk4::Button::new();

        settings
            .bind_saved_screenshots_path(&folder_chooser_button, "label")
            .build();
        folder_chooser_button.connect_clicked(move |_| {
            folder_chooser.show();
        });

        let content_area = gtk4::Box::new(gtk4::Orientation::Vertical, 4);

        let hbox = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
        settings.bind_is_history_enabled(&hbox, "sensitive").build();
        hbox.append(&folder_chooser_about);
        hbox.append(&folder_chooser_button);

        content_area.append(&hbox);

        let history_enabled_label = gtk4::Label::new(Some("Enable history"));
        history_enabled_label.set_halign(gtk4::Align::Start);
        let history_enabled_button = gtk4::Switch::new();
        history_enabled_button.set_halign(gtk4::Align::End);
        settings
            .bind_is_history_enabled(&history_enabled_button, "active")
            .build();

        let history_enabled = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
        history_enabled.set_homogeneous(true);
        history_enabled.append(&history_enabled_label);
        history_enabled.append(&history_enabled_button);

        content_area.append(&history_enabled);
        content_area.set_margin_top(5);
        content_area.set_margin_bottom(10);
        content_area.set_margin_start(10);
        content_area.set_margin_end(10);

        let capture_mouse_cursor_label = gtk4::Label::new(Some("Capture mouse cursor"));
        capture_mouse_cursor_label.set_halign(gtk4::Align::Start);
        let capture_mouse_cursor_button = gtk4::Switch::new();
        capture_mouse_cursor_button.set_halign(gtk4::Align::End);
        settings
            .bind_capture_mouse_cursor(&capture_mouse_cursor_button, "active")
            .build();

        let capture_mouse_cursor_container = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
        capture_mouse_cursor_container.set_homogeneous(true);
        capture_mouse_cursor_container.append(&capture_mouse_cursor_label);
        capture_mouse_cursor_container.append(&capture_mouse_cursor_button);

        content_area.append(&capture_mouse_cursor_container);
        content_area.set_margin_top(5);
        content_area.set_margin_bottom(10);
        content_area.set_margin_start(10);
        content_area.set_margin_end(10);

        let editing_starts_by_cropping_label = gtk4::Label::builder()
            .label("Editing starts by cropping")
            .halign(gtk4::Align::Start)
            .build();
        let editing_starts_by_cropping_button =
            gtk4::Switch::builder().halign(gtk4::Align::End).build();
        settings
            .bind_editing_starts_with_cropping(&editing_starts_by_cropping_button, "active")
            .build();
        let editing_starts_by_cropping_container = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(6)
            .homogeneous(true)
            .build();
        editing_starts_by_cropping_container.append(&editing_starts_by_cropping_label);
        editing_starts_by_cropping_container.append(&editing_starts_by_cropping_button);

        content_area.append(&editing_starts_by_cropping_container);
        content_area.set_margin_top(5);
        content_area.set_margin_bottom(10);
        content_area.set_margin_start(10);
        content_area.set_margin_end(10);

        let notebook = gtk4::Notebook::new();
        notebook.append_page(&content_area, Some(&gtk4::Label::new(Some("General"))));

        window.set_child(Some(&notebook));

        window
    }

    fn make_label(text: &str) -> gtk4::Label {
        let label = gtk4::Label::new(Some(text));
        label.set_halign(gtk4::Align::Start);
        label
    }

    impl WidgetImpl for AppWindow {}
    impl ApplicationWindowImpl for AppWindow {}
}

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
        CompositeTemplate,
    };
    use kcshot_data::settings::Settings;
    use once_cell::unsync::OnceCell;

    use crate::{editor::EditorWindow, historymodel::RowData, kcshot::KCShot};

    #[derive(Debug, Properties, CompositeTemplate)]
    #[properties(wrapper_type = super::AppWindow)]
    #[template(file = "src/appwindow.blp")]
    pub struct AppWindow {
        #[property(get, set, construct_only)]
        history_model: RefCell<super::HistoryModel>,

        #[template_child]
        image_grid: TemplateChild<gtk4::GridView>,
        #[template_child]
        stack: TemplateChild<gtk4::Stack>,
        #[template_child]
        history_button: TemplateChild<gtk4::Button>,

        settings: OnceCell<Settings>,
    }

    impl Default for AppWindow {
        fn default() -> Self {
            Self {
                history_model: Default::default(),
                image_grid: Default::default(),
                stack: Default::default(),
                history_button: Default::default(),
                settings: OnceCell::with_value(Settings::open()),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AppWindow {
        const NAME: &'static str = "KCShotAppWindow";
        type Type = super::AppWindow;
        type ParentType = gtk4::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_callbacks();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for AppWindow {
        fn constructed(&self) {
            // self.parent_constructed();

            let settings = self.settings.get().unwrap();

            let obj = self.obj();

            let list_model = obj.history_model();
            let selection_model = gtk4::SingleSelection::new(Some(list_model));
            self.image_grid.set_model(Some(&selection_model));

            let factory = build_item_factory();

            self.image_grid.set_factory(Some(&factory));

            let is_history_enabled = settings.is_history_enabled();
            if is_history_enabled {
                self.stack.set_visible_child_name("image-grid");
            } else {
                self.stack.set_visible_child_name("message");
            }

            settings.connect_is_history_enabled_changed(
                clone!(@strong self.stack as stack => move |settings| {
                    if settings.is_history_enabled() {
                        stack.set_visible_child_name("image-grid");
                    } else {
                        stack.set_visible_child_name("message");
                    }
                }),
            );
            settings
                .bind_is_history_enabled(&self.history_button.get(), "visible")
                .build();
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

    #[gtk4::template_callbacks]
    impl AppWindow {
        #[template_callback]
        fn on_capture_clicked(&self, _: &gtk4::Button) {
            let editing_starts_with_cropping = self.settings().editing_starts_with_cropping();

            EditorWindow::show(KCShot::the().upcast_ref(), editing_starts_with_cropping);
        }

        #[template_callback]
        fn on_settings_clicked(&self, _: &gtk4::Button) {
            build_settings_window().show();
        }

        #[template_callback]
        fn on_screenshots_folder_clicked(&self, _: &gtk4::Button) {
            let res = Command::new("xdg-open")
                .arg(&KCShot::screenshot_folder())
                .spawn();
            if let Err(why) = res {
                tracing::error!("Failed to spawn xdg-open: {why}");
            }
        }

        #[template_callback]
        fn on_history_clicked(&self, _: &gtk4::Button) {
            tracing::error!("TODO: Implement history button");
        }

        #[template_callback]
        fn on_quit_clicked(&self, _: &gtk4::Button) {
            KCShot::the().quit();
        }

        fn settings(&self) -> &Settings {
            self.settings.get().unwrap()
        }
    }

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

    impl WidgetImpl for AppWindow {}
    impl ApplicationWindowImpl for AppWindow {}
}

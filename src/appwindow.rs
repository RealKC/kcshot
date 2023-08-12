use gtk4::glib;

use crate::{history::HistoryModel, kcshot::KCShot};

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
        gdk,
        glib::{self, clone, Properties},
        prelude::*,
        subclass::{application_window::ApplicationWindowImpl, prelude::*},
        CompositeTemplate,
    };
    use kcshot_data::settings::Settings;
    use once_cell::unsync::OnceCell;

    use crate::{
        editor::EditorWindow, ext::DisposeExt, history, history::RowData, kcshot::KCShot,
        settings_window::SettingsWindow,
    };

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

    #[glib::derived_properties]
    impl ObjectImpl for AppWindow {
        fn constructed(&self) {
            self.parent_constructed();

            let settings = self.settings.get().unwrap();

            let obj = self.obj();

            let list_model = obj.history_model();
            let selection_model = gtk4::SingleSelection::new(Some(list_model));
            self.image_grid.set_model(Some(&selection_model));

            let factory = build_item_factory(selection_model);

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

        fn dispose(&self) {
            self.obj().dispose_children();
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
            SettingsWindow::default().show();
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

    fn build_item_factory(model: gtk4::SingleSelection) -> gtk4::SignalListItemFactory {
        let factory = gtk4::SignalListItemFactory::new();

        factory.connect_setup(|_this, list_item| {
            let picture = gtk4::Picture::builder()
                .height_request(400)
                .width_request(300)
                .build();

            list_item.set_child(Some(&picture));
        });

        factory.connect_bind(move |_this, list_item| {
            let object = list_item
                .item()
                .and_downcast::<RowData>()
                .expect("The item has to exist and be a RowData");

            let picture = list_item
                .child()
                .and_downcast::<gtk4::Picture>()
                .expect("The child has to exist and it should be a gtk4::Picture");

            let mouse = gtk4::GestureClick::builder()
                .button(gdk::BUTTON_SECONDARY)
                .build();
            mouse.connect_released(
                clone!(@strong list_item, @strong model, @strong picture, @strong object => move |_, _, x, y| {
                    model.set_selected(list_item.position());

                    match object.context_menu() {
                        Some(context_menu) => {
                            context_menu.set_pointing_to(Some(&gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
                            context_menu.popup();
                        }
                        None => {
                            let context_menu = history::context_menu(object.clone(), picture.upcast_ref());
                            context_menu.set_pointing_to(Some(&gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
                            context_menu.popup();
                            object.set_context_menu(context_menu);
                        }
                    }
                }),
            );
            picture.add_controller(mouse);

            if let Some(path) = object.path() {
                picture.set_filename(Some(&path));
            }
        });

        factory
    }

    impl WidgetImpl for AppWindow {}
    impl ApplicationWindowImpl for AppWindow {}
}

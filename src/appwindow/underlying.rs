use gtk4::{
    glib::{self, signal::Inhibit, ParamSpec},
    prelude::*,
    subclass::{
        application_window::ApplicationWindowImpl,
        prelude::{ObjectImpl, ObjectSubclass, WidgetImpl, WindowImpl},
    },
    SignalListItemFactory,
};
use once_cell::sync::{Lazy, OnceCell};

use crate::editor::EditorWindow;

use super::rowdata::RowData;

#[derive(Default, Debug)]
pub struct AppWindow {
    widgets: OnceCell<Widgets>,
}

#[derive(Debug)]
struct Widgets {
    hbox: gtk4::Box,
    image_grid: gtk4::GridView,
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

        let button_list = build_button_pane(&dbg!(obj.application()).unwrap());
        let left_frame = gtk4::Frame::new(None);
        left_frame.set_child(Some(&button_list));
        hbox.append(&left_frame);

        let factory = build_item_factory();

        let list_model = super::model::ListModel::new();
        let selection_model = gtk4::SingleSelection::new(Some(&list_model));

        let image_grid = gtk4::GridView::new(Some(&selection_model), Some(&factory));
        image_grid.set_min_columns(3);
        let right_frame = gtk4::Frame::new(None);
        right_frame.set_child(Some(&image_grid));

        hbox.append(&right_frame);

        obj.set_child(Some(&hbox));

        self.widgets
            .set(Widgets { hbox, image_grid })
            .expect("Failed to create an AppWindow")
    }

    fn properties() -> &'static [ParamSpec] {
        static PROPERTIES: Lazy<Vec<ParamSpec>> = Lazy::new(|| {
            vec![ParamSpec::new_object(
                "application",
                "Application",
                "Application",
                gtk4::Application::static_type(),
                glib::ParamFlags::READWRITE | glib::ParamFlags::CONSTRUCT_ONLY,
            )]
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
    fn set_property(&self, obj: &Self::Type, _id: usize, value: &glib::Value, pspec: &ParamSpec) {
        match pspec.name() {
            "application" => {
                let application = value.get::<gtk4::Application>().ok();
                obj.set_application(application.as_ref());
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

        picture.set_filename(&object.path());
    });

    factory
}

fn build_button_pane(application: &gtk4::Application) -> gtk4::Box {
    let buttons = gtk4::Box::new(gtk4::Orientation::Vertical, 0);

    let capture_button = gtk4::Button::new();
    capture_button.set_child(Some(&make_label("Capture")));
    capture_button.connect_clicked(glib::clone!(@weak application => move |_| {
        let editor_window = EditorWindow::new(&application);
        editor_window.set_decorated(false);
        editor_window.fullscreen();

        editor_window.show();
    }));
    buttons.append(&capture_button);

    let settings_button = gtk4::Button::new();
    settings_button.set_child(Some(&make_label("Settings")));
    settings_button.connect_clicked(|_| tracing::error!("TODO: Implement settings button"));
    buttons.append(&settings_button);

    let screenshots_folder_button = gtk4::Button::new();
    screenshots_folder_button.set_child(Some(&make_label("Screenshot folder")));
    screenshots_folder_button
        .connect_clicked(|_| tracing::error!("TODO: Implement screenshots folder button"));
    buttons.append(&screenshots_folder_button);

    let history_button = gtk4::Button::new();
    history_button.set_child(Some(&make_label("History")));
    history_button.connect_clicked(|_| tracing::error!("TODO: Implement history button"));
    buttons.append(&history_button);

    buttons
}

fn make_label(text: &str) -> gtk4::Label {
    let label = gtk4::Label::new(Some(text));
    label.set_halign(gtk4::Align::Start);
    label
}

impl WidgetImpl for AppWindow {}
impl ApplicationWindowImpl for AppWindow {}

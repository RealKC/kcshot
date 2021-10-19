use gtk::{
    cairo,
    gdk::keys::constants as GdkKey,
    glib::{self, clone, signal::Inhibit},
    prelude::*,
    subclass::prelude::*,
    Allocation,
};
use once_cell::unsync::OnceCell;
use tracing::{error, info, warn};

macro_rules! op {
    ($call:expr) => {
        match $call {
            Ok(_) => {}
            Err(err) => error!(
                "Got error: {:?}\n\twith the following call: {}",
                err,
                std::stringify!($call)
            ),
        }
    };
}

#[derive(Debug)]
struct Widgets {
    image: cairo::ImageSurface,
    overlay: gtk::Overlay,
    drawing_area: gtk::DrawingArea,
    toolbar: gtk::Fixed,
    tools: gtk::Box,
    button: gtk::Button,
}

#[derive(Default, Debug)]
pub struct EditorWindow {
    widgets: OnceCell<Widgets>,
}

#[glib::object_subclass]
impl ObjectSubclass for EditorWindow {
    const NAME: &'static str = "EditorWindow";
    type Type = super::EditorWindow;
    type ParentType = gtk::ApplicationWindow;
}

impl ObjectImpl for EditorWindow {
    fn constructed(&self, obj: &Self::Type) {
        self.parent_constructed(obj);
        let image = super::screenshot::take_screenshot().expect("Couldn't take a screenshot");
        warn!("Image status {:?}", image.status());

        let overlay = gtk::Overlay::new();
        overlay.set_visible(true);
        let drawing_area = gtk::DrawingArea::builder().can_focus(true).build();
        let toolbar = gtk::Fixed::new();
        let button = gtk::Button::new();
        let tools = gtk::Box::new(gtk::Orientation::Horizontal, 12);

        drawing_area.set_visible(true);
        drawing_area.size_allocate(&Allocation {
            x: 0,
            y: 0,
            width: image.width(),
            height: image.height(),
        });
        drawing_area.set_width_request(image.width());
        drawing_area.set_height_request(image.height());

        overlay.add(&drawing_area);
        overlay.add_overlay(&toolbar);

        toolbar.put(&tools, 1920 / 2 - 12, 1080 / 3);
        tools.add(&button);

        button.set_label("a");

        obj.add(&overlay);
        // obj.add(&drawing_area);

        drawing_area.connect_draw(
            clone!(@strong obj => @default-return Inhibit(false), move |_widget, cairo| {
                info!("wa");

                let instance = EditorWindow::from_instance(&obj);
                let image = &instance.widgets.get().unwrap().image;

                // op!(cairo.restore());
                // op!(cairo.save());

                cairo.set_operator(cairo::Operator::Source);

                op!(cairo.set_source_surface(image, 0f64,0f64));

                // op!(cairo.show_text("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAa"));

                op!(cairo.paint());

                Inhibit(false)
            }),
        );

        obj.connect_key_press_event(clone!(@strong obj => move |_this, key| {
            if key.keyval() == GdkKey::Escape {
                obj.hide();
            }
            Inhibit(false)
        }));

        self.widgets
            .set(Widgets {
                image,
                overlay,
                drawing_area,
                toolbar,
                tools,
                button,
            })
            .expect("Failed to create an editor");
    }
}

impl WidgetImpl for EditorWindow {}
impl ContainerImpl for EditorWindow {}
impl BinImpl for EditorWindow {}
impl WindowImpl for EditorWindow {}
impl ApplicationWindowImpl for EditorWindow {}

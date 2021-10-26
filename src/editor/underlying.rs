use std::{cell::RefCell, rc::Rc};

use cairo::{Context, ImageSurface};
use gtk::{
    cairo,
    gdk::keys::constants as GdkKey,
    glib::{self, clone, signal::Inhibit},
    pango::FontDescription,
    prelude::*,
    subclass::prelude::*,
    Allocation,
};
use once_cell::unsync::OnceCell;
use tracing::{error, warn};

use crate::editor::operations::{point::Point, Colour, Ellipse, Operation, Rectangle};

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

#[derive(Debug, Clone)]
struct Widgets {
    overlay: gtk::Overlay,
    drawing_area: gtk::DrawingArea,
    toolbar: gtk::Fixed,
    tools: gtk::Box,
    button: gtk::Button,
}

#[derive(Default, Debug)]
pub struct EditorWindow {
    widgets: OnceCell<Widgets>,
    image: Rc<RefCell<Option<cairo::ImageSurface>>>,
}

impl EditorWindow {
    fn do_draw_event(image: &mut ImageSurface, cairo: &Context) {
        cairo.set_operator(cairo::Operator::Source);
        op!(cairo.set_source_surface(image, 0f64, 0f64));
        op!(cairo.paint());
        cairo.set_operator(cairo::Operator::Over);

        op!(Operation::DrawEllipse {
            ellipse: Ellipse {
                x: 352.0,
                y: 36.0,
                w: 329.9,
                h: 460.0
            },
            border: Colour {
                red: 255,
                green: 0,
                blue: 0,
                alpha: 255
            },
            fill: Colour {
                red: 0,
                green: 0,
                blue: 255,
                alpha: 127
            }
        }
        .execute(image, cairo));

        op!(Operation::DrawRectangle {
            rect: Rectangle {
                x: 200.0,
                y: 300.0,
                w: 200.0,
                h: 300.0,
            },
            colour: Colour {
                red: 100,
                green: 100,
                blue: 100,
                alpha: 127
            }
        }
        .execute(image, cairo));

        op!(Operation::Highlight {
            rect: Rectangle {
                x: 0.0,
                y: 0.0,
                w: 100.0,
                h: 100.0
            }
        }
        .execute(image, cairo));

        op!(Operation::DrawLine {
            start: Point { x: 100.0, y: 100.0 },
            end: Point { x: 200.0, y: 150.0 },
            colour: Colour {
                red: 255,
                green: 0,
                blue: 0,
                alpha: 255
            }
        }
        .execute(image, cairo));

        op!(Operation::DrawArrow {
            start: Point { x: 50.0, y: 50.0 },
            end: Point { x: 200.0, y: 50.0 },
            colour: Colour {
                red: 0,
                green: 0,
                blue: 255,
                alpha: 255
            }
        }
        .execute(image, cairo));

        op!(Operation::Blur {
            rect: Rectangle {
                x: 100.0,
                y: 250.0,
                w: 50.0,
                h: 300.0
            },
            radius: 5.0
        }
        .execute(image, cairo));

        let font_description = FontDescription::from_string("Fira Code, 40pt");

        op!(Operation::Text {
            text: "<b>hello</b> <i>world</i>".into(),
            colour: Colour {
                red: 0,
                green: 255,
                blue: 0,
                alpha: 255
            },
            font_description
        }
        .execute(image, cairo));
    }
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
        let image = super::display_server::take_screenshot().expect("Couldn't take a screenshot");
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
            clone!(@strong self.image as image => @default-return Inhibit(false), move |_widget, cairo| {
                EditorWindow::do_draw_event(image.borrow_mut().as_mut().unwrap(), cairo);

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
                overlay,
                drawing_area,
                toolbar,
                tools,
                button,
            })
            .expect("Failed to create an editor");
        self.image.replace(Some(image));
    }
}

impl WidgetImpl for EditorWindow {}
impl ContainerImpl for EditorWindow {}
impl BinImpl for EditorWindow {}
impl WindowImpl for EditorWindow {}
impl ApplicationWindowImpl for EditorWindow {}

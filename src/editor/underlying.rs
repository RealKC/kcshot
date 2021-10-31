use std::{cell::RefCell, rc::Rc};

use cairo::{Context, ImageSurface};
use gtk::{
    cairo,
    gdk::{keys::constants as GdkKey, EventMask, BUTTON_PRIMARY},
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
    toolbar: gtk::Box,
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
            border: Colour {
                red: 127,
                green: 255,
                blue: 69,
                alpha: 254
            },
            fill: Colour {
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

        op!(Operation::Pixelate {
            rect: Rectangle {
                x: 400.0,
                y: 400.0,
                w: 200.0,
                h: 200.0
            },
            seed: 12345,
        }
        .execute(image, cairo));

        let font_description = FontDescription::from_string("Fira Code, 40pt");

        op!(Operation::Text {
            top_left: Point {
                x: 1000.0,
                y: 420.0,
            },
            text: "<b>hello</b> <i>world</i>".into(),
            colour: Colour {
                red: 0,
                green: 255,
                blue: 0,
                alpha: 255
            },
            font_description: font_description.clone()
        }
        .execute(image, cairo));

        op!(Operation::Bubble {
            centre: Point { x: 600.0, y: 600.0 },
            bubble_colour: Colour {
                red: 0,
                green: 0,
                blue: 255,
                alpha: 255
            },
            text_colour: Colour {
                red: 0,
                green: 0,
                blue: 0,
                alpha: 255
            },
            number: 123,
            font_description
        }
        .execute(image, cairo));
    }

    fn do_save_surface(app: &gtk::Application, image: &mut ImageSurface) {
        let cairo = match Context::new(image) {
            Ok(cairo) => cairo,
            Err(err) => {
                error!(
                    "Got error constructing cairo context inside button press event: {}",
                    err
                );
                return;
            }
        };
        EditorWindow::do_draw_event(image, &cairo);
        let now = chrono::Local::now();
        let stream = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .open(format!("screenshot_{}.png", now.to_rfc3339()));
        let mut stream = match stream {
            Ok(stream) => stream,
            Err(err) => {
                error!("Failed to open file: {}", err);
                return;
            }
        };
        if let Err(err) = image.write_to_png(&mut stream) {
            error!("Failed to write surface to png: {}", err);
        }
        app.quit();
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
        let drawing_area = gtk::DrawingArea::builder()
            .can_focus(true)
            .events(EventMask::ALL_EVENTS_MASK)
            .build();
        let button = gtk::Button::new();
        let toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        obj.add(&overlay);
        overlay.add(&drawing_area);
        button.set_label("long text");

        toolbar.add(&button);
        overlay.add_overlay(&toolbar);

        overlay.connect_get_child_position(|_this, widget| {
            let (screen_width, screen_height) = match super::display_server::get_screen_resolution()
            {
                Ok(res) => res,
                Err(why) => {
                    error!(
                        "Error getting screen resolution: {}.\n\t\tGoing with 1920x1080",
                        why
                    );
                    (1920, 1080)
                }
            };
            Some(Allocation {
                x: screen_width / 2 - widget.preferred_width().1 / 2,
                y: screen_height / 5,
                width: widget.preferred_width().1,
                height: widget.preferred_height().1,
            })
        });

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

        drawing_area.connect_button_press_event(
            clone!(@strong self.image as image, @strong obj => @default-return {warn!("A");Inhibit(false)}, move |_this, button| {
                tracing::warn!("y??");
                if button.button() == BUTTON_PRIMARY {
                    let mut image = image.borrow_mut();
                    let image = image.as_mut().unwrap();
                    let app = match obj.property("application") {
                        Ok(app) => app,
                        Err(err) => {
                            error!("{}", err);
                            return Inhibit(false);
                        }
                    };
                    match app.get::<gtk::Application>() {
                        Ok(app) => EditorWindow::do_save_surface(&app, image),
                        Err(err) => error!("{}", err),
                    }
                }
                Inhibit(false)
            }),
        );

        self.widgets
            .set(Widgets {
                overlay,
                drawing_area,
                toolbar,
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

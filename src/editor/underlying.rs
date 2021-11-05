use std::{cell::RefCell, rc::Rc};

use cairo::Context;
use gtk::{
    cairo,
    gdk::{keys::constants as GdkKey, EventMask, ModifierType, BUTTON_PRIMARY},
    glib::{self, clone, signal::Inhibit},
    prelude::*,
    subclass::prelude::*,
    Allocation, ResponseType,
};
use once_cell::unsync::OnceCell;
use tracing::{error, warn};

use crate::editor::{
    display_server::get_screen_resolution,
    operations::{Colour, Rectangle, Tool},
    utils::{self, CairoExt},
};

use super::operations::OperationStack;

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
    tool_buttons: Vec<gtk::RadioButton>,
}

#[derive(Debug)]
struct Image {
    surface: cairo::ImageSurface,
    operation_stack: OperationStack,
}

#[derive(Default, Debug)]
pub struct EditorWindow {
    widgets: OnceCell<Widgets>,
    image: Rc<RefCell<Option<Image>>>,
}

impl EditorWindow {
    fn do_draw(image: &Image, cairo: &Context, is_in_draw_event: bool) {
        cairo.set_operator(cairo::Operator::Source);
        op!(cairo.set_source_surface(&image.surface, 0f64, 0f64));
        op!(cairo.paint());
        cairo.set_operator(cairo::Operator::Over);

        image
            .operation_stack
            .execute(&image.surface, cairo, is_in_draw_event);
    }

    fn do_save_surface(app: &gtk::Application, image: &Image) {
        let cairo = match Context::new(&image.surface) {
            Ok(cairo) => cairo,
            Err(err) => {
                error!(
                    "Got error constructing cairo context inside button press event: {}",
                    err
                );
                return;
            }
        };
        EditorWindow::do_draw(image, &cairo, false);

        let rectangle = image.operation_stack.crop_region().unwrap_or_else(|| {
            let (w, h) = get_screen_resolution().map_or_else(
                |why| {
                    error!(
                        "Unable to retrieve screen resolution{}\n\t\tgoing with 1920x1080",
                        why
                    );
                    (1920, 1080)
                },
                |screen_resolution| screen_resolution,
            );
            Rectangle {
                x: 0.0,
                y: 0.0,
                w: w as f64,
                h: h as f64,
            }
        });

        let pixbuf = match utils::pixbuf_for(&image.surface, rectangle) {
            Some(pixbuf) => pixbuf,
            None => {
                error!(
                    "Failed to create a pixbuf from the surface: {:?} with crop region {:#?}",
                    image.surface, rectangle
                );
                return;
            }
        };

        let now = chrono::Local::now();
        let res = pixbuf.savev(format!("screenshot_{}.png", now.to_rfc3339()), "png", &[]);

        match res {
            Ok(_) => {}
            Err(why) => error!("Failed to save screenshot to file: {}", why),
        }

        app.quit();
    }

    fn make_primary_colour_chooser_button(
        image: Rc<RefCell<Option<Image>>>,
        parent_window: &gtk::Window,
    ) -> gtk::Button {
        let drawing_area = gtk::DrawingArea::builder()
            .events(EventMask::ALL_EVENTS_MASK)
            .build();
        drawing_area.set_size_request(32, 32);
        drawing_area.connect_draw(
            clone!(@strong image  => @default-return Inhibit(false), move |_this, cairo| {
                let image = image.borrow();
                let image = image.as_ref().unwrap();

                cairo.set_operator(cairo::Operator::Over);

                if image.operation_stack.primary_colour.alpha != 0 {
                    cairo.rectangle(0.0, 0.0, 32.0, 32.0);
                    cairo.set_source_colour(image.operation_stack.primary_colour);
                    op!(cairo.fill());
                } else {
                    // Instead of drawing nothing (what a fully transparent colour is) we draw a
                    // checkerboard pattern instead
                    cairo.set_source_colour(Colour {
                        red: 0xff,
                        green: 0x00,
                        blue: 0xdc,
                        alpha: 0xff
                    });
                    cairo.rectangle(0.0, 0.0, 16.0, 16.0);
                    op!(cairo.fill());
                    cairo.rectangle(16.0, 16.0, 16.0, 16.0);
                    op!(cairo.fill());

                    cairo.set_source_colour(Colour::BLACK);
                    cairo.rectangle(0.0, 16.0, 16.0, 16.0);
                    op!(cairo.fill());
                    cairo.rectangle(16.0, 0.0, 16.0, 16.0);
                    op!(cairo.fill());
                }

                cairo.set_source_colour(Colour::BLACK);
                cairo.rectangle(1.0, 1.0, 30.0, 30.0);
                cairo.set_line_width(1.0);
                op!(cairo.stroke());

                Inhibit(false)
            }),
        );

        Self::make_button::<true>(&drawing_area, parent_window, image)
    }

    fn make_secondary_colour_button(
        image: Rc<RefCell<Option<Image>>>,
        parent_window: &gtk::Window,
    ) -> gtk::Button {
        let drawing_area = gtk::DrawingArea::builder()
            .events(EventMask::ALL_EVENTS_MASK)
            .build();
        drawing_area.set_size_request(32, 32);
        drawing_area.connect_draw(
            clone!(@strong image  => @default-return Inhibit(false), move |_this, cairo| {
                let image = image.borrow();
                let image = image.as_ref().unwrap();

                cairo.set_operator(cairo::Operator::Over);

                cairo.set_source_colour(Colour::BLACK);
                cairo.rectangle(11.0, 11.0, 10.0, 10.0);
                cairo.set_line_width(1.0);
                op!(cairo.stroke());

                cairo.set_source_colour(image.operation_stack.secondary_colour);
                cairo.rectangle(8.0, 8.0, 16.0, 16.0);
                cairo.set_line_width(6.0);
                op!(cairo.stroke());

                cairo.set_source_colour(Colour::BLACK);
                cairo.rectangle(4.0, 4.0, 24.0, 24.0);
                cairo.set_line_width(1.0);
                op!(cairo.stroke());

                Inhibit(false)
            }),
        );

        Self::make_button::<false>(&drawing_area, parent_window, image)
    }

    fn make_button<const IS_PRIMARY: bool>(
        drawing_area: &gtk::DrawingArea,
        parent_window: &gtk::Window,
        image: Rc<RefCell<Option<Image>>>,
    ) -> gtk::Button {
        let button = gtk::Button::new();
        button.set_image(Some(drawing_area));

        button.connect_button_release_event(clone!(@strong parent_window, @strong image => move |_this, event| {
            if event.button() != BUTTON_PRIMARY {
                return Inhibit(false);
            }

            let colour_chooser = gtk::ColorChooserDialog::new(Some("Pick a colour"), Some(&parent_window));

            colour_chooser.connect_response(clone!(@strong image => move |this, response| {
                if response == ResponseType::Ok {
                    let mut image = image.borrow_mut();
                    let image = image.as_mut().unwrap();
                    if IS_PRIMARY {
                        image.operation_stack.primary_colour = Colour::from_gdk_rgba(this.rgba());
                    } else {
                        image.operation_stack.secondary_colour = Colour::from_gdk_rgba(this.rgba());
                    }
                }

                this.close();
            }));

            colour_chooser.show();

            Inhibit(false)
        }));

        button
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
        obj.add(&overlay);
        let drawing_area = gtk::DrawingArea::builder()
            .can_focus(true)
            .events(EventMask::ALL_EVENTS_MASK)
            .build();

        let toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 0);

        overlay.add(&drawing_area);
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
                EditorWindow::do_draw(image.borrow().as_ref().unwrap(), cairo, true);

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
            clone!(@strong self.image as image, @strong obj => @default-return Inhibit(false), move |this, button| {
                tracing::warn!("Got button-press on drawing_area");
                let mut image = image.borrow_mut();
                let image = image.as_mut().unwrap();
                image.operation_stack.start_operation_at(button.position().into());
                this.queue_draw();
                Inhibit(false)
            }),
        );

        drawing_area.connect_motion_notify_event(
            clone!(@strong self.image as image => @default-return Inhibit(false), move |this, motion| {
                let primary_button_is_held = motion.state().contains(ModifierType::BUTTON1_MASK);
                if primary_button_is_held {
                    let mut image = image.borrow_mut();
                    let image = image.as_mut().unwrap();
                    image.operation_stack.update_current_operation_end_coordinate(motion.position().into());
                    this.queue_draw();
                }
                Inhibit(false)
            }),
        );

        drawing_area.connect_button_release_event(
            clone!(@strong self.image as image, @strong obj => @default-return {warn!("A");Inhibit(false)}, move |this, button| {
                tracing::warn!("y??");
                if button.button() == BUTTON_PRIMARY {
                    let mut image = image.borrow_mut();
                    let image = image.as_mut().unwrap();
                    if image.operation_stack.current_tool() != Tool::CropAndSave {
                        tracing::info!("This is called");
                        image.operation_stack.finish_current_operation();
                        this.queue_draw();
                        return Inhibit(false);
                    }

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
            })
        );

        self.image.replace(Some(Image {
            surface: image,
            operation_stack: OperationStack::new(),
        }));

        fn make_tool_button(
            tool: Tool,
            toolbar: &gtk::Box,
            image: Rc<RefCell<Option<Image>>>,
        ) -> gtk::RadioButton {
            let button = gtk::RadioButton::builder()
                .image(&gtk::Image::from_file(tool.path()))
                .build();
            button.connect_clicked(clone!(@strong image => move |_| {
                image.borrow_mut().as_mut().unwrap().operation_stack.set_current_tool(tool);
            }));
            button.set_mode(false);
            toolbar.pack_start(&button, false, true, 0);
            button
        }

        let tool_buttons = vec![
            make_tool_button(Tool::CropAndSave, &toolbar, self.image.clone()),
            make_tool_button(Tool::Line, &toolbar, self.image.clone()),
            make_tool_button(Tool::Arrow, &toolbar, self.image.clone()),
            make_tool_button(Tool::Rectangle, &toolbar, self.image.clone()),
            make_tool_button(Tool::Highlight, &toolbar, self.image.clone()),
            make_tool_button(Tool::Ellipse, &toolbar, self.image.clone()),
            make_tool_button(Tool::Pixelate, &toolbar, self.image.clone()),
            make_tool_button(Tool::Blur, &toolbar, self.image.clone()),
            make_tool_button(Tool::AutoincrementBubble, &toolbar, self.image.clone()),
            make_tool_button(Tool::Text, &toolbar, self.image.clone()),
        ];

        let group_source = &tool_buttons[0];

        for button in tool_buttons.iter().skip(1) {
            button.join_group(Some(group_source));
        }

        let primary_colour_button =
            EditorWindow::make_primary_colour_chooser_button(self.image.clone(), obj.upcast_ref());
        toolbar.pack_start(&primary_colour_button, false, true, 0);

        let secondary_colour_button =
            EditorWindow::make_secondary_colour_button(self.image.clone(), obj.upcast_ref());
        toolbar.pack_start(&secondary_colour_button, false, true, 0);

        self.widgets
            .set(Widgets {
                overlay,
                drawing_area,
                toolbar,
                tool_buttons,
            })
            .expect("Failed to create an editor");
    }
}

impl WidgetImpl for EditorWindow {}
impl ContainerImpl for EditorWindow {}
impl BinImpl for EditorWindow {}
impl WindowImpl for EditorWindow {}
impl ApplicationWindowImpl for EditorWindow {}

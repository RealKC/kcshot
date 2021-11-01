use std::{cell::RefCell, rc::Rc};

use cairo::Context;
use gtk::{
    cairo,
    gdk::{keys::constants as GdkKey, EventMask, ModifierType, BUTTON_PRIMARY},
    glib::{self, clone, signal::Inhibit},
    prelude::*,
    subclass::prelude::*,
    Allocation,
};
use once_cell::unsync::OnceCell;
use tracing::{error, warn};

use crate::editor::operations::Tool;

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
    toolbar: gtk::Toolbar,
    tool_buttons: Vec<gtk::ToolButton>,
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
    fn do_draw_event(image: &Image, cairo: &Context) {
        cairo.set_operator(cairo::Operator::Source);
        op!(cairo.set_source_surface(&image.surface, 0f64, 0f64));
        op!(cairo.paint());
        cairo.set_operator(cairo::Operator::Over);

        image.operation_stack.execute(&image.surface, cairo);
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
        if let Err(err) = image.surface.write_to_png(&mut stream) {
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
        obj.add(&overlay);
        let drawing_area = gtk::DrawingArea::builder()
            .can_focus(true)
            .events(EventMask::ALL_EVENTS_MASK)
            .build();

        let toolbar = gtk::Toolbar::new();

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
                EditorWindow::do_draw_event(image.borrow().as_ref().unwrap(), cairo);

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
            toolbar: &gtk::Toolbar,
            image: Rc<RefCell<Option<Image>>>,
        ) -> gtk::ToolButton {
            let button = gtk::ToolButton::builder()
                .icon_widget(&gtk::Image::from_file(tool.path()))
                .build();
            gtk::prelude::ToolItemExt::set_expand(&button, false);
            button.connect_clicked(clone!(@strong image => move |_| {
                image.borrow_mut().as_mut().unwrap().operation_stack.set_current_tool(tool);
            }));
            toolbar.insert(&button, tool as i32);
            button
        }

        let tool_buttons = vec![
            make_tool_button(Tool::CropAndSave, &toolbar, self.image.clone()),
            make_tool_button(Tool::Line, &toolbar, self.image.clone()),
            make_tool_button(Tool::Arrow, &toolbar, self.image.clone()),
            make_tool_button(Tool::Rectangle, &toolbar, self.image.clone()),
            make_tool_button(Tool::Highlight, &toolbar, self.image.clone()),
            make_tool_button(Tool::Pixelate, &toolbar, self.image.clone()),
            make_tool_button(Tool::Blur, &toolbar, self.image.clone()),
            make_tool_button(Tool::AutoincrementBubble, &toolbar, self.image.clone()),
            make_tool_button(Tool::Text, &toolbar, self.image.clone()),
        ];

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

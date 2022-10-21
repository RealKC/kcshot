use std::cell::{Cell, RefCell};

use cairo::Context;
use diesel::SqliteConnection;
use gtk4::{
    gdk::{self, BUTTON_PRIMARY, BUTTON_SECONDARY},
    gio,
    glib::{self, clone, ParamSpec},
    prelude::*,
    subclass::prelude::*,
    Allocation,
};
use once_cell::sync::{Lazy, OnceCell};
use tracing::error;

use super::{toolbar, utils::ContextLogger, Colour};
use crate::{
    editor::{
        data::{Point, Rectangle},
        display_server,
        operations::{OperationStack, SelectionMode, Tool},
        utils,
    },
    historymodel::ModelNotifier,
    kcshot::KCShot,
    log_if_err,
    postcapture::run_postcapture_actions,
};

#[derive(Debug)]
pub(super) struct Image {
    surface: cairo::ImageSurface,
    pub(super) operation_stack: OperationStack,
}

impl Image {
    fn get_colour_at(&self, x: f64, y: f64) -> Colour {
        let (x, y) = (x as usize, y as usize);

        let width = self.surface.width() as usize;
        let idx = x + (y * width);

        let mut red = 255;
        let mut green = 255;
        let mut blue = 255;

        self.surface
            .with_data(|data| {
                blue = data[idx];
                green = data[idx + 1];
                red = data[idx + 2];
            })
            .unwrap();

        Colour {
            red,
            green,
            blue,
            alpha: 255,
        }
    }
}

#[derive(Default)]
pub struct EditorWindow {
    pub(super) image: RefCell<Option<Image>>,
    pub(super) is_picking_colour: Cell<bool>,
    overlay: OnceCell<gtk4::Overlay>,
    editing_started_with_cropping: Cell<bool>,

    pub(super) colour_tx: Cell<Option<glib::Sender<Colour>>>,
}

impl std::fmt::Debug for EditorWindow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EditorWindow")
            .field("image", &self.image)
            .field("is_picking_colour", &self.is_picking_colour)
            .field("overlay", &self.overlay)
            .field(
                "editing_started_with_cropping",
                &self.editing_started_with_cropping,
            )
            .field("colour_tx", &"<...>")
            .finish()
    }
}

impl EditorWindow {
    fn do_draw(image: &Image, cairo: &Context, is_in_draw_event: bool) {
        cairo.set_operator(cairo::Operator::Source);
        log_if_err!(cairo.set_source_surface(&image.surface, 0f64, 0f64));
        log_if_err!(cairo.paint());
        cairo.set_operator(cairo::Operator::Over);

        image
            .operation_stack
            .execute(&image.surface, cairo, is_in_draw_event);
    }

    pub(super) fn do_save_surface(
        model_notifier: &ModelNotifier,
        conn: &mut SqliteConnection,
        window: &gtk4::Window,
        image: &Image,
        point: Option<Point>,
    ) {
        let cairo = match Context::new(&image.surface) {
            Ok(cairo) => cairo,
            Err(err) => {
                error!("Got error constructing Cairo context inside do_save_surface: {err}");
                return;
            }
        };
        EditorWindow::do_draw(image, &cairo, false);

        let rectangle = image
            .operation_stack
            .crop_region(point)
            .unwrap_or(image.operation_stack.screen_dimensions);

        window.close();

        match utils::pixbuf_for(&image.surface, rectangle) {
            // Process all post capture actions
            Some(mut pixbuf) => run_postcapture_actions(model_notifier, conn, &mut pixbuf),
            None => {
                error!(
                    "Failed to create a pixbuf from the surface: {:?} with crop region {:#?}",
                    image.surface,
                    rectangle.normalised()
                );
            }
        };
    }

    pub(super) fn with_image<F, T>(&self, ctx: &str, func: F) -> Option<T>
    where
        F: FnOnce(&Image) -> T,
    {
        let _ctx = ContextLogger::new(ctx, "with_image");

        match self.image.try_borrow() {
            Ok(image) => {
                if let Some(image) = image.as_ref() {
                    return Some(func(image));
                }
            }
            Err(why) => {
                if ctx.is_empty() {
                    tracing::info!(
                        "Failed to immutably borrow self.image:\n\t- error: '{why}' ({why:?})"
                    );
                } else {
                    tracing::info!(
                        "Failed to immutably borrow self.image:\n\t- error: '{why}' ({why:?})\n\t- context: {ctx}"
                    );
                }
            }
        }

        None
    }

    pub(super) fn with_image_mut<F, T>(&self, ctx: &str, func: F) -> Option<T>
    where
        F: FnOnce(&mut Image) -> T,
    {
        let _ctx = ContextLogger::new(ctx, "with_image_mut");

        match self.image.try_borrow_mut() {
            Ok(mut image) => {
                if let Some(image) = image.as_mut() {
                    return Some(func(image));
                }
            }
            Err(why) => {
                if ctx.is_empty() {
                    tracing::info!(
                        "Failed to mutably borrow self.image:\n\t- error: '{why}' ({why:?})"
                    );
                } else {
                    tracing::info!(
                        "Failed to borrow mutably self.image:\n\t- error: '{why}' ({why:?})\n\t- context: {ctx}"
                    );
                }
            }
        }

        None
    }
}

#[glib::object_subclass]
impl ObjectSubclass for EditorWindow {
    const NAME: &'static str = "KCShotEditorWindow";
    type Type = super::EditorWindow;
    type ParentType = gtk4::ApplicationWindow;
}

impl ObjectImpl for EditorWindow {
    fn constructed(&self) {
        self.parent_constructed();
        let obj = self.instance();

        let app = obj.application().unwrap().downcast::<KCShot>().unwrap();
        let image =
            super::display_server::take_screenshot(&app).expect("Couldn't take a screenshot");
        let windows = display_server::get_windows().unwrap_or_else(|why| {
            tracing::info!("Got while trying to retrieve windows: {why}");
            vec![]
        });
        let screen_dimensions = Rectangle {
            x: 0.0,
            y: 0.0,
            w: image.width() as f64,
            h: image.height() as f64,
        };

        let overlay = gtk4::Overlay::new();
        obj.set_child(Some(&overlay));
        let drawing_area = gtk4::DrawingArea::builder().can_focus(true).build();

        overlay.set_child(Some(&drawing_area));

        let toolbar = toolbar::ToolbarWidget::new(&obj, self.editing_started_with_cropping.get());
        overlay.add_overlay(&toolbar);

        overlay.connect_get_child_position(move |_this, widget| {
            let Rectangle {
                w: screen_width,
                h: screen_height,
                ..
            } = screen_dimensions;

            Some(Allocation::new(
                (screen_width / 2.0 - widget.width() as f64 / 2.0) as i32,
                (screen_height / 5.0) as i32,
                11 * 32,
                32,
            ))
        });

        self.overlay
            .set(overlay)
            .expect("construct should not be called more than once");

        drawing_area.set_draw_func(clone!(@weak obj => move |_widget, cairo, _w, _h| {
            obj.imp().with_image("draw event", |image| EditorWindow::do_draw(image, cairo, true));
        }));

        let click_event_handler = gtk4::GestureClick::new();

        click_event_handler.set_button(0);
        click_event_handler.connect_pressed(clone!(@weak obj =>  move |this, _n_clicks, x, y| {
            if this.current_button() == BUTTON_PRIMARY {
                let is_picking_colour = obj.imp().is_picking_colour.get();

                if is_picking_colour {
                    obj.imp().with_image("colour picker", |image| {
                        let colour = image.get_colour_at(x, y);
                        if let Some(colour_tx) = obj.imp().colour_tx.take() {
                            if let Err(why) = colour_tx.send(colour) {
                                tracing::error!("Failed to send colour through colour_tx: {why}");
                            }
                        } else {
                            tracing::error!("A colour has been picked but we don't have a colour_tx?!");
                        }
                        obj.imp().is_picking_colour.set(false);
                    });
                } else {
                    obj.imp().with_image_mut("primary button pressed", |image| {
                        image.operation_stack.start_operation_at(Point { x, y });
                    });
                }
            } else if this.current_button() == BUTTON_SECONDARY {
                obj.close();
            }
        }));

        let motion_event_handler = gtk4::EventControllerMotion::new();
        motion_event_handler.connect_motion(
            clone!(@weak obj, @weak drawing_area => move |_, x, y| {
                obj.imp().with_image_mut("motion event", |image| {
                    image.operation_stack.set_current_window(x, y);
                    drawing_area.queue_draw();
                });
            }),
        );
        drawing_area.add_controller(&motion_event_handler);

        click_event_handler.connect_released(
            clone!(@weak obj, @weak drawing_area, @weak app => move |_this, _n_clicks, x, y| {
                let should_queue_draw = obj.imp().with_image_mut("mouse button released event", |image| {
                    if image.operation_stack.current_tool() == Tool::Text {
                        super::textdialog::pop_text_dialog_and_get_text(&obj);
                        true
                    } else if !image.operation_stack.current_tool().is_saving_tool() {
                        image.operation_stack.finish_current_operation();
                        true
                    } else {
                        image.operation_stack.finish_current_operation();

                        app.with_conn(|conn| EditorWindow::do_save_surface(
                            &app.model_notifier(),
                            conn,
                            obj.upcast_ref(),
                            image,
                            Some(Point { x, y })
                        ));
                        false
                    }
                });

                if should_queue_draw.unwrap_or(true) {
                    drawing_area.queue_draw();
                }
            }),
        );

        drawing_area.add_controller(&click_event_handler);

        let drag_controller = gtk4::GestureDrag::new();
        drag_controller.connect_drag_update(
            clone!(@weak obj, @weak drawing_area =>  move |_this, x, y| {
                obj.imp().with_image_mut("drag update event", |image| {
                    image.operation_stack.update_current_operation_end_coordinate(x, y);
                    if image.operation_stack.current_tool().is_cropping_tool() {
                        image.operation_stack.set_is_in_crop_drag(true);
                    }
                    drawing_area.queue_draw();
                });
            }),
        );
        drag_controller.connect_drag_end(
            clone!(@weak obj, @weak drawing_area, @weak toolbar => move |_, x, y| {
                obj.imp().with_image_mut("drag end event", |image| {
                    image.operation_stack.update_current_operation_end_coordinate(x, y);
                    if image.operation_stack.current_tool() == Tool::Crop {
                        toolbar.set_visible(true);
                        image.operation_stack.finish_current_operation();
                        image.operation_stack.set_current_tool(Tool::Pencil);
                    }
                    drawing_area.queue_draw();
                });
            }),
        );
        drawing_area.add_controller(&drag_controller);

        let key_event_controller = gtk4::EventControllerKey::new();
        key_event_controller.connect_key_pressed(
            clone!(@weak obj, @weak drawing_area => @default-return gtk4::Inhibit(false), move |_, key, _, _| {
                obj.imp().with_image_mut("key pressed event", |image| {
                    if key == gdk::Key::Control_L || key == gdk::Key::Control_R {
                        image.operation_stack.set_ignore_windows(true);
                        drawing_area.queue_draw();
                    } else if key == gdk::Key::Return {
                        if !obj.imp().editing_started_with_cropping.get() {
                            // Saving a screenshot using `Return` only makes sense in "crop-first"
                            // mode
                            return;
                        }

                        let app = obj
                            .application()
                            .and_then(|app| app.downcast::<KCShot>().ok())
                            .expect("The EditorWindow's application should always be an instance of `KCShot`");

                        app.with_conn(|conn| Self::do_save_surface(
                            &app.model_notifier(),
                            conn,
                            obj.upcast_ref(),
                            image,
                            None
                        ));
                    } else if key == gdk::Key::Shift_L || key == gdk::Key::Shift_R {
                        image.operation_stack.selection_mode = SelectionMode::WindowsWithoutDecorations;
                    }
                });
                gtk4::Inhibit(false)
            }),
        );
        key_event_controller.connect_key_released(
            clone!(@weak obj, @weak drawing_area => move |_, key, _, _| {
                obj.imp().with_image_mut("key released event", |image| {
                    if key == gdk::Key::Control_L || key == gdk::Key::Control_R {
                        image.operation_stack.set_ignore_windows(false);
                        drawing_area.queue_draw();
                    } else if key == gdk::Key::Escape {
                        obj.close();
                    } else if key == gdk::Key::Shift_L || key == gdk::Key::Shift_R {
                        image.operation_stack.selection_mode = SelectionMode::WindowsWithDecorations;
                    }
                });
            }),
        );
        obj.add_controller(&key_event_controller);

        let undo_action = gio::SimpleAction::new("undo", None);
        undo_action.connect_activate(clone!(@weak obj, @weak drawing_area => move |_, _| {
            obj.imp().with_image_mut("win.undo activated", |image| {
                image.operation_stack.undo();
                drawing_area.queue_draw();
            });
        }));
        obj.add_action(&undo_action);

        let redo_action = gio::SimpleAction::new("redo", None);
        redo_action.connect_activate(clone!(@weak obj, @weak drawing_area => move |_, _| {
            obj.imp().with_image_mut("win.redo activated", |image| {
                image.operation_stack.redo();
                drawing_area.queue_draw();
            });
        }));
        obj.add_action(&redo_action);

        // FIXME: Figure out how/if we make this work across keyboard layouts that don't have Z and Y
        // in the same place QWERTY does.
        app.set_accels_for_action("win.undo", &["<Ctrl>Z"]);
        app.set_accels_for_action("win.redo", &["<Ctrl>Y"]);

        self.image.replace(Some(Image {
            surface: image,
            operation_stack: OperationStack::new(
                windows,
                screen_dimensions,
                self.editing_started_with_cropping.get(),
            ),
        }));
    }

    fn dispose(&self) {
        if let Some(overlay) = self.overlay.get() {
            overlay.unparent();
        }

        self.with_image_mut("dispose", |image| image.surface.finish());
    }

    fn properties() -> &'static [ParamSpec] {
        static PROPERTIES: Lazy<Vec<ParamSpec>> = Lazy::new(|| {
            use crate::properties::*;
            vec![
                construct_only_rw_object_property::<KCShot>("application"),
                glib::ParamSpecBoolean::builder("editing-starts-with-cropping")
                    .default_value(false)
                    .write_only()
                    .construct_only()
                    .build(),
            ]
        });

        PROPERTIES.as_ref()
    }

    #[tracing::instrument]
    fn property(&self, _id: usize, pspec: &ParamSpec) -> glib::Value {
        match pspec.name() {
            "application" => self.instance().application().to_value(),
            name => {
                tracing::error!("Unknown property: {name}");
                panic!()
            }
        }
    }

    #[tracing::instrument]
    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &ParamSpec) {
        match pspec.name() {
            "application" => {
                let application = value.get::<KCShot>().ok();
                self.instance().set_application(application.as_ref());
            }
            "editing-starts-with-cropping" => {
                let editing_starts_with_cropping = value.get::<bool>();
                match editing_starts_with_cropping {
                    Ok(b) => {
                        self.editing_started_with_cropping.set(b);
                    }
                    Err(why) => {
                        tracing::error!("set_property called for editing-starts-with-cropping but with the wrong type: {why}");
                    }
                }
            }
            name => tracing::warn!("Unknown property: {name}"),
        }
    }
}

impl WidgetImpl for EditorWindow {}
impl WindowImpl for EditorWindow {}
impl ApplicationWindowImpl for EditorWindow {}

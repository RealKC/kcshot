use std::cell::{Cell, OnceCell, RefCell};

use cairo::Context;
use diesel::SqliteConnection;
use gtk4::{
    gdk::{self, BUTTON_PRIMARY, BUTTON_SECONDARY},
    gio,
    glib::{self, clone, ParamSpec, Properties},
    prelude::*,
    subclass::prelude::*,
    Allocation, CompositeTemplate,
};
use kcshot_data::geometry::{Point, Rectangle};
use tracing::error;

use super::{toolbar, utils::ContextLogger, Colour};
use crate::{
    editor::{
        operations::{OperationStack, SelectionMode, Tool},
        utils,
    },
    ext::DisposeExt,
    history::ModelNotifier,
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

        let stride = self.surface.stride() as usize;
        // NOTE: We multiply by 4 here because CAIRO_FORMAT_RGB24 pixels are 4 bytes in size
        let idx = x * 4 + (y * stride);

        let mut red = 255;
        let mut green = 255;
        let mut blue = 255;

        self.surface
            .with_data(|data| {
                // NOTE: The documentation for CAIRO_FORMAT_RGB24 doesn't mention it explicitly, but
                //       we can extrapolate from the docs for CAIRO_FORMAT_ARGB that **both** formats
                //       are stored native-endian, in our case this means little-endian, so "each pixel
                //       is a 32-bit quantity, with the upper 8 bits unused. Red, Green, and Blue are
                //       stored in the remaining 24 bits in that order. (Since 1.0)" ends up meaning
                //       that the order of the channels in memory is blue-green-red-unused.
                // See: https://cairographics.org/manual/cairo-Image-Surfaces.html#cairo-format-t
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

#[derive(Default, Properties, CompositeTemplate)]
#[properties(wrapper_type = super::EditorWindow)]
#[template(file = "src/editor/editor.blp")]
pub struct EditorWindow {
    #[property(name = "editing-starts-with-cropping", construct_only, set)]
    editing_started_with_cropping: Cell<bool>,

    pub(super) image: RefCell<Option<Image>>,

    #[template_child]
    overlay: TemplateChild<gtk4::Overlay>,
    #[template_child]
    drawing_area: TemplateChild<gtk4::DrawingArea>,

    toolbar: OnceCell<toolbar::ToolbarWidget>,

    /// This field is part of the "pick a colour from the screen" mechanism, we send the colour under
    /// the mouse cursor to the colour chooser dialog currently open
    pub(super) colour_tx: Cell<Option<glib::Sender<Colour>>>,
}

impl std::fmt::Debug for EditorWindow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EditorWindow")
            .field("image", &self.image)
            .field(
                "editing_started_with_cropping",
                &self.editing_started_with_cropping,
            )
            .field("colour_tx", &"<...>")
            .finish()
    }
}

#[glib::object_subclass]
impl ObjectSubclass for EditorWindow {
    const NAME: &'static str = "KCShotEditorWindow";
    type Type = super::EditorWindow;
    type ParentType = gtk4::ApplicationWindow;

    fn class_init(klass: &mut Self::Class) {
        klass.bind_template();
        klass.bind_template_callbacks();
    }

    fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
        obj.init_template();
    }
}

impl ObjectImpl for EditorWindow {
    fn constructed(&self) {
        self.parent_constructed();
        let obj = self.obj();

        let image = kcshot_screenshot::take_screenshot(KCShot::the().tokio_rt())
            .expect("Couldn't take a screenshot");
        let windows = kcshot_screenshot::get_windows().unwrap_or_else(|why| {
            tracing::info!("Got while trying to retrieve windows: {why}");
            vec![]
        });
        let screen_dimensions = Rectangle {
            x: 0.0,
            y: 0.0,
            w: image.width() as f64,
            h: image.height() as f64,
        };

        self.overlay.connect_get_child_position({
            let obj = obj.clone();
            move |_, widget| obj.imp().on_get_child_position(widget)
        });

        let toolbar = self.toolbar.get_or_init(|| {
            toolbar::ToolbarWidget::new(&obj, self.editing_started_with_cropping.get())
        });
        self.overlay.add_overlay(toolbar);

        self.drawing_area.set_draw_func(clone!(@weak obj => move |_, cairo, _, _| {
            obj.imp().with_image("draw event", |image| EditorWindow::do_draw(image, cairo, true));
        }));

        self.setup_actions();

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
        self.obj().dispose_children();
        self.with_image_mut("dispose", |image| image.surface.finish());
    }

    fn properties() -> &'static [ParamSpec] {
        Self::derived_properties()
    }

    #[tracing::instrument]
    fn set_property(&self, id: usize, value: &glib::Value, pspec: &ParamSpec) {
        Self::derived_set_property(self, id, value, pspec);
    }
}

impl EditorWindow {
    fn on_get_child_position(&self, widget: &gtk4::Widget) -> Option<Allocation> {
        if widget.is::<gtk4::DrawingArea>() {
            return Some(Allocation::new(0, 0, widget.width(), widget.height()));
        }

        let (width, height) = self.with_image("get-child-position", |image| {
            (image.surface.width(), image.surface.height())
        })?;

        Some(Allocation::new(
            width / 2 - widget.width() / 2,
            height / 5,
            widget.width(),
            widget.height().max(32),
        ))
    }
}

// Event controllers
#[gtk4::template_callbacks]
impl EditorWindow {
    #[track_caller]
    fn toolbar(&self) -> &toolbar::ToolbarWidget {
        self.toolbar.get().unwrap()
    }

    #[template_callback]
    fn on_mouse_button_pressed(&self, _: i32, x: f64, y: f64, click: &gtk4::GestureClick) {
        if click.current_button() == BUTTON_PRIMARY {
            if let Some(colour_tx) = self.colour_tx.take() {
                // if colour_tx is non-None it means there is a colour dialog open, and the user
                // is trying to pick a colour at the moment!
                self.with_image("colour picker", |image| {
                    let colour = image.get_colour_at(x, y);
                    if let Err(why) = colour_tx.send(colour) {
                        tracing::error!("Failed to send colour through colour_tx: {why}");
                    }
                });
            } else {
                assert!(
                    self.colour_tx.take().is_none(),
                    "There should be no colour_tx on the EditorWindow when we're not picking a colour"
                );

                self.with_image_mut("primary button pressed", |image| {
                    image.operation_stack.start_operation_at(Point { x, y });
                });
            }
        } else if click.current_button() == BUTTON_SECONDARY {
            self.obj().close();
        }
    }

    #[template_callback]
    fn on_mouse_motion(&self, x: f64, y: f64, _: &gtk4::EventControllerMotion) {
        self.with_image_mut("motion event", |image| {
            image.operation_stack.set_current_window(x, y);
            self.drawing_area.queue_draw();
        });
    }

    #[template_callback]
    fn on_mouse_button_released(&self, _: i32, x: f64, y: f64, _: &gtk4::GestureClick) {
        let should_queue_draw = self.with_image_mut("mouse button released event", |image| {
            // NOTE: image.operation_stack.finish_current_operation MUST be called in all
            //       branches of this if-chain, in order for tools to take part in the undo
            //       stack! For the Text tool, this happens in pop_text_dialog_and_get_text.
            if image.operation_stack.current_tool() == Tool::Text {
                super::textdialog::pop_text_dialog_and_get_text(&self.obj());
                true
            } else if !image.operation_stack.current_tool().is_saving_tool() {
                image.operation_stack.finish_current_operation();
                true
            } else {
                image.operation_stack.finish_current_operation();

                KCShot::the().with_conn(|conn| {
                    Self::do_save_surface(
                        &KCShot::the().model_notifier(),
                        conn,
                        self.obj().upcast_ref(),
                        image,
                        Some(Point { x, y }),
                    );
                });
                false
            }
        });

        if should_queue_draw.unwrap_or(true) {
            self.drawing_area.queue_draw();
        }
    }

    #[template_callback]
    fn on_drag_update(&self, x: f64, y: f64, _: &gtk4::GestureDrag) {
        self.with_image_mut("drag update event", |image| {
            image
                .operation_stack
                .update_current_operation_end_coordinate(x, y);
            if image.operation_stack.current_tool().is_cropping_tool() {
                image.operation_stack.set_is_in_crop_drag(true);
            }
            self.drawing_area.queue_draw();
        });
    }

    #[template_callback]
    fn on_drag_end(&self, x: f64, y: f64, _: &gtk4::GestureDrag) {
        self.with_image_mut("drag end event", |image| {
            image
                .operation_stack
                .update_current_operation_end_coordinate(x, y);
            if image.operation_stack.current_tool() == Tool::Crop {
                self.toolbar().set_visible(true);
                image.operation_stack.finish_current_operation();
                image.operation_stack.set_current_tool(Tool::Pencil);
            }
            self.drawing_area.queue_draw();
        });
    }

    #[template_callback]
    fn on_key_pressed(
        &self,
        key: gdk::Key,
        _: u32,
        _: gdk::ModifierType,
        _: &gtk4::EventControllerKey,
    ) -> bool {
        let handled = self
            .with_image_mut("key pressed event", |image| {
                if key == gdk::Key::Control_L || key == gdk::Key::Control_R {
                    image.operation_stack.set_ignore_windows(true);
                    self.drawing_area.queue_draw();
                    return true;
                } else if key == gdk::Key::Return {
                    if !self.editing_started_with_cropping.get() {
                        // Saving a screenshot using `Return` only makes sense in "crop-first"
                        // mode
                        return false;
                    }

                    KCShot::the().with_conn(|conn| {
                        Self::do_save_surface(
                            &KCShot::the().model_notifier(),
                            conn,
                            self.obj().upcast_ref(),
                            image,
                            None,
                        );
                    });

                    return true;
                } else if key == gdk::Key::Shift_L || key == gdk::Key::Shift_R {
                    image.operation_stack.selection_mode = SelectionMode::WindowsWithoutDecorations;
                    return true;
                }

                false
            })
            .unwrap_or(false);

        if handled {
            return true;
        }

        self.toolbar().key_activates_tool(key)
    }

    #[template_callback]
    fn on_key_released(
        &self,
        key: gdk::Key,
        _: u32,
        _: gdk::ModifierType,
        _: &gtk4::EventControllerKey,
    ) {
        self.with_image_mut("key released event", |image| {
            if key == gdk::Key::Control_L || key == gdk::Key::Control_R {
                image.operation_stack.set_ignore_windows(false);
                self.drawing_area.queue_draw();
            } else if key == gdk::Key::Escape {
                self.obj().close();
            } else if key == gdk::Key::Shift_L || key == gdk::Key::Shift_R {
                image.operation_stack.selection_mode = SelectionMode::WindowsWithDecorations;
            }
        });
    }
}

// Actions
impl EditorWindow {
    fn setup_actions(&self) {
        let obj = self.obj();

        let undo_action = gio::SimpleAction::new("undo", None);
        undo_action.connect_activate(clone!(@weak obj => move |_, _| {
            obj.imp().with_image_mut("win.undo activated", |image| {
                image.operation_stack.undo();
                obj.imp().drawing_area.queue_draw();
            });
        }));
        obj.add_action(&undo_action);

        let redo_action = gio::SimpleAction::new("redo", None);
        redo_action.connect_activate(clone!(@weak obj => move |_, _| {
            obj.imp().with_image_mut("win.redo activated", |image| {
                image.operation_stack.redo();
                obj.imp().drawing_area.queue_draw();
            });
        }));
        obj.add_action(&redo_action);
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

impl WidgetImpl for EditorWindow {}
impl WindowImpl for EditorWindow {}
impl ApplicationWindowImpl for EditorWindow {}

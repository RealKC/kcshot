use std::{
    backtrace::Backtrace,
    cell::{Cell, OnceCell, RefCell},
};

use cairo::Context;
use diesel::SqliteConnection;
use gtk4::{
    Allocation, CompositeTemplate,
    gdk::{self, BUTTON_PRIMARY, BUTTON_SECONDARY},
    gio,
    glib::{self, Propagation, Properties, clone},
    prelude::*,
    subclass::prelude::*,
};
use kcshot_data::geometry::{Point, Rectangle};
use tokio::sync::mpsc::{self, Receiver, Sender};
use tracing::error;

use super::{Colour, textdialog::TextDialog, toolbar, utils::ContextLogger};
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

#[derive(Debug, Properties, CompositeTemplate)]
#[properties(wrapper_type = super::EditorWindow)]
#[template(file = "src/editor/editor.blp")]
pub struct EditorWindow {
    #[property(name = "editing-starts-with-cropping", set, construct_only)]
    editing_started_with_cropping: Cell<bool>,

    pub(super) image: RefCell<Option<Image>>,

    #[template_child]
    overlay: TemplateChild<gtk4::Overlay>,
    #[template_child]
    drawing_area: TemplateChild<gtk4::DrawingArea>,

    toolbar: OnceCell<toolbar::ToolbarWidget>,

    colour_tx: Sender<Colour>,
    colour_rx: RefCell<Receiver<Colour>>,
    colour_requested: Cell<bool>,

    is_in_with_image_mut: Cell<bool>,
}

impl Default for EditorWindow {
    fn default() -> Self {
        let (colour_tx, colour_rx) = mpsc::channel(8);

        Self {
            editing_started_with_cropping: Default::default(),
            image: Default::default(),
            overlay: Default::default(),
            drawing_area: Default::default(),
            toolbar: Default::default(),
            colour_tx,
            colour_rx: RefCell::new(colour_rx),
            colour_requested: Default::default(),
            is_in_with_image_mut: Default::default(),
        }
    }
}

#[glib::object_subclass]
impl ObjectSubclass for EditorWindow {
    const NAME: &'static str = "KCShotEditorWindow";
    type Type = super::EditorWindow;
    type ParentType = gtk4::ApplicationWindow;

    fn class_init(klass: &mut Self::Class) {
        klass.set_css_name("kcshot-editor-window");

        klass.bind_template();
        klass.bind_template_callbacks();
    }

    fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
        obj.init_template();
    }
}

#[glib::derived_properties]
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

        self.drawing_area.set_draw_func(clone!(
            #[weak]
            obj,
            move |_, cairo, _, _| {
                obj.imp().with_image("draw event", |image| {
                    EditorWindow::do_draw(image, cairo, true);
                });
            }
        ));

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
    async fn on_mouse_button_pressed(&self, _: i32, x: f64, y: f64, click: &gtk4::GestureClick) {
        if click.current_button() == BUTTON_PRIMARY {
            if self.colour_requested.get() {
                let colour = self.with_image("colour picker", |image| image.get_colour_at(x, y));
                self.colour_requested.set(false);
                self.colour_tx.send(colour.unwrap()).await.unwrap();
            } else {
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
                let dialog = TextDialog::new(&self.obj());
                dialog.set_transient_for(Some(&*self.obj()));
                dialog.show();
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
    ) -> Propagation {
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
            return Propagation::Stop;
        }

        if self.toolbar().key_activates_tool(key) {
            Propagation::Stop
        } else {
            Propagation::Proceed
        }
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
        undo_action.connect_activate(clone!(
            #[weak]
            obj,
            move |_, _| {
                obj.imp().with_image_mut("win.undo activated", |image| {
                    image.operation_stack.undo();
                    obj.imp().drawing_area.queue_draw();
                });
            }
        ));
        obj.add_action(&undo_action);

        let redo_action = gio::SimpleAction::new("redo", None);
        redo_action.connect_activate(clone!(
            #[weak]
            obj,
            move |_, _| {
                obj.imp().with_image_mut("win.redo activated", |image| {
                    image.operation_stack.redo();
                    obj.imp().drawing_area.queue_draw();
                });
            }
        ));
        obj.add_action(&redo_action);
    }
}

impl EditorWindow {
    #[allow(clippy::await_holding_refcell_ref)]
    pub(super) async fn pick_colour(&self) -> Colour {
        self.colour_requested.set(true);
        self.colour_rx.borrow_mut().recv().await.unwrap()
    }

    fn do_draw(image: &Image, cairo: &Context, is_in_draw_event: bool) {
        cairo.set_operator(cairo::Operator::Source);
        log_if_err!(cairo.set_source_surface(&image.surface, 0f64, 0f64));
        log_if_err!(cairo.paint());
        cairo.set_operator(cairo::Operator::Over);

        image.operation_stack.execute(cairo, is_in_draw_event);
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

        Self::do_draw(image, &cairo, false);

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

        if self.is_in_with_image_mut.get() {
            tracing::error!(
                "with_image called inside with_image_mut, likely a bug...\n{}",
                Backtrace::capture()
            );
            return None;
        }

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

        if self.is_in_with_image_mut.get() {
            tracing::error!(
                "with_image_mut called inside with_image_mut, likely a bug...\n{}",
                Backtrace::capture()
            );
            return None;
        }

        self.is_in_with_image_mut.set(true);

        match self.image.try_borrow_mut() {
            Ok(mut image) => {
                if let Some(image) = image.as_mut() {
                    self.is_in_with_image_mut.set(false);
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
        self.is_in_with_image_mut.set(false);

        None
    }
}

impl WidgetImpl for EditorWindow {}
impl WindowImpl for EditorWindow {}
impl ApplicationWindowImpl for EditorWindow {}

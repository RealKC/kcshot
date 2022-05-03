use std::cell::RefCell;

use cairo::Context;
use diesel::SqliteConnection;
use gtk4::{
    gdk::{self, BUTTON_PRIMARY, BUTTON_SECONDARY},
    gio,
    glib::{self, clone, ParamSpec, ParamSpecObject},
    prelude::*,
    subclass::prelude::*,
    Allocation,
};
use once_cell::sync::Lazy;
use tracing::{error, info};

use super::toolbar;
use crate::{
    editor::{
        data::{Point, Rectangle},
        display_server,
        operations::{OperationStack, Tool},
        textdialog::DialogResponse,
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

#[derive(Default, Debug)]
pub struct EditorWindow {
    pub(super) image: RefCell<Option<Image>>,
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

    fn do_save_surface(
        model_notifier: &ModelNotifier,
        conn: &SqliteConnection,
        window: &gtk4::Window,
        image: &Image,
        point: Point,
    ) {
        let cairo = match Context::new(&image.surface) {
            Ok(cairo) => cairo,
            Err(err) => {
                error!("Got error constructing cairo context inside button press event: {err}");
                return;
            }
        };
        EditorWindow::do_draw(image, &cairo, false);

        let rectangle = image
            .operation_stack
            .crop_region(point)
            .unwrap_or_else(|| display_server::get_screen_resolution(window));

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
}

#[glib::object_subclass]
impl ObjectSubclass for EditorWindow {
    const NAME: &'static str = "EditorWindow";
    type Type = super::EditorWindow;
    type ParentType = gtk4::ApplicationWindow;
}

impl ObjectImpl for EditorWindow {
    fn constructed(&self, obj: &Self::Type) {
        self.parent_constructed(obj);
        let app = obj.application().unwrap().downcast::<KCShot>().unwrap();
        let image =
            super::display_server::take_screenshot(&app).expect("Couldn't take a screenshot");
        let windows = display_server::get_windows().unwrap_or_else(|why| {
            tracing::info!("Got while trying to retrieve windows: {why}");
            vec![]
        });

        let overlay = gtk4::Overlay::new();
        obj.set_child(Some(&overlay));
        let drawing_area = gtk4::DrawingArea::builder().can_focus(true).build();

        overlay.set_child(Some(&drawing_area));
        overlay.add_overlay(&toolbar::ToolbarWidget::new(obj));

        overlay.connect_get_child_position(clone!(@weak app => @default-return Some(gdk::Rectangle::new(0, 0, 1920, 1080)), move|_this, widget| {
            let Rectangle { w: screen_width, h: screen_height, .. } = display_server::get_screen_resolution(app.main_window().upcast_ref());
            Some(Allocation::new(
                (screen_width / 2.0 - widget.width() as f64 / 2.0) as i32,
                (screen_height / 5.0) as i32,
                11 * 32,
                32,
            ))
        }));

        drawing_area.set_draw_func(clone!(@weak obj => move |_widget, cairo, _w, _h| {
            let imp = obj.imp();
            let image = imp.image.borrow();
            let image = image.as_ref().unwrap();
            EditorWindow::do_draw(image, cairo, true);
        }));

        let click_event_handler = gtk4::GestureClick::new();

        click_event_handler.set_button(0);
        click_event_handler.connect_pressed(clone!(@weak obj =>  move |this, _n_clicks, x, y| {
            tracing::warn!("Got button-press on drawing_area");
            if this.current_button() == BUTTON_PRIMARY {
                let imp = obj.imp();
                let mut image = imp.image.borrow_mut();
                let image = image.as_mut().unwrap();
                image.operation_stack.start_operation_at(Point { x, y });
            } else if this.current_button() == BUTTON_SECONDARY {
                obj.close();
            }

        }));

        let motion_event_handler = gtk4::EventControllerMotion::new();
        motion_event_handler.connect_motion(
            clone!(@weak obj, @weak drawing_area => move |_this, x, y| {
                let imp = obj.imp();
                let image = &imp.image;
                match image.try_borrow_mut() {
                    Ok(mut image) => {
                        let image = image.as_mut().unwrap();
                        image.operation_stack.set_current_window(x, y);
                        drawing_area.queue_draw();
                    }
                    Err(why) => info!("Image already borrowed: {why}"),
                };
            }),
        );
        drawing_area.add_controller(&motion_event_handler);

        click_event_handler.connect_released(
            clone!(@weak obj, @weak drawing_area, @weak app => move |_this, _n_clicks, x, y| {
                let imp = obj.imp();
                let image = &imp.image;
                let mut imagerc = image.borrow_mut();
                let image = imagerc.as_mut().unwrap();
                if image.operation_stack.current_tool() == Tool::Text {
                    tracing::info!("Text tool has been activated");
                    let res = super::textdialog::pop_text_dialog_and_get_text(obj.upcast_ref());
                    match res {
                        DialogResponse::Text(text) => {
                            image.operation_stack.set_text(text);
                            drawing_area.queue_draw();
                        }
                        DialogResponse::Cancel => { /* do nothing */ }
                    }
                    return;
                } else if image.operation_stack.current_tool() != Tool::CropAndSave {
                    tracing::info!("This is called");
                    image.operation_stack.finish_current_operation();
                    drawing_area.queue_draw();
                    return;
                }

                EditorWindow::do_save_surface(&app.model_notifier(), app.conn(), obj.upcast_ref(), image, Point { x, y });
            }),
        );

        drawing_area.add_controller(&click_event_handler);

        let drag_controller = gtk4::GestureDrag::new();
        drag_controller.connect_drag_update(
            clone!(@weak obj, @weak drawing_area =>  move |_this, x, y| {
                let imp = obj.imp();
                let image = &imp.image;
                let mut image = image.borrow_mut();
                let image = image.as_mut().unwrap();
                info!("Dragging to {{ {x}, {y} }}");
                image.operation_stack.update_current_operation_end_coordinate(x, y);
                if image.operation_stack.current_tool() == Tool::CropAndSave {
                    image.operation_stack.set_is_in_crop_drag(true);
                }
                drawing_area.queue_draw();
            }),
        );
        drawing_area.add_controller(&drag_controller);

        let undo_action = gio::SimpleAction::new("undo", None);
        undo_action.connect_activate(
            clone!(@weak obj, @weak drawing_area => move |_, _| {
                let imp = obj.imp();
                let image = &imp.image;
                match image.try_borrow_mut() {
                    Ok(mut image) => {
                        let image = image.as_mut().unwrap();
                        image.operation_stack.undo();
                        drawing_area.queue_draw();
                    }
                    Err(why) => tracing::error!("Failed to borrow self.image when trying to handle undo: {why}")
                };
            }),
        );
        obj.add_action(&undo_action);

        let redo_action = gio::SimpleAction::new("redo", None);
        redo_action.connect_activate(
            clone!(@weak obj, @weak drawing_area => move |_, _| {
                let imp = obj.imp();
                let image = &imp.image;
                match image.try_borrow_mut() {
                    Ok(mut image) => {
                        let image = image.as_mut().unwrap();
                        image.operation_stack.redo();
                        drawing_area.queue_draw();
                    }
                    Err(why) => tracing::error!("Failed to borrow self.image when trying to handle redo: {why}")
                };
            }),
        );
        obj.add_action(&redo_action);

        // FIXME: Figure out how/if we make this work across keyboard layouts that don't have Z and Y
        // in the same place QWERTY does.
        app.set_accels_for_action("win.undo", &["<Ctrl>Z"]);
        app.set_accels_for_action("win.redo", &["<Ctrl>Y"]);

        self.image.replace(Some(Image {
            surface: image,
            operation_stack: OperationStack::new(windows),
        }));
    }

    fn properties() -> &'static [ParamSpec] {
        static PROPERTIES: Lazy<Vec<ParamSpec>> = Lazy::new(|| {
            vec![ParamSpecObject::new(
                "application",
                "Application",
                "Application",
                KCShot::static_type(),
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
                tracing::error!("Unknown property: {name}");
                panic!()
            }
        }
    }

    #[tracing::instrument]
    fn set_property(&self, obj: &Self::Type, _id: usize, value: &glib::Value, pspec: &ParamSpec) {
        match pspec.name() {
            "application" => {
                let application = value.get::<KCShot>().ok();
                obj.set_application(application.as_ref());
            }
            name => tracing::warn!("Unknown property: {name}"),
        }
    }
}

impl WidgetImpl for EditorWindow {}
impl WindowImpl for EditorWindow {}
impl ApplicationWindowImpl for EditorWindow {}

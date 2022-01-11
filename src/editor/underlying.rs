use std::{cell::RefCell, convert::TryFrom, rc::Rc};

use cairo::Context;
use diesel::SqliteConnection;
use gtk4::{
    ffi::GTK_INVALID_LIST_POSITION,
    gdk::{self, Key, BUTTON_PRIMARY},
    glib::{self, clone, signal::Inhibit, ParamSpec, ParamSpecObject},
    prelude::*,
    subclass::prelude::*,
    Allocation, ResponseType,
};
use once_cell::sync::Lazy;
use tracing::{error, info};

use crate::{
    editor::{
        data::{Colour, Point, Rectangle},
        display_server,
        operations::{OperationStack, SelectionMode, Tool},
        textdialog::DialogResponse,
        utils::{self, CairoExt},
    },
    historymodel::ModelNotifier,
    kcshot::KCShot,
    log_if_err, postcapture,
};

#[derive(Debug)]
struct Image {
    surface: cairo::ImageSurface,
    operation_stack: OperationStack,
}

type ImageRef = Rc<RefCell<Option<Image>>>;

#[derive(Default, Debug)]
pub struct EditorWindow {
    image: ImageRef,
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
            // TODO: Give the user the option which actions to run and in which order.
            Some(mut pixbuf) => {
                for action in postcapture::get_postcapture_actions() {
                    action.handle(model_notifier.clone(), conn, &mut pixbuf)
                }
            },
            None => {
                error!(
                    "Failed to create a pixbuf from the surface: {:?} with crop region {rectangle:#?}",
                    image.surface
                );
            }
        };
    }

    fn make_primary_colour_chooser_button(
        image: ImageRef,
        parent_window: &gtk4::Window,
    ) -> gtk4::Button {
        let drawing_area = gtk4::DrawingArea::new();
        drawing_area.set_size_request(32, 32);
        drawing_area.set_draw_func(clone!(@strong image =>  move |_this, cairo, _w, _h| {
            let image = match image.try_borrow() {
                Ok(image) => image,
                Err(why) => {
                    info!("image already borrowed: {why}");
                    return;
                }
            };
            let image = image.as_ref().unwrap();

            cairo.set_operator(cairo::Operator::Over);

            if image.operation_stack.primary_colour.alpha != 0 {
                cairo.rectangle(0.0, 0.0, 32.0, 32.0);
                cairo.set_source_colour(image.operation_stack.primary_colour);
                log_if_err!(cairo.fill());
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
                log_if_err!(cairo.fill());
                cairo.rectangle(16.0, 16.0, 16.0, 16.0);
                log_if_err!(cairo.fill());

                cairo.set_source_colour(Colour::BLACK);
                cairo.rectangle(0.0, 16.0, 16.0, 16.0);
                log_if_err!(cairo.fill());
                cairo.rectangle(16.0, 0.0, 16.0, 16.0);
                log_if_err!(cairo.fill());
            }

            cairo.set_source_colour(Colour::BLACK);
            cairo.rectangle(1.0, 1.0, 30.0, 30.0);
            cairo.set_line_width(1.0);
            log_if_err!(cairo.stroke());

        }));

        Self::make_button::<true>(&drawing_area, parent_window, image)
    }

    fn make_secondary_colour_button(image: ImageRef, parent_window: &gtk4::Window) -> gtk4::Button {
        let drawing_area = gtk4::DrawingArea::new();
        drawing_area.set_size_request(32, 32);
        drawing_area.set_draw_func(clone!(@strong image =>  move |_this, cairo, _w, _h| {
            let image = match image.try_borrow() {
                Ok(image) => image,
                Err(why) => {
                    info!("image already borrowed: {why}");
                    return;
                }
            };
            let image = image.as_ref().unwrap();

            cairo.set_operator(cairo::Operator::Over);

            cairo.set_source_colour(Colour::BLACK);
            cairo.rectangle(11.0, 11.0, 10.0, 10.0);
            cairo.set_line_width(1.0);
            log_if_err!(cairo.stroke());

            cairo.set_source_colour(image.operation_stack.secondary_colour);
            cairo.rectangle(8.0, 8.0, 16.0, 16.0);
            cairo.set_line_width(6.0);
            log_if_err!(cairo.stroke());

            cairo.set_source_colour(Colour::BLACK);
            cairo.rectangle(4.0, 4.0, 24.0, 24.0);
            cairo.set_line_width(1.0);
            log_if_err!(cairo.stroke());

        }));

        Self::make_button::<false>(&drawing_area, parent_window, image)
    }

    fn make_button<const IS_PRIMARY: bool>(
        drawing_area: &gtk4::DrawingArea,
        parent_window: &gtk4::Window,
        image: ImageRef,
    ) -> gtk4::Button {
        let button = gtk4::Button::new();
        button.set_child(Some(drawing_area));

        button.connect_clicked(clone!(@strong parent_window, @strong image, @strong drawing_area => move |_this| {
            let colour_chooser = gtk4::ColorChooserDialog::new(Some("Pick a colour"), Some(&parent_window));

            colour_chooser.connect_response(clone!(@strong image, @strong drawing_area => move |this, response| {
                if response == ResponseType::Ok {
                    let mut image = image.borrow_mut();
                    let image = image.as_mut().unwrap();
                    if IS_PRIMARY {
                        image.operation_stack.primary_colour = Colour::from_gdk_rgba(this.rgba());
                    } else {
                        image.operation_stack.secondary_colour = Colour::from_gdk_rgba(this.rgba());
                    }
                    drawing_area.queue_draw();
                }

                this.close();
            }));

            colour_chooser.show();
        }));

        button
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

        let toolbar = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);

        overlay.set_child(Some(&drawing_area));
        overlay.add_overlay(&toolbar);

        overlay.connect_get_child_position(clone!(@weak app => @default-return Some(gdk::Rectangle::new(0, 0, 1920, 1080)), move|_this, widget| {
            let Rectangle { w: screen_width, h: screen_height, .. } = display_server::get_screen_resolution(app.main_window().upcast_ref());
            Some(Allocation::new(
                (screen_width / 2.0 - widget.width() as f64 / 2.0) as i32,
                (screen_height / 5.0) as i32,
                11 * 32,
                32,
            ))
        }));

        drawing_area.set_draw_func(
            clone!(@strong self.image as image => move |_widget, cairo, _w, _h| {
                match image.try_borrow() {
                    Ok(image) => EditorWindow::do_draw(image.as_ref().unwrap(), cairo, true),
                    Err(why) => info!("Image already borrowed: {why}")
                }
            }),
        );

        let click_event_handler = gtk4::GestureClick::new();

        click_event_handler.set_button(BUTTON_PRIMARY);
        click_event_handler.connect_pressed(
            clone!(@strong self.image as image, @strong obj =>  move |_this, _n_clicks, x, y| {
                tracing::warn!("Got button-press on drawing_area");
                match image.try_borrow_mut() {
                    Ok(mut image) => {
                        let image = image.as_mut().unwrap();
                        image.operation_stack.start_operation_at(Point { x, y });
                        obj.queue_draw();
                    }
                    Err(why) => info!("Image already borrowed: {why}"),
                }

            }),
        );

        let motion_event_handler = gtk4::EventControllerMotion::new();
        motion_event_handler.connect_motion(
            clone!(@strong self.image as image, @strong drawing_area => move |_this, x, y| {
                match image.try_borrow_mut() {
                    Ok(mut image) => {
                        let image = image.as_mut().unwrap();
                        image.operation_stack.set_current_window(x, y);
                        drawing_area.queue_draw();
                    }
                    Err(why) => info!("Image already borrowed: {why}"),
                }
            }),
        );
        drawing_area.add_controller(&motion_event_handler);

        click_event_handler.connect_released(
            clone!(@strong self.image as image, @strong obj, @strong drawing_area, @weak app => move |_this, _n_clicks, x, y| {
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
            clone!(@strong self.image as image, @strong drawing_area =>  move |_this, x, y| {
                let mut image = image.borrow_mut();
                let image = image.as_mut().unwrap();
                info!("Dragging to {{ {x}, {y} }}");
                image.operation_stack.update_current_operation_end_coordinate(x, y);
                image.operation_stack.set_is_in_crop_drag(true);
                drawing_area.queue_draw();
            }),
        );
        drawing_area.add_controller(&drag_controller);

        self.image.replace(Some(Image {
            surface: image,
            operation_stack: OperationStack::new(windows),
        }));

        fn make_tool_button(
            tool: Tool,
            toolbar: &gtk4::Box,
            image: ImageRef,
            group_source: Option<&gtk4::ToggleButton>,
        ) -> (gtk4::ToggleButton, Tool) {
            let button = match group_source {
                Some(group_source) => {
                    let button = gtk4::ToggleButton::new();
                    button.set_group(Some(group_source));
                    button
                }
                None => gtk4::ToggleButton::new(),
            };
            button.set_child(Some(&gtk4::Image::from_resource(tool.path())));
            button.set_tooltip_markup(Some(tool.tooltip()));

            button.connect_clicked(clone!(@strong image => move |_| {
                info!("Entered on-click handler of {tool:?}");
                image.borrow_mut().as_mut().unwrap().operation_stack.set_current_tool(tool);
            }));
            button.set_active(false);
            toolbar.append(&button);
            (button, tool)
        }

        let (group_source, _) =
            make_tool_button(Tool::CropAndSave, &toolbar, self.image.clone(), None);
        group_source.set_active(true);

        // rustfmt make this section of code ugly, tell it to shutup
        #[rustfmt::skip]
        let mut buttons = vec!{
            make_tool_button(Tool::Line, &toolbar, self.image.clone(), Some(&group_source)),
            make_tool_button(Tool::Arrow, &toolbar, self.image.clone(), Some(&group_source)),
            make_tool_button(Tool::Rectangle, &toolbar, self.image.clone(), Some(&group_source)),
            make_tool_button(Tool::Highlight, &toolbar, self.image.clone(), Some(&group_source)),
            make_tool_button(Tool::Ellipse, &toolbar, self.image.clone(), Some(&group_source)),
            make_tool_button(Tool::Pixelate, &toolbar, self.image.clone(), Some(&group_source)),
            make_tool_button(Tool::Blur, &toolbar, self.image.clone(), Some(&group_source)),
            make_tool_button(Tool::AutoincrementBubble, &toolbar, self.image.clone(), Some(&group_source)),
            make_tool_button(Tool::Text, &toolbar, self.image.clone(), Some(&group_source)),
        };

        let primary_colour_button =
            EditorWindow::make_primary_colour_chooser_button(self.image.clone(), obj.upcast_ref());
        primary_colour_button.set_tooltip_text(Some("Set primary colour"));
        toolbar.append(&primary_colour_button);

        let secondary_colour_button =
            EditorWindow::make_secondary_colour_button(self.image.clone(), obj.upcast_ref());
        secondary_colour_button.set_tooltip_text(Some("Set secondary colour"));
        toolbar.append(&secondary_colour_button);

        let drop_down = gtk4::DropDown::from_strings(&[
            "Windows w/ decorations",
            "Windows w/o decorations",
            "Ignore windows",
        ]);
        drop_down.set_tooltip_text(Some("Selection mode"));
        drop_down.connect_selected_item_notify(clone!( @weak self.image as image => move |this| {
            if this.selected() != GTK_INVALID_LIST_POSITION {
                match image.try_borrow_mut(){
                    Ok(mut image) => {
                        let image = image.as_mut().unwrap();
                        if let Ok(selection_mode) = SelectionMode::try_from(this.selected()) {
                            image.operation_stack.selection_mode = selection_mode;
                        }
                    }
                    Err(why) => info!("Image already borrowed: {why}"),
                }
            }
        }));

        // Don't add this button when the WM doesn't support retrieving windows
        if display_server::can_retrieve_windows() {
            toolbar.append(&drop_down);
        }

        buttons.insert(0, (group_source, Tool::CropAndSave));

        let key_event_handler = gtk4::EventControllerKey::new();
        key_event_handler.connect_key_pressed(
            clone!(@strong obj, @weak self.image as image => @default-return Inhibit(false), move |_this, key, _, _| {
                if key == Key::Escape {
                    obj.hide();
                } else if let Some(tool) = key.to_unicode().map(Tool::from_unicode).flatten() {
                    match image.try_borrow_mut() {
                        Ok(mut image) => {
                            let image = image.as_mut().unwrap();
                            image.operation_stack.set_current_tool(tool);
                            for (button, button_tool) in buttons.iter() {
                                if *button_tool == tool {
                                    button.set_active(true);
                                    break;
                                }
                            }
                        }
                        Err(why) => info!("Image already borrowed: {why}"),
                    }
                }
                Inhibit(false)
            }),
        );
        obj.add_controller(&key_event_handler);
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

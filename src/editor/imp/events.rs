use gtk4::{
    Allocation,
    gdk::{self, BUTTON_PRIMARY, BUTTON_SECONDARY},
    glib::Propagation,
    graphene,
    prelude::*,
    subclass::prelude::*,
};
use kcshot_data::geometry::Point;
use kcshot_editor::operations::{SelectionMode, Tool};

use crate::{
    editor::{imp::EditorWindow, textdialog::TextDialog, toolbar},
    kcshot::KCShot,
};

// Manually registered event handlers
impl EditorWindow {
    pub fn on_get_child_position(&self, widget: &gtk4::Widget) -> Option<Allocation> {
        let preferred_size = widget.preferred_size();

        let (image_width, image_height) = self.with_image("get-child-position", |image| {
            (image.surface.width(), image.surface.height())
        })?;

        // Separate variable to avoid refcell-reborrow panics
        let toolbar_position = *self.toolbar_position.borrow();

        match (*self.toolbar_offset.borrow(), toolbar_position) {
            (None, None) => {
                let x = image_width / 2 - preferred_size.0.width() / 2;
                let y = image_height / 5;
                self.set_toolbar_position(x, y);

                Some(Allocation::new(
                    x,
                    y,
                    preferred_size.0.width(),
                    preferred_size.0.height(), //.max(32),
                ))
            }
            (None, Some((x, y))) => Some(Allocation::new(
                x,
                y,
                preferred_size.0.width(),
                preferred_size.0.height(), //.max(32),
            )),
            (Some((off_x, off_y)), _) => {
                let start = self.toolbar_position();

                Some(Allocation::new(
                    start.0 + off_x,
                    start.1 + off_y,
                    preferred_size.0.width(),
                    preferred_size.0.height(), //.max(32),
                ))
            }
        }
    }
}

// Event controller helpers
impl EditorWindow {
    #[track_caller]
    fn toolbar(&self) -> &toolbar::ToolbarWidget {
        self.toolbar.get().unwrap()
    }

    fn should_start_toolbar_move(&self, x: f64, y: f64) -> bool {
        let toolbar_point = self
            .obj()
            .compute_point(self.toolbar(), &graphene::Point::new(x as f32, y as f32));

        toolbar_point
            .is_some_and(|point| self.toolbar().contains(point.x() as f64, point.y() as f64))
    }
}

// Event controllers
#[gtk4::template_callbacks]
impl EditorWindow {
    #[template_callback]
    async fn on_mouse_button_pressed(&self, _: i32, x: f64, y: f64, click: &gtk4::GestureClick) {
        if click.current_button() == BUTTON_PRIMARY {
            if self.should_start_toolbar_move(x, y) {
                self.set_toolbar_offset(0, 0);
            } else if self.colour_requested.get() {
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
        if self.is_moving_toolbar() {
            self.finish_toolbar_move(x as i32, y as i32);
            return;
        }

        let should_queue_draw = self.with_image_mut("mouse button released event", |image| {
            // NOTE: image.operation_stack.finish_current_operation MUST be called in all
            //       branches of this if-chain, in order for tools to take part in the undo
            //       stack! For the Text tool, this happens in pop_text_dialog_and_get_text.
            if image.operation_stack.current_tool() == Tool::Text {
                let dialog = TextDialog::new(&self.obj());
                dialog.set_transient_for(Some(&*self.obj()));
                dialog.present();
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
        if self.is_moving_toolbar() {
            self.set_toolbar_offset(x as i32, y as i32);

            self.overlay.queue_allocate();
        } else {
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
    }

    #[template_callback]
    fn on_drag_end(&self, x: f64, y: f64, _: &gtk4::GestureDrag) {
        if self.is_moving_toolbar() {
            self.set_toolbar_offset(x as i32, y as i32);
        }

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

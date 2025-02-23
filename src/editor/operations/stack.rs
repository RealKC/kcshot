use cairo::Context;
use kcshot_data::{
    Text,
    colour::Colour,
    geometry::{Point, Rectangle},
};
use kcshot_screenshot::Window;
use tracing::{error, warn};

use super::{Operation, Tool};
use crate::{
    editor::{operations::shapes, utils::CairoExt},
    log_if_err,
};

#[derive(Debug)]
pub struct OperationStack {
    // The stack itself
    operations: Vec<Operation>,
    undone_operations: Vec<Operation>,

    // State relating to the operation going on right now
    current_tool: Tool,
    current_operation: Option<Operation>,
    autoincrement_bubble_number: i32,
    pub primary_colour: Colour,
    pub secondary_colour: Colour,
    pub line_width: f64,

    // State relating to crop selection
    pub selection_mode: SelectionMode,
    is_in_crop_drag: bool,
    /// This in in stacking order
    windows: Vec<Window>,
    current_window: Option<usize>,
    ignore_windows: bool,

    /// Used for arrows, lines, pencil and the contours of rectangles
    editing_started_with_cropping: bool,
    pub screen_dimensions: Rectangle,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum SelectionMode {
    WindowsWithDecorations,
    WindowsWithoutDecorations,
}

impl OperationStack {
    pub fn new(
        windows: Vec<Window>,
        screen_dimensions: Rectangle,
        editing_started_with_cropping: bool,
    ) -> Self {
        Self {
            operations: vec![],
            undone_operations: vec![],
            current_tool: if editing_started_with_cropping {
                Tool::Crop
            } else {
                Tool::CropAndSave
            },
            current_operation: None,
            autoincrement_bubble_number: 1,
            primary_colour: Colour {
                red: 127,
                green: 0,
                blue: 127,
                alpha: 255,
            },
            secondary_colour: Colour {
                red: 0,
                green: 127,
                blue: 127,
                alpha: 255,
            },
            windows,
            current_window: None,
            is_in_crop_drag: false,
            ignore_windows: false,
            selection_mode: SelectionMode::WindowsWithDecorations,
            line_width: 4.0,
            editing_started_with_cropping,
            screen_dimensions,
        }
    }

    pub fn set_current_tool(&mut self, tool: Tool) {
        self.current_tool = tool;
    }

    pub fn current_tool(&self) -> Tool {
        self.current_tool
    }

    pub fn set_current_window(&mut self, x: f64, y: f64) {
        if self.ignore_windows {
            self.current_window = None;
            return;
        }

        for (idx, window) in self.windows.iter().enumerate().rev() {
            if window.outer_rect.contains(Point { x, y }) {
                self.current_window = Some(idx);
                break;
            }
        }
    }

    pub fn set_ignore_windows(&mut self, b: bool) {
        self.ignore_windows = b;
    }

    pub fn start_operation_at(&mut self, point: Point) {
        self.undone_operations.clear();

        if let Some(old_operation) = self.current_operation.take() {
            self.operations.push(old_operation);
        }

        self.current_operation = Some(Operation::create_default_for_tool(
            self.current_tool,
            point,
            &mut self.autoincrement_bubble_number,
            self.primary_colour,
            self.secondary_colour,
            self.line_width,
        ));
    }

    pub fn undo(&mut self) {
        if self.operations.len() == 1 && matches!(self.operations[0], Operation::Crop(_)) {
            return;
        }

        if let Some(op) = self.operations.pop() {
            self.undone_operations.push(op);
        }
    }

    pub fn redo(&mut self) {
        if let Some(op) = self.undone_operations.pop() {
            self.operations.push(op);
        }
    }

    pub fn update_current_operation_end_coordinate(&mut self, new_width: f64, new_height: f64) {
        let Some(current_operation) = self.current_operation.as_mut() else {
            return;
        };

        match current_operation {
            Operation::Crop(rect)
            | Operation::Blur { rect, .. }
            | Operation::Pixelate { rect, .. }
            | Operation::DrawRectangle { rect, .. }
            | Operation::Highlight { rect } => {
                rect.w = new_width;
                rect.h = new_height;
            }
            Operation::DrawLine { start, end, .. } | Operation::DrawArrow { start, end, .. } => {
                *end = Point {
                    x: start.x + new_width,
                    y: start.y + new_height,
                }
            }
            Operation::DrawEllipse { ellipse, .. } => {
                ellipse.w = new_width;
                ellipse.h = new_height;
            }
            Operation::Pencil {
                start: Point { x, y },
                points,
                ..
            } => points.push(Point {
                x: new_width + *x,
                y: new_height + *y,
            }),
            Operation::Bubble { .. } | Operation::Text { .. } => {}
        }
    }

    pub fn set_text(&mut self, text: Text) {
        if self.current_tool != Tool::Text {
            warn!(
                "Trying to set text when self.current_tool={:?}",
                self.current_tool
            );
            return;
        }
        self.current_operation
            .as_mut()
            .expect("A current operation should exist if we reach this")
            .set_text(text);
    }

    pub fn set_is_in_crop_drag(&mut self, is_in_crop_drag: bool) {
        self.is_in_crop_drag = is_in_crop_drag;
    }

    pub fn finish_current_operation(&mut self) {
        if let Some(mut operation) = self.current_operation.take() {
            if self.current_tool == Tool::Crop {
                self.ignore_windows = true;
                if let Operation::Crop(rect) = operation {
                    if should_crop_selected_window_or_screen(rect) {
                        if let Some(current_window) = self.current_window {
                            // FIXME: We should allow selecting the content rect somehow
                            operation = Operation::Crop(self.windows[current_window].outer_rect);
                        }
                    }
                }
            }

            self.operations.push(operation);
        }
    }

    pub fn crop_region(&self, point: Option<Point>) -> Option<Rectangle> {
        // We do this in order to support both "crop-first" and "crop-last" modes
        let crop_rect = match self.operations.last() {
            Some(Operation::Crop(rect)) => Some(rect),
            _ => match self.operations.first() {
                Some(Operation::Crop(rect)) => Some(rect),
                _ => None,
            },
        };

        if let Some(rect) = crop_rect {
            if should_crop_selected_window_or_screen(*rect) {
                if !self.ignore_windows {
                    let point =
                        point.expect("If we're trying to select windows, the point should be set, as it only makes sense for it to be unset when saving in crop-first mode, where save doesn't involve clicking anywhere");

                    self.windows
                        .iter()
                        .rev()
                        .find(|window| window.outer_rect.contains(point))
                        .map(|window| match self.selection_mode {
                            SelectionMode::WindowsWithDecorations => window.outer_rect,
                            SelectionMode::WindowsWithoutDecorations => window.content_rect,
                        })
                } else {
                    None
                }
            } else {
                Some(*rect)
            }
        } else {
            None
        }
    }

    pub fn execute(&self, cairo: &Context, is_in_draw_event: bool) {
        for operation in self.operations.iter() {
            if let Err(why) =
                operation.execute(cairo, is_in_draw_event, !self.editing_started_with_cropping)
            {
                error!("Got error trying to execute {operation:?}: {why}");
            }
        }

        if let Some(operation) = &self.current_operation {
            if let Err(why) =
                operation.execute(cairo, is_in_draw_event, !self.editing_started_with_cropping)
            {
                error!("Got error trying to execute {operation:?}: {why}");
            }
        }

        // We only want to draw window "crop indicators" when:
        //  * we're not saving the screenshot
        //  * the user's tool is the CropAndSave tool
        //  * we are not in a crop drag
        //  * we are not in "ignore windows" mode (entered by holding Ctrl)
        let should_draw_windows = is_in_draw_event
            && self.current_tool().is_cropping_tool()
            && !self.is_in_crop_drag
            && !self.ignore_windows;

        // We only want to dim around the "manual selection"/whole screen if
        //  * we won't be drawing windows (they have their own dimming logic)
        //  * editing started with cropping (we don't want to dim in the crop last mode)
        let should_dim_manual_selection_or_whole_screen =
            (!should_draw_windows || self.windows.is_empty()) && self.editing_started_with_cropping;

        if should_dim_manual_selection_or_whole_screen {
            self.dimmen_manual_selection_or_whole_screen(cairo);
        }

        if should_draw_windows {
            if let Some(idx) = self.current_window {
                let Rectangle { x, y, w, h } = match self.selection_mode {
                    SelectionMode::WindowsWithDecorations => self.windows[idx].outer_rect,
                    SelectionMode::WindowsWithoutDecorations => self.windows[idx].content_rect,
                };
                log_if_err!(cairo.save());

                cairo.rectangle(x, y, w, h);
                // When we are in draw events (aka this is being shown to the user), we want to make it clear
                // they are selecting the region which will be cropped
                cairo.set_source_colour(Colour {
                    red: 0,
                    green: 127,
                    blue: 190,
                    alpha: 255,
                });

                if !self.editing_started_with_cropping {
                    cairo.set_dash(&[4.0, 21.0, 4.0], 0.0);
                }

                log_if_err!(cairo.stroke());

                if self.editing_started_with_cropping {
                    shapes::dimmen_rectangle_around(
                        cairo,
                        self.screen_dimensions,
                        Rectangle { x, y, w, h },
                    );
                    log_if_err!(cairo.fill());
                }

                log_if_err!(cairo.restore());
            }
        }
    }

    fn dimmen_manual_selection_or_whole_screen(&self, cairo: &Context) {
        if let Some(Operation::Crop(rect)) = self.current_operation {
            shapes::dimmen_rectangle_around(cairo, self.screen_dimensions, rect.normalised());
            log_if_err!(cairo.fill());
        } else if let Some(&Operation::Crop(rect)) = self.operations.first() {
            shapes::dimmen_rectangle_around(cairo, self.screen_dimensions, rect.normalised());
            log_if_err!(cairo.fill());
        } else if self.operations.is_empty() {
            cairo.set_source_colour(Colour {
                red: 0,
                green: 0,
                blue: 0,
                alpha: 128,
            });
            cairo.rectangle(
                self.screen_dimensions.x,
                self.screen_dimensions.y,
                self.screen_dimensions.w,
                self.screen_dimensions.h,
            );
            log_if_err!(cairo.fill());
        }
    }
}

/// If the width or height of the rectangle are 0, or the area of the rectangle covers
/// less than a pixel, we consider the entire screen or window under the cursor to be
/// the crop region
fn should_crop_selected_window_or_screen(rect: Rectangle) -> bool {
    rect.area() < 1.0
}

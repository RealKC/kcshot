use cairo::{Context, ImageSurface};
use tracing::{error, warn};

use super::{Colour, Operation, Point, Rectangle, Tool};
use crate::{
    editor::{data::Text, display_server::Window, utils::CairoExt},
    log_if_err,
};

#[derive(Debug)]
pub struct OperationStack {
    operations: Vec<Operation>,
    undone_operations: Vec<Operation>,
    current_tool: Tool,
    current_operation: Option<Operation>,
    autoincrement_bubble_number: i32,
    pub primary_colour: Colour,
    pub secondary_colour: Colour,
    /// This in in stacking order
    windows: Vec<Window>,
    current_window: Option<usize>,
    is_in_crop_drag: bool,
    pub selection_mode: SelectionMode,
    /// Used for arrows, lines, pencil and the contours of rectangles
    pub line_width: f64,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum SelectionMode {
    WindowsWithDecorations,
    WindowsWithoutDecorations,
    IgnoreWindows,
}

impl SelectionMode {
    /// Contains strings representing the variants of [`Self`] with mentions of window decorations.
    pub const DECORATIONS: &'static [&'static str] = &[
        "Windows w/ decorations",
        "Windows w/o decorations",
        "Ignore windows",
    ];

    /// Contains string representing the variants of [`Self`] that make sense when we can't retrieve
    /// window decorations.
    pub const NO_DECORATIONS: &'static [&'static str] = &["Windows", "Ignore windows"];

    pub fn from_integer(value: u32, can_retrieve_window_decorations: bool) -> Option<Self> {
        use SelectionMode::*;
        if can_retrieve_window_decorations {
            match value {
                0 => Some(WindowsWithDecorations),
                1 => Some(WindowsWithoutDecorations),
                2 => Some(IgnoreWindows),
                _ => None,
            }
        } else {
            match value {
                0 => Some(WindowsWithoutDecorations),
                1 => Some(IgnoreWindows),
                _ => None,
            }
        }
    }
}

impl OperationStack {
    pub fn new(windows: Vec<Window>) -> Self {
        Self {
            operations: vec![],
            undone_operations: vec![],
            current_tool: Tool::CropAndSave,
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
            selection_mode: SelectionMode::WindowsWithDecorations,
            line_width: 4.0,
        }
    }

    pub fn set_current_tool(&mut self, tool: Tool) {
        self.current_tool = tool;
    }

    pub fn current_tool(&self) -> Tool {
        self.current_tool
    }

    pub fn set_current_window(&mut self, x: f64, y: f64) {
        if self.selection_mode == SelectionMode::IgnoreWindows {
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
        let current_operation = match self.current_operation.as_mut() {
            Some(curr) => curr,
            None => return,
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
        if let Some(operation) = self.current_operation.take() {
            self.operations.push(operation);
        }
    }

    pub fn crop_region(&self, point: Point) -> Option<Rectangle> {
        // We do not look at the top of `self.operations` as cropping should be the last operation
        // in the UX I want.
        if let Some(Operation::Crop(rect)) = &self.current_operation {
            // If the width or height of the rectangle are 0, or the area of the rectangle covers
            // less than a pixel, we consider the entire screen or window under the cursor to be
            // the crop region
            if rect.area() < 1.0 {
                if self.selection_mode != SelectionMode::IgnoreWindows {
                    self.windows
                        .iter()
                        .rev()
                        .find(|window| window.outer_rect.contains(point))
                        .map(|window| match self.selection_mode {
                            SelectionMode::WindowsWithDecorations => window.outer_rect,
                            SelectionMode::WindowsWithoutDecorations => window.content_rect,
                            _ => unreachable!(),
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

    pub fn execute(&self, surface: &ImageSurface, cairo: &Context, is_in_draw_event: bool) {
        for operation in &self.operations {
            if let Err(why) = operation.execute(surface, cairo, is_in_draw_event) {
                error!("Got error trying to execute an operation({operation:?}): {why}");
            }
        }

        if let Some(operation) = &self.current_operation {
            if let Err(why) = operation.execute(surface, cairo, is_in_draw_event) {
                error!("Got error trying to execute self.current_operation({operation:?}): {why}");
            }
        }

        // We don't want to draw window "crop indicators" in the following cases:
        //  * we're saving the screenshot
        //  * the user's tool is not the CropAndSave tool
        //  * we are in a crop drag
        if is_in_draw_event && self.current_tool() == Tool::CropAndSave && !self.is_in_crop_drag {
            if let Some(idx) = self.current_window {
                let Rectangle { x, y, w, h } = match self.selection_mode {
                    SelectionMode::WindowsWithDecorations => self.windows[idx].outer_rect,
                    SelectionMode::WindowsWithoutDecorations => self.windows[idx].content_rect,
                    _ => unreachable!(),
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
                cairo.set_dash(&[4.0, 21.0, 4.0], 0.0);
                log_if_err!(cairo.stroke());
                log_if_err!(cairo.restore());
            }
        }
    }
}

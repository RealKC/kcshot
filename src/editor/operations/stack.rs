use std::convert::TryFrom;

use crate::{
    editor::{data::Text, display_server::Window, utils::CairoExt},
    log_if_err,
};

use super::{Colour, Operation, Point, Rectangle, Tool};

use cairo::{Context, ImageSurface};
use tracing::{error, warn};

#[derive(Debug)]
pub struct OperationStack {
    operations: Vec<Operation>,
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
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum SelectionMode {
    WindowsWithDecorations,
    WindowsWithoutDecorations,
    IgnoreWindows,
}

impl TryFrom<u32> for SelectionMode {
    type Error = ();

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        use SelectionMode::*;
        match value {
            0 => Ok(WindowsWithDecorations),
            1 => Ok(WindowsWithoutDecorations),
            2 => Ok(IgnoreWindows),
            _ => Err(()),
        }
    }
}

impl OperationStack {
    pub fn new(windows: Vec<Window>) -> Self {
        Self {
            operations: vec![],
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
        if let Some(old_operation) = self.current_operation.take() {
            self.operations.push(old_operation);
        }

        self.current_operation = Some(Operation::create_default_for_tool(
            self.current_tool,
            point,
            &mut self.autoincrement_bubble_number,
            self.primary_colour,
            self.secondary_colour,
        ));
    }

    pub fn update_current_operation_end_coordinate(&mut self, new_width: f64, new_height: f64) {
        let current_operation = match self.current_operation.as_mut() {
            Some(curr) => curr,
            None => return,
        };

        match current_operation {
            Operation::Crop(rect) => {
                rect.w = new_width;
                rect.h = new_height;
                rect.normalise();
            }
            Operation::Blur { rect, .. } => {
                rect.w = new_width;
                rect.h = new_height;
                rect.normalise();
            }
            Operation::Pixelate { rect, .. } => {
                rect.w = new_width;
                rect.h = new_height;
                rect.normalise();
            }
            Operation::DrawLine { start, end, .. } => {
                *end = Point {
                    x: start.x + new_width,
                    y: start.y + new_height,
                }
            }
            Operation::DrawRectangle { rect, .. } => {
                dbg!(&rect);
                rect.w = new_width;
                rect.h = new_height;
                rect.normalise();
            }
            Operation::DrawArrow { start, end, .. } => {
                *end = Point {
                    x: start.x + new_width,
                    y: start.y + new_height,
                }
            }
            Operation::Highlight { rect } => {
                rect.w = new_width;
                rect.h = new_height;
                rect.normalise();
            }
            Operation::DrawEllipse { ellipse, .. } => {
                ellipse.w = new_width;
                ellipse.h = new_height;
            }
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
            // We reserve (w, h) == (0,0) as special values in order to signal the entire screen or
            // the window under the mouse as being the crop region
            if rect.w == 0.0 && rect.h == 0.0 {
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

        // We don't want to "crop indicators" for the windows when we're saving the screenshot (similarly
        // to what we do for normal crop) or when the user is creating a selection
        if is_in_draw_event && !self.is_in_crop_drag {
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

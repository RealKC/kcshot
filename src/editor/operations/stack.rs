use crate::editor::data::Text;

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
}

impl OperationStack {
    pub fn new() -> Self {
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
        }
    }

    pub fn set_current_tool(&mut self, tool: Tool) {
        self.current_tool = tool;
    }

    pub fn current_tool(&self) -> Tool {
        self.current_tool
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

    pub fn finish_current_operation(&mut self) {
        if let Some(operation) = self.current_operation.take() {
            self.operations.push(operation);
        }
    }

    pub fn crop_region(&self) -> Option<Rectangle> {
        // We do not look at the top of `self.operations` as cropping should be the last operation
        // in the UX I want.
        if let Some(Operation::Crop(rect)) = &self.current_operation {
            // We reserve (w, h) == (0,0) as special values in order to signal the entire screen as the
            // crop region
            if rect.w == 0.0 && rect.h == 0.0 {
                None
            } else {
                Some(*rect)
            }
        } else {
            None
        }
    }

    pub fn execute(&self, surface: &ImageSurface, cairo: &Context, is_in_draw_event: bool) {
        for operation in &self.operations {
            tracing::warn!("We had at least one operation");
            if let Err(why) = operation.execute(surface, cairo, is_in_draw_event) {
                error!("{}", why);
            }
        }

        if let Some(operation) = &self.current_operation {
            if let Err(why) = operation.execute(surface, cairo, is_in_draw_event) {
                error!("{}", why);
            }
        }
    }
}

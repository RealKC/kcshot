use super::{Colour, Operation, Point, Tool};

use cairo::{Context, ImageSurface};
use tracing::error;

#[derive(Debug)]
pub struct OperationStack {
    operations: Vec<Operation>,
    current_tool: Tool,
    current_operation: Option<Operation>,
    autoincrement_bubble_number: i32,
}

impl OperationStack {
    pub fn new() -> Self {
        Self {
            operations: vec![],
            current_tool: Tool::CropAndSave,
            current_operation: None,
            autoincrement_bubble_number: 1,
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
        ));
    }

    pub fn update_current_operation_end_coordinate(&mut self, point: Point) {
        let current_operation = match self.current_operation.as_mut() {
            Some(curr) => curr,
            None => return,
        };

        match current_operation {
            Operation::Crop(rect) => {
                rect.w = point.x - rect.x;
                rect.h = point.y - rect.y;
            }
            Operation::Blur { rect, .. } => {
                rect.w = point.x - rect.x;
                rect.h = point.y - rect.y;
            }
            Operation::Pixelate { rect, .. } => {
                rect.w = point.x - rect.x;
                rect.h = point.y - rect.y;
            }
            Operation::DrawLine { end, .. } => *end = point,
            Operation::DrawRectangle { rect, .. } => {
                rect.w = point.x - rect.x;
                rect.h = point.y - rect.y;
            }
            Operation::DrawArrow { end, .. } => *end = point,
            Operation::Highlight { rect } => {
                rect.w = point.x - rect.x;
                rect.h = point.y - rect.y;
            }
            Operation::DrawEllipse { ellipse, .. } => {
                ellipse.w = point.x - ellipse.x;
                ellipse.h = point.y - ellipse.y;
            }
            Operation::Bubble { .. } | Operation::Text { .. } => {}
        }
    }

    pub fn finish_current_operation(&mut self) {
        if let Some(operation) = self.current_operation.take() {
            self.operations.push(operation);
        }
    }

    pub fn change_top_operation_fill_colour(&mut self, _new_colour: Colour) {
        todo!()
    }

    pub fn execute(&self, surface: &ImageSurface, cairo: &Context) {
        for operation in &self.operations {
            tracing::warn!("We had at least one operation");
            if let Err(why) = operation.execute(surface, cairo) {
                error!("{}", why);
            }
        }

        if let Some(operation) = &self.current_operation {
            if let Err(why) = operation.execute(surface, cairo) {
                error!("{}", why);
            }
        }
    }
}

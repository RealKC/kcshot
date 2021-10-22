use super::{Colour, Operation, Point};

use cairo::{Context, ImageSurface};
use tracing::error;

pub struct OperationStack(Vec<Operation>);

impl OperationStack {
    pub fn new() -> Self {
        Self(vec![])
    }

    pub fn push_new_operation(&mut self, op: Operation) {
        self.0.push(op);
    }

    pub fn change_top_operation_border_colour(&mut self, _new_colour: Colour) {
        todo!()
    }

    pub fn change_top_operation_fill_colour(&mut self, _new_colour: Colour) {
        todo!()
    }

    pub fn change_top_operation_end(&mut self, _new_end: Point) {
        todo!();
    }

    pub fn execute(&self, surface: &mut ImageSurface, cairo: &Context) {
        for operation in &self.0 {
            if let Err(why) = operation.execute(surface, cairo) {
                error!("{}", why);
            }
        }
    }
}

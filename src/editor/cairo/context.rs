use cairo::Context;
use kcshot_data::colour::Colour;

pub trait CairoExt {
    fn set_source_colour(&self, colour: Colour);
}

impl CairoExt for Context {
    fn set_source_colour(&self, colour: Colour) {
        let Colour {
            red,
            green,
            blue,
            alpha,
        } = colour;

        let red = red as f64 / 255.0;
        let green = green as f64 / 255.0;
        let blue = blue as f64 / 255.0;
        let alpha = alpha as f64 / 255.0;

        self.set_source_rgba(red, green, blue, alpha);
    }
}

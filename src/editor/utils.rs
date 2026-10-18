#[macro_export]
macro_rules! log_if_err {
    ($call:expr) => {
        match $call {
            Ok(_) => {}
            Err(err) => ::tracing::error!(
                "Got error: {err:?}\n\twith the following call: {}",
                ::std::stringify!($call)
            ),
        }
    };
}

pub struct ContextLogger<'s> {
    ctx: &'s str,
    method: &'static str,
}

impl<'s> ContextLogger<'s> {
    pub fn new(ctx: &'s str, method: &'static str) -> Self {
        tracing::trace!("\x1b[32mEntering\x1b[0m context inside {method}: '{ctx}'");

        Self { ctx, method }
    }
}

impl Drop for ContextLogger<'_> {
    fn drop(&mut self) {
        let Self { method, ctx } = self;
        tracing::trace!("\x1b[31mExiting\x1b[0m context inside {method}: '{ctx}'");
    }
}

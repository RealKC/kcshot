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

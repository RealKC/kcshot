pub fn install_panic_hook_handler() {
    let prev = std::panic::take_hook();

    std::panic::set_hook(Box::new(move |info| {
        let payload = if let Some(s) = info.payload().downcast_ref::<&str>() {
            s
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s
        } else {
            "non-string payload"
        };

        let backtrace = std::backtrace::Backtrace::capture();
        let thread = std::thread::current();
        let thread_name = thread.name().unwrap_or("<unknown thread>");

        if let Some(location) = info.location() {
            tracing::error!(
                "thread '{thread_name}' panicked at: {}:{}{}: '{payload}'\n{}",
                location.file(),
                location.line(),
                location.column(),
                backtrace
            );
        } else {
            tracing::error!(
                "thread '{thread_name}' panicked: '{payload}'\n{}",
                backtrace
            );
        }

        prev(info);
    }));
}

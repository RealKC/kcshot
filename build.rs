fn main() {
    // See: https://docs.rs/diesel_migrations/2.0.0-rc.1/diesel_migrations/macro.embed_migrations.html#automatic-rebuilds
    println!("cargo:rerun-if-changed=migrations/");

    if std::option_env!("KCSHOT_LINTING").is_some() {
        println!("cargo:rustc-cfg=kcshot_linting");
    }

    glib_build_tools::compile_resources(
        &["resources"],
        "resources/resources.gresource.xml",
        "compiled.gresource",
    );
}

fn main() {
    // gsettings-macro doesn't currently do any hacks to rerun cargo if the schema changes, so
    // let's do it ourselves. (A hack is needed as proc-macros can't currently tell rustc they
    // depend on external files.)
    println!("cargo:rerun-if-changed=resources/kc.kcshot.gschema.xml");

    // See: https://docs.rs/diesel_migrations/2.0.0-rc.1/diesel_migrations/macro.embed_migrations.html#automatic-rebuilds
    println!("cargo:rerun-if-changed=migrations/");

    glib_build_tools::compile_resources(
        "resources",
        "resources/resources.gresource.xml",
        "compiled.gresource",
    );
}

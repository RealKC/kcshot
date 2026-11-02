fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    // See: https://docs.rs/diesel_migrations/2.0.0-rc.1/diesel_migrations/macro.embed_migrations.html#automatic-rebuilds
    println!("cargo:rerun-if-changed=migrations/");
}

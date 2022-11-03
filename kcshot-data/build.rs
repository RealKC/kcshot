fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    // gsettings-macro doesn't currently do any hacks to rerun cargo if the schema changes, so
    // let's do it ourselves. (A hack is needed as proc-macros can't currently tell rustc they
    // depend on external files.)
    println!("cargo:rerun-if-changed=../resources/kc.kcshot.gschema.xml");
}

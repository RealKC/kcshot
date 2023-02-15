use std::{env, process::Command};

fn main() {
    // See: https://docs.rs/diesel_migrations/2.0.0-rc.1/diesel_migrations/macro.embed_migrations.html#automatic-rebuilds
    println!("cargo:rerun-if-changed=migrations/");

    if std::option_env!("KCSHOT_LINTING").is_none() {
        let blueprint_dir = env::var("OUT_DIR").unwrap() + "/resources";
        compile_blueprints(&["src/editor/textdialog.blp"], &blueprint_dir);
        glib_build_tools::compile_resources(
            &[blueprint_dir.as_str(), "resources"],
            "resources/resources.gresource.xml",
            "compiled.gresource",
        );
    } else {
        println!("cargo:rustc-cfg=kcshot_linting");
    }
}

fn compile_blueprints(blueprints: &[&str], target: &str) {
    let blueprint_compiler = std::env::var("BLUEPRINT_COMPILER_PATH")
        .unwrap_or_else(|_| "blueprint-compiler".to_owned());

    let mut blueprint_compiler = Command::new(blueprint_compiler);
    blueprint_compiler
        .arg("batch-compile")
        .arg(target)
        .arg("src/");

    for blueprint in blueprints {
        println!("cargo:rerun-if-changed={blueprint}");
        blueprint_compiler.arg(blueprint);
    }

    let output = blueprint_compiler
        .output()
        .expect("Failed to execute blueprint-compiler");

    if !output.status.success() {
        panic!(
            "blueprint-compiler returned {}, with stderr:\n{}",
            output.status,
            std::str::from_utf8(&output.stderr).unwrap()
        );
    }
}

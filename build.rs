use core::panic;
use std::{env, process::Command};

fn main() {
    // See: https://docs.rs/diesel_migrations/2.0.0-rc.1/diesel_migrations/macro.embed_migrations.html#automatic-rebuilds
    println!("cargo:rerun-if-changed=migrations/");

    if std::option_env!("KCSHOT_LINTING").is_some() {
        println!("cargo:rustc-cfg=kcshot_linting");
    }

    let mut blueprintc = "blueprint-compiler".to_owned();

    // A hack to make the build work when building using meson and blueprint-compiler isn't in the path
    if let Ok(blueprint_path) = env::var("BLUEPRINT_PATH") {
        let path = env::var("PATH").unwrap();
        println!("cargo:rustc-env=PATH={path}:{blueprint_path}");
        blueprintc = format!("{blueprint_path}/{blueprintc}");
    }

    let blueprint_dir = env::var("OUT_DIR").unwrap() + "/resources";
    compile_blueprints(
        &blueprintc,
        &["src/history/context_menu.blp"],
        &blueprint_dir,
    );

    glib_build_tools::compile_resources(
        &["resources", &blueprint_dir],
        "resources/resources.gresource.xml",
        "compiled.gresource",
    );
}

fn compile_blueprints(blueprintc: &str, blueprints: &[&str], target: &str) {
    let mut blueprintc = Command::new(blueprintc);
    // blueprintc.args(["batch-compile", target, "src/"]);
    blueprintc.arg("batch-compile");
    blueprintc.arg(target);
    blueprintc.arg("src/");

    for blueprint in blueprints {
        println!("cargo:rerun-if-changed={blueprint}");
        blueprintc.arg(blueprint);
    }

    let output = blueprintc
        .output()
        .expect("Failed to execute blueprint-compiler");

    if !output.status.success() {
        panic!(
            "blueprint-compiler returned {}, with stdout=:\n{}\n\t and stderr:\n{}",
            output.status,
            std::str::from_utf8(&output.stdout).unwrap(),
            std::str::from_utf8(&output.stderr).unwrap()
        );
    }
}

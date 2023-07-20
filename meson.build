project('kcshot', 'rust', license: 'EUPL-1.2')

gnome = import('gnome')

dependency('sqlite3', version: '>= 3.37')
dependency('glib-2.0', version: '>= 2.66')
dependency('gio-2.0', version: '>= 2.66')
dependency('gtk4', version: '>= 4.0.0')

cargo = find_program('cargo', required: true)
glib_compile_resources = find_program('glib-compile-resources', required: true)
glib_compile_schemas = find_program('glib-compile-schemas', required: true)

cargo_options = ['--manifest-path', meson.project_source_root() / 'Cargo.toml']
cargo_options += ['--target-dir', meson.project_build_root() / 'src']

prefix = get_option('prefix')
bindir = prefix / get_option('bindir')

if get_option('profile') == 'default'
    cargo_options += ['--release']
    rust_target = 'release'
    message('Building in release mode')
else
    rust_target = 'debug'
    message('Building in debug mode')
endif

message(
    'blueprint-compiler is: ' + find_program('blueprint-compiler').full_path(),
)

fs = import('fs')
env = environment()
env.set('BLUEPRINT_PATH', fs.parent(find_program('blueprint-compiler').full_path()))
env.set('CARGO_HOME', meson.project_build_root() / 'cargo-home')
meson.add_devenv(env)

cargo_build = custom_target(
    'cargo-build',
    build_by_default: true,
    build_always_stale: true,
    output: meson.project_name(),
    console: true,
    install: true,
    install_dir: bindir,
    env: env,
    command: [
        'sh', '-c',
        cargo, 'build',
        cargo_options,
        '&&',
        'cp',
        'src' / rust_target / meson.project_name(),
        '@OUTPUT@',
    ],
)

test(
    'cargo-test',
    cargo,
    args: [
        'test',
        '--manifest-path=@0@/Cargo.toml'.format(meson.global_source_root()),
    ],
    env: env,
    timeout: 3000,
)

gnome.post_install(
    gtk_update_icon_cache: true,
    glib_compile_schemas: true,
    update_desktop_database: true,
)


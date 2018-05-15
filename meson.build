project(
  'fractal', 'rust',
  version: '0.1.28',
  license: 'GPL-3.0-only',
)

fractal_version = meson.project_version()
build_date = run_command('date', '+%Y-%m-%d').stdout().strip()

conf = configuration_data()
conf.set('VERSION', fractal_version)
conf.set('LICENSE', meson.project_license()[0])
conf.set('BUILD_DATE', build_date)

mac_bundle = get_option('mac_bundle')
datadir = get_option('datadir')
icondir = join_paths(datadir, 'icons')

if mac_bundle and host_machine.system() != 'darwin'
    error('Can\'t build a mac bundle if the target machine isn\'t darwin')
endif

subdir('fractal-gtk/res')

cargo = find_program('cargo', required: false)
gresource = find_program('glib-compile-resources', required: false)
cargo_vendor = find_program('cargo-vendor', required: false)
cargo_script = find_program('scripts/cargo.sh')
grabber = find_program('scripts/grabber.sh')
cargo_release = find_program('scripts/release.sh')
create_mac_bundle = find_program('scripts/create-mac-bundle.sh')

c = run_command(grabber)
sources = c.stdout().strip().split('\n')

fractal = custom_target('cargo-build',
                        build_by_default: true,
                        input: sources,
                        output: ['fractal'],
                        install: true,
                        install_dir: get_option('bindir'),
                        command: [cargo_script, '@CURRENT_SOURCE_DIR@', '@OUTPUT@'])

run_target('release', command: ['scripts/release.sh',
                                meson.project_name() + '-' + fractal_version
                               ])

if mac_bundle
    brew = find_program('brew', required: true)
    pkg_config = find_program('pkg-config', required: true)
    install_name_tool = find_program('install_name_tool', required: true)
    glib_compile_schemas = find_program('glib-compile-schemas', required: true)
    gtk_query_immodules = find_program('gtk-query-immodules-3.0', required: true)

    fractal_mac_wrapper = custom_target('cargo-build-mac-wrapper',
                                        build_by_default: true,
                                        input: sources,
                                        output: ['fractal-mac-wrapper'],
                                        install: true,
                                        install_dir: get_option('bindir'),
                                        command: [cargo_script, '@CURRENT_SOURCE_DIR@', '@OUTPUT@', 'fractal-mac-wrapper'])

    meson.add_install_script(
        create_mac_bundle.path(),
        join_paths(get_option('prefix'), get_option('bindir'), 'fractal'),
        get_option('prefix')
    )
endif

project(
  'fractal', 'rust',
  version: '0.1.29',
  license: 'GPLv3',
)

fractal_version = meson.project_version()
version_array = fractal_version.split('.')
fractal_major_version = version_array[0].to_int()
fractal_minor_version = version_array[1].to_int()
fractal_version_micro = version_array[2].to_int()

fractal_prefix = get_option('prefix')
fractal_bindir = join_paths(fractal_prefix, get_option('bindir'))

datadir = get_option('datadir')
icondir = join_paths(datadir, 'icons')

subdir('fractal-gtk/res')

cargo = find_program('cargo', required: false)
gresource = find_program('glib-compile-resources', required: false)
cargo_vendor = find_program('cargo-vendor', required: false)
cargo_script = find_program('scripts/cargo.sh')
grabber = find_program('scripts/grabber.sh')
cargo_release = find_program('scripts/release.sh')

c = run_command(grabber)
sources = c.stdout().strip().split('\n')

cargo_release = custom_target('cargo-build',
                        build_by_default: true,
                        input: sources,
                        output: ['fractal'],
                        install: true,
                        install_dir: fractal_bindir,
                        command: [cargo_script, '@CURRENT_SOURCE_DIR@', '@OUTPUT@'])

run_target('release', command: ['scripts/release.sh',
                                meson.project_name() + '-' + fractal_version
                               ])

meson.add_install_script('scripts/meson_post_install.py')

project(
  'fractal', 'rust',
  version: '3.30.0',
  license: 'GPLv3',
)

if get_option('profile') == 'development'
  profile = 'Devel'
  name_suffix = ' (Development)'
  vcs_tag = run_command('git', 'rev-parse', '--short', 'HEAD').stdout().strip()
  if vcs_tag == ''
    version_suffix = '-devel'
  else
    version_suffix = '-@0@'.format (vcs_tag)
  endif
else
  profile = ''
  name_suffix = ''
  version_suffix = ''
endif

application_id = 'org.gnome.Fractal@0@'.format(profile)

fractal_version = meson.project_version()
version_array = fractal_version.split('.')
fractal_major_version = version_array[0].to_int()
fractal_minor_version = version_array[1].to_int()
fractal_version_micro = version_array[2].to_int()

fractal_prefix = get_option('prefix')
fractal_bindir = join_paths(fractal_prefix, get_option('bindir'))
fractal_localedir = join_paths(fractal_prefix, get_option('localedir'))

datadir = get_option('datadir')

i18n = import('i18n')

podir = join_paths (meson.source_root (), 'fractal-gtk', 'po')

subdir('fractal-gtk/po')
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
                        console: true,
                        command: [cargo_script,
                                  '@CURRENT_SOURCE_DIR@',
                                  '@OUTPUT@',
                                  fractal_localedir,
                                  application_id,
                                  name_suffix,
                                  fractal_version + version_suffix,
                                  profile
                                 ])

run_target('release', command: ['scripts/release.sh',
                                meson.project_name() + '-' + fractal_version
                               ])

meson.add_install_script('scripts/meson_post_install.py')

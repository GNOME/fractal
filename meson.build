project(
  'fractal', 'rust',
  version: '3.30.0',
  license: 'GPLv3',
)

dependency('openssl', version: '>= 1.0')
dependency('dbus-1')
dependency('libsecret-1')

dependency('glib-2.0', version: '>= 2.56')
dependency('gio-2.0', version: '>= 2.56')
dependency('cairo', version: '>= 1.16')
dependency('gdk-pixbuf-2.0')
dependency('gtk+-3.0', version: '>= 3.22')
dependency('libhandy-0.0', version: '>= 0.0.5')
dependency('gtksourceview-3.0', version: '>= 3.22')

dependency('gstreamer-1.0', version: '>= 1.12')
dependency('gstreamer-base-1.0', version: '>= 1.12')
dependency('gstreamer-audio-1.0', version: '>= 1.12')
dependency('gstreamer-video-1.0', version: '>= 1.12')
dependency('gstreamer-player-1.0', version: '>= 1.12')
dependency('gstreamer-plugins-base-1.0', version: '>= 1.12')
dependency('gstreamer-plugins-bad-1.0', version: '>= 1.12')
dependency('gstreamer-bad-audio-1.0', version: '>= 1.12')
dependency('gstreamer-bad-video-1.0', version: '>= 1.12')

cargo = find_program('cargo', required: true)
gresource = find_program('glib-compile-resources', required: true)
gschemas = find_program('glib-compile-schemas', required: true)
cargo_vendor = find_program('cargo-vendor', required: false)

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

# Setup pre-commit hook for ensuring coding style is always consistent
rustup = find_program('rustup', required : false)
if not rustup.found()
    run_command('curl', 'https://sh.rustup.rs', '-sSf', '|', 'sh', '-s', '--', '-y')
    rustup = find_program('rustup')
endif
message('Installing nightly rustfmt..')
run_command(rustup, 'component', 'add', 'rustfmt-preview', '--toolchain', 'nightly')
message('Setting up git pre-commit hook..')
run_command('cp', '-f', 'hooks/pre-commit.hook', '.git/hooks/pre-commit')

meson.add_install_script('scripts/meson_post_install.py')

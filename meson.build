project(
  'fractal', 'rust',
  version: '4.4.0',
  license: 'GPL-3.0-or-later',
)

dependency('openssl', version: '>= 1.0')
dependency('dbus-1')

dependency('glib-2.0', version: '>= 2.56')
dependency('gio-2.0', version: '>= 2.56')
dependency('cairo', version: '>= 1.16')
dependency('gdk-pixbuf-2.0')
dependency('gtk+-3.0', version: '>= 3.24.7')
dependency('libhandy-1', version: '>= 1.2.0')
dependency('gtksourceview-4', version: '>= 4.0')
dependency('gspell-1', version: '>= 1.8')

dependency('gstreamer-1.0', version: '>= 1.12')
dependency('gstreamer-base-1.0', version: '>= 1.12')
dependency('gstreamer-audio-1.0', version: '>= 1.12')
dependency('gstreamer-video-1.0', version: '>= 1.12')
dependency('gstreamer-player-1.0', version: '>= 1.12')
dependency('gstreamer-plugins-base-1.0', version: '>= 1.12')
dependency('gstreamer-plugins-bad-1.0', version: '>= 1.12')
dependency('gstreamer-bad-audio-1.0', version: '>= 1.12')
dependency('gstreamer-pbutils-1.0', version: '>= 1.14')
dependency('gst-editing-services-1.0', version: '>= 1.14.4')

cargo = find_program('cargo', required: true)
gresource = find_program('glib-compile-resources', required: true)
gschemas = find_program('glib-compile-schemas', required: true)

rust_version = meson.get_compiler('rust').version()
min_req_rust_version = '1.46.0'
assert(rust_version >= min_req_rust_version, 'Minimum rust version ' + min_req_rust_version)

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

fractal_prefix = get_option('prefix')
fractal_bindir = join_paths(fractal_prefix, get_option('bindir'))
fractal_localedir = join_paths(fractal_prefix, get_option('localedir'))

datadir = get_option('datadir')
fractal_pkgdatadir = join_paths(fractal_prefix, datadir, meson.project_name())

i18n = import('i18n')
gnome = import('gnome')

podir = join_paths (meson.source_root (), 'fractal-gtk', 'po')

subdir('fractal-gtk/po')
subdir('fractal-gtk/res')

cargo_script = find_program('scripts/cargo.sh')
test_script = find_program('scripts/test.sh')
doc_script = find_program('scripts/cargo-doc.sh')

cargo_manifests = files(
  'Cargo.toml',
  'Cargo.lock',
  'fractal-gtk/Cargo.toml',
)

subdir('fractal-gtk/src')

meson.add_dist_script(
  'scripts/vendor.sh',
  meson.source_root(),
  join_paths(meson.build_root(), 'meson-dist', meson.project_name() + '-' + fractal_version)
)

test(
  'cargo-test',
  test_script,
  args: meson.build_root(),
  workdir: meson.source_root(),
  timeout: 3000,
)

if get_option('profile') == 'development'
    # Setup pre-commit hook for ensuring coding style is always consistent
    message('Setting up git pre-commit hook..')
    run_command('cp', '-f', 'hooks/pre-commit.hook', '.git/hooks/pre-commit')
endif

meson.add_install_script('scripts/meson_post_install.py')

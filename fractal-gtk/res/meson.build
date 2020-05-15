subdir('icons')

desktop_conf = configuration_data()
desktop_conf.set('icon', application_id)
desktop_file = i18n.merge_file ('desktop-file',
  type: 'desktop',
  input: configure_file(
    input: files('org.gnome.Fractal.desktop.in.in'),
    output: 'org.gnome.Fractal.desktop.in',
    configuration: desktop_conf
  ),
  output: '@0@.desktop'.format(application_id),
  po_dir: podir,
  install: true,
  install_dir: join_paths (datadir, 'applications')
)
desktop_file_validate = find_program('desktop-file-validate', required: false)
if desktop_file_validate.found()
  test(
    'validate-desktop',
    desktop_file_validate,
    args: [
      desktop_file.full_path()
    ]
  )
endif


metainfo_conf = configuration_data()
metainfo_conf.set('appid', application_id)
metainfo_file = i18n.merge_file ('metainfo-file',
  input: configure_file(
    input: files('org.gnome.Fractal.metainfo.xml.in.in'),
    output: 'org.gnome.Fractal.metainfo.xml.in',
    configuration: metainfo_conf
  ),
  output: '@0@.metainfo.xml'.format(application_id),
  po_dir: podir,
  install: true,
  install_dir: join_paths (datadir, 'metainfo')
)
appstream_util = find_program('appstream-util', required: false)
if appstream_util.found()
  test(
    'validate-metainfo', appstream_util,
    args: [
      'validate-relax', '--nonet', metainfo_file.full_path()
    ]
  )
endif
install_data('org.gnome.Fractal.gschema.xml', install_dir: join_paths(get_option('datadir'), 'glib-2.0', 'schemas'))

fractal_resources = gnome.compile_resources(
  'resources',
  'resources.xml',
  gresource_bundle: true,
  source_dir: meson.current_build_dir()
)

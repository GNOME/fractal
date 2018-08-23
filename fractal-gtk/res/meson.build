subdir('icons')

i18n.merge_file ('desktop-file',
  type: 'desktop',
  input: 'org.gnome.Fractal.desktop.in',
  output: 'org.gnome.Fractal.desktop',
  po_dir: podir,
  install: true,
  install_dir: join_paths (datadir, 'applications')
)

i18n.merge_file ('appdata-file',
  input: 'org.gnome.Fractal.appdata.xml.in',
  output: 'org.gnome.Fractal.appdata.xml',
  po_dir: podir,
  install: true,
  install_dir: join_paths (datadir, 'appdata')
)

i18n.merge_file ('appdata-file',
  input: 'org.gnome.Fractal.gschema.xml.in',
  output: 'org.gnome.Fractal.gschema.xml',
  po_dir: podir,
  install: true,
  install_dir: join_paths(datadir, 'glib-2.0', 'schemas')
)

subdir('icons')

appdata = configure_file(input : 'org.gnome.Fractal.appdata.xml.in',
                         output : 'org.gnome.Fractal.appdata.xml',
                         configuration : conf)

install_data('org.gnome.Fractal.desktop', install_dir : datadir + '/applications')
install_data(appdata, install_dir : datadir + '/appdata')

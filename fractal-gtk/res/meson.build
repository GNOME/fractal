subdir('icons')

appdata = configure_file(input : 'org.gnome.Fractal.appdata.xml.in',
                         output : 'org.gnome.Fractal.appdata.xml',
                         configuration : conf)

infoplist = configure_file(input : 'Info.plist.in',
                           output : 'Info.plist',
                           configuration : conf)


if host_machine.system() == 'linux' or host_machine.system() == 'bsd'
    install_data('org.gnome.Fractal.desktop', install_dir : datadir + '/applications')
    install_data(appdata, install_dir : datadir + '/appdata')
elif host_machine.system() == 'darwin' and mac_bundle
    install_data(infoplist, install_dir : get_option('prefix') + '/Contents')
endif

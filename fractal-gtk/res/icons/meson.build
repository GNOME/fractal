install_subdir('hicolor', install_dir: icondir)

# Install the mac icon only if we are building a .app bundle for macOS
iconutil = find_program('iconutil', required: mac_bundle)

if iconutil.found()
    icns = custom_target('mac-icns',
                         input : 'fractal.iconset',
                         output : 'fractal.icns',
                         command : [iconutil, '-c', 'icns', '-o', '@OUTPUT@', '@INPUT@'],
                         install : mac_bundle,
                         install_dir : datadir)
endif

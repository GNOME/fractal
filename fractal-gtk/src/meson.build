global_conf = configuration_data()
global_conf.set_quoted('APP_ID', application_id)
global_conf.set_quoted('VERSION', fractal_version + version_suffix)
global_conf.set_quoted('LOCALEDIR', fractal_localedir)
global_conf.set_quoted('NAME_SUFFIX', name_suffix)
config_rs = configure_file(
  input: 'config.rs.in',
  output: 'config.rs',
  configuration: global_conf
)

run_command(
  'cp',
  config_rs,
  meson.current_source_dir(),
  check: true
)

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
                                  '@SOURCE_ROOT@',
                                  '@OUTPUT@',
                                  profile
                                 ])


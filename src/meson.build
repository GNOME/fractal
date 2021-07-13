global_conf = configuration_data()
global_conf.set_quoted('APP_ID', application_id)
global_conf.set_quoted('PKGDATADIR', pkgdatadir)
global_conf.set_quoted('PROFILE', profile)
global_conf.set_quoted('VERSION', version + version_suffix)
global_conf.set_quoted('GETTEXT_PACKAGE', gettext_package)
global_conf.set_quoted('LOCALEDIR', localedir)
config = configure_file(
  input: 'config.rs.in',
  output: 'config.rs',
  configuration: global_conf
)
# Copy the config.rs output to the source directory.
run_command(
  'cp',
  meson.build_root() / 'src' / 'config.rs',
  meson.source_root() / 'src' / 'config.rs',
  check: true
)

sources = files(
  'application.rs',
  'components/avatar.rs',
  'components/context_menu_bin.rs',
  'components/label_with_widgets.rs',
  'components/mod.rs',
  'components/pill.rs',
  'components/room_title.rs',
  'components/in_app_notification.rs',
  'components/spinner_button.rs',
  'config.rs',
  'error.rs',
  'main.rs',
  'window.rs',
  'login.rs',
  'secret.rs',
  'utils.rs',
  'session/avatar.rs',
  'session/event_source_dialog.rs',
  'session/user.rs',
  'session/mod.rs',
  'session/content/content.rs',
  'session/content/divider_row.rs',
  'session/content/explore/explore.rs',
  'session/content/explore/mod.rs',
  'session/content/explore/public_room.rs',
  'session/content/explore/public_room_list.rs',
  'session/content/explore/public_room_row.rs',
  'session/content/item_row.rs',
  'session/content/invite.rs',
  'session/content/markdown_popover.rs',
  'session/content/message_row.rs',
  'session/content/mod.rs',
  'session/content/room_history.rs',
  'session/content/state_row.rs',
  'session/room/event.rs',
  'session/room/highlight_flags.rs',
  'session/room/item.rs',
  'session/room/mod.rs',
  'session/room/room.rs',
  'session/room/room_type.rs',
  'session/room_list.rs',
  'session/room/timeline.rs',
  'session/sidebar/account_switcher.rs',
  'session/sidebar/item_list.rs',
  'session/sidebar/category.rs',
  'session/sidebar/category_row.rs',
  'session/sidebar/mod.rs',
  'session/sidebar/row.rs',
  'session/sidebar/room_row.rs',
  'session/sidebar/selection.rs',
  'session/sidebar/sidebar.rs',
)

custom_target(
  'cargo-build',
  build_by_default: not get_option('skip-cargo-build'),
  input: sources,
  output: meson.project_name(),
  console: true,
  install: true,
  install_dir: bindir,
  depends: resources,
  command: [
    cargo_script,
    meson.build_root(),
    meson.source_root(),
    '@OUTPUT@',
    profile,
    meson.project_name(),
  ]
)

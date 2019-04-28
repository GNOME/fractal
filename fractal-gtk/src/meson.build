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

# include_bytes! only takes a string literal
resource_conf = configuration_data()
resource_conf.set_quoted('RESOURCEFILE', fractal_resources.full_path())
resource_rs = configure_file(
  input: 'static_resources.rs.in',
  output: 'static_resources.rs',
  configuration: resource_conf
)

run_command(
  'cp',
  resource_rs,
  meson.current_source_dir(),
  check: true
)

run_target(
  'doc',
  command: [
    doc_script,
    '@SOURCE_ROOT@',
    '@OUTPUT@',
    meson.build_root(),
    profile
  ],
  depends: fractal_resources
)

# Keep in alphabetical order
app_sources = files(
  'actions/account_settings.rs',
  'actions/global.rs',
  'actions/login.rs',
  'actions/mod.rs',
  'actions/room_history.rs',
  'actions/room_settings.rs',
  'app/connect/account.rs',
  'app/connect/autocomplete.rs',
  'app/connect/direct.rs',
  'app/connect/directory.rs',
  'app/connect/headerbar.rs',
  'app/connect/invite.rs',
  'app/connect/join_room.rs',
  'app/connect/leave_room.rs',
  'app/connect/markdown.rs',
  'app/connect/mod.rs',
  'app/connect/new_room.rs',
  'app/connect/roomlist_search.rs',
  'app/connect/send.rs',
  'app/backend_loop.rs',
  'app/mod.rs',
  'app/windowstate.rs',
  'appop/about.rs',
  'appop/account.rs',
  'appop/attach.rs',
  'appop/directory.rs',
  'appop/invite.rs',
  'appop/login.rs',
  'appop/media_viewer.rs',
  'appop/member.rs',
  'appop/message.rs',
  'appop/mod.rs',
  'appop/notifications.rs',
  'appop/notify.rs',
  'appop/room.rs',
  'appop/room_settings.rs',
  'appop/start_chat.rs',
  'appop/state.rs',
  'appop/sync.rs',
  'appop/user.rs',
  'cache/mod.rs',
  'cache/state.rs',
  'widgets/address.rs',
  'widgets/autocomplete.rs',
  'widgets/avatar.rs',
  'widgets/divider.rs',
  'widgets/error_dialog.rs',
  'widgets/file_dialog.rs',
  'widgets/image.rs',
  'widgets/inline_player.rs',
  'widgets/login.rs',
  'widgets/media_viewer.rs',
  'widgets/member.rs',
  'widgets/members_list.rs',
  'widgets/message.rs',
  'widgets/message_menu.rs',
  'widgets/mod.rs',
  'widgets/room.rs',
  'widgets/room_history.rs',
  'widgets/roomlist.rs',
  'widgets/roomrow.rs',
  'widgets/room_history.rs',
  'widgets/roomlist.rs',
  'widgets/roomrow.rs',
  'widgets/room_settings.rs',
  'widgets/scroll_widget.rs',
  'widgets/source_dialog.rs',
  'widgets/sourceview_entry.rs',
  'globals.rs',
  'i18n.rs',
  'main.rs',
  'passwd.rs',
  'uibuilder.rs',
  'uitypes.rs',
  'util.rs'
)

cargo_release = custom_target('cargo-build',
                        build_by_default: true,
                        input: [
                          cargo_manifests,
                          api_sources,
                          app_sources,
                        ],
                        depends: fractal_resources,
                        output: ['fractal'],
                        install: true,
                        install_dir: fractal_bindir,
                        console: true,
                        command: [cargo_script,
                                  '@SOURCE_ROOT@',
                                  '@OUTPUT@',
                                  meson.build_root(),
                                  profile
                                 ])


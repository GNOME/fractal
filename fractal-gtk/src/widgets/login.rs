use gio::prelude::*;
use gtk::prelude::*;
use log::info;
use url::Url;

use crate::actions;
use crate::actions::global::AppState;
use crate::actions::login::LoginState;
use crate::appop::AppOp;
use crate::globals;
use crate::i18n::i18n;
use crate::widgets::ErrorDialog;

use fractal_api::backend::register::get_well_known;

use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct LoginWidget {
    pub container: gtk::Stack,
    pub headers: gtk::Stack,
    pub server_entry: gtk::Entry,
    pub username_entry: gtk::Entry,
    pub password_entry: gtk::Entry,
    server_err_label: gtk::Label,
    credentials_err_label: gtk::Label,
    actions: gio::SimpleActionGroup,
}

impl LoginWidget {
    pub fn new(op: &Arc<Mutex<AppOp>>) -> Self {
        let widget = Self::default();

        let weak_server = widget.server_entry.downgrade();
        let weak_username = widget.username_entry.downgrade();
        let weak_password = widget.password_entry.downgrade();
        let weak_err = widget.credentials_err_label.downgrade();

        // Grab the focus for each state
        let weak_ser = weak_server.clone();
        let weak_user = weak_username.clone();
        widget
            .container
            .connect_property_visible_child_name_notify(move |container| {
                let server = upgrade_weak!(weak_ser);
                let username = upgrade_weak!(weak_user);

                let state: LoginState = container
                    .get_visible_child_name()
                    .unwrap()
                    .to_string()
                    .into();

                match state {
                    LoginState::ServerChooser => server.grab_focus(),
                    LoginState::Credentials => username.grab_focus(),
                    _ => (),
                }
            });

        let op = op.clone();
        let weak_server = widget.server_entry.downgrade();

        let login = widget
            .actions
            .lookup_action("login")
            .expect("Could not find 'login' action for LoginWidget")
            .downcast::<gio::SimpleAction>()
            .expect("Could not cast action 'login' to SimpleAction");

        let weak_pass = weak_password.clone();
        login.connect_activate(move |_, _| {
            let server_entry = upgrade_weak!(weak_server);
            let username_entry = upgrade_weak!(weak_username);
            let password_entry = upgrade_weak!(weak_pass);
            let err_label = upgrade_weak!(weak_err);

            if let Some(txt) = server_entry.get_text() {
                let username = username_entry
                    .get_text()
                    .map_or(String::new(), |gstr| gstr.to_string());

                let password = password_entry
                    .get_text()
                    .map_or(String::new(), |gstr| gstr.to_string());

                let txt = format!(
                    "https://{}",
                    txt.trim()
                        .trim_start_matches("http://")
                        .trim_start_matches("https://"),
                );

                if !password.is_empty() && !username.is_empty() {
                    // take the user's homeserver value if the
                    // well-known request fails
                    let hs_url = Url::parse(&txt);

                    if hs_url.is_err() {
                        let msg = i18n("Malformed server URL");
                        ErrorDialog::new(false, &msg);
                        return;
                    };

                    let mut homeserver_url =
                        hs_url.expect("hs_url must return earlier if it's Err");
                    let mut idserver = globals::DEFAULT_IDENTITYSERVER.clone();
                    match get_well_known(&txt) {
                        // TODO: Use Url everywhere
                        Ok(response) => {
                            info!("Got well-known response from {}: {:#?}", &txt, response);
                            homeserver_url =
                                Url::parse(&response.homeserver.base_url).unwrap_or(homeserver_url);
                            idserver = response
                                .identity_server
                                .and_then(|ids| Url::parse(&ids.base_url).ok())
                                .unwrap_or(idserver);
                        }
                        Err(e) => info!("Failed to .well-known request: {:#?}", e),
                    };

                    err_label.hide();
                    op.lock().unwrap().set_state(AppState::Loading);
                    op.lock().unwrap().since = None;
                    op.lock().unwrap().connect(
                        Some(username),
                        Some(password),
                        homeserver_url,
                        idserver,
                    );
                } else {
                    err_label.show();
                }
            }
        });

        let credentials = widget
            .actions
            .lookup_action("credentials")
            .expect("Could not find 'credentials' action for LoginWidget")
            .downcast::<gio::SimpleAction>()
            .expect("Could not cast action 'credentials' to SimpleAction");
        widget
            .server_entry
            .connect_activate(move |_| credentials.activate(None));

        widget.username_entry.connect_activate(move |_| {
            let password_entry = upgrade_weak!(weak_password);
            password_entry.grab_focus();
        });

        widget
            .password_entry
            .connect_activate(move |_| login.activate(None));

        widget
    }
}

impl Default for LoginWidget {
    fn default() -> Self {
        let builder = gtk::Builder::new_from_resource("/org/gnome/Fractal/ui/login_flow.ui");

        let container: gtk::Stack = builder.get_object("login_flow_stack").unwrap();
        let headers: gtk::Stack = builder.get_object("login_flow_headers").unwrap();
        let server_entry = builder.get_object("server_chooser_entry").unwrap();
        let username_entry = builder.get_object("username_entry").unwrap();
        let password_entry = builder.get_object("password_entry").unwrap();

        let server_err_label = builder.get_object("server_err_label").unwrap();
        let credentials_err_label = builder.get_object("credentials_err_label").unwrap();

        let actions = actions::Login::new(&container, &headers, &server_entry, &server_err_label);

        container.show_all();
        headers.show_all();

        LoginWidget {
            container,
            headers,
            server_entry,
            username_entry,
            password_entry,
            server_err_label,
            credentials_err_label,
            actions,
        }
    }
}

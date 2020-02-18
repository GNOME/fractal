use log::error;

use fractal_api::r0::AccessToken;

use fractal_api::url::Url;

use crate::appop::AppOp;

use crate::backend::BKCommand;
use crate::backend::BKResponse;
use crate::backend::Backend;
use crate::cache;

use std::sync::mpsc::channel;
use std::sync::mpsc::{Receiver, Sender};

use crate::app::backend_loop;

use crate::passwd::PasswordStorage;

use crate::actions::AppState;

use super::LoginData;

impl AppOp {
    pub fn bk_login(
        &mut self,
        uid: String,
        access_token: AccessToken,
        device: Option<String>,
        server_url: Url,
        identity_url: Url,
    ) {
        if let Err(_) = self.store_token(uid.clone(), access_token.clone()) {
            error!("Can't store the token using libsecret");
        }

        self.set_login_data(LoginData {
            access_token,
            uid,
            username: None,
            avatar: None,
            server_url,
            identity_url,
        });

        self.set_state(AppState::NoRoom);
        self.device_id = self.device_id.clone().or(device);
        self.since = None;
        self.get_username();

        // initial sync, we're shoing some feedback to the user
        self.initial_sync(true);
        self.sync(true);
        self.init_protocols();
    }

    pub fn bk_logout(&mut self) {
        self.set_rooms(vec![], true);
        if let Err(_) = cache::get().destroy() {
            error!("Error removing cache file");
        }

        self.syncing = false;

        self.set_state(AppState::Login);
        self.login_data = None;
        self.device_id = None;

        // stoping the backend and starting again, we don't want to receive more messages from
        // backend
        self.backend.send(BKCommand::ShutDown).unwrap();

        let (tx, rx): (Sender<BKResponse>, Receiver<BKResponse>) = channel();
        let bk = Backend::new(tx);
        self.backend = bk.run();
        backend_loop(rx);
    }

    #[allow(dead_code)]
    // TODO
    /*
    pub fn register(&mut self) {
        let user_entry: gtk::Entry = self
            .ui
            .builder
            .get_object("register_username")
            .expect("Can't find register_username in ui file.");
        let pass_entry: gtk::Entry = self
            .ui
            .builder
            .get_object("register_password")
            .expect("Can't find register_password in ui file.");
        let pass_conf: gtk::Entry = self
            .ui
            .builder
            .get_object("register_password_confirm")
            .expect("Can't find register_password_confirm in ui file.");
        let server_entry: gtk::Entry = self
            .ui
            .builder
            .get_object("register_server")
            .expect("Can't find register_server in ui file.");
        let _idp_entry: gtk::Entry = self
            .ui
            .builder
            .get_object("login_idp")
            .expect("Can't find login_idp in ui file.");

        let username = match user_entry.get_text() {
            Some(s) => s.to_string(),
            None => String::new(),
        };
        let password = match pass_entry.get_text() {
            Some(s) => s.to_string(),
            None => String::new(),
        };
        let passconf = match pass_conf.get_text() {
            Some(s) => s.to_string(),
            None => String::new(),
        };

        if password != passconf {
            let msg = i18n("Passwords didn’t match, try again");
            ErrorDialog::new(false, &msg);
            return;
        }

        let server = match server_entry
            .get_text()
            .as_ref()
            .map(Url::parse)
            .unwrap_or(Ok(globals::DEFAULT_HOMESERVER))
        {
            Ok(u) => u,
            Err(_) => {
                let msg = i18n("Malformed server URL");
                ErrorDialog::new(false, &msg);
                return;
            }
        }
        // FIXME: ask also for the identity server

        //self.store_pass(username.clone(), password.clone(), server_url.clone())
        //    .unwrap_or_else(|_| {
        //        // TODO: show an error
        //        error!("Can't store the password using libsecret");
        //    });

        self.backend
            .send(BKCommand::Register(username, password, server, id_s))
            .unwrap();
    }
    */

    pub fn connect(&mut self, username: String, password: String, server: Url, identity: Url) {
        self.store_pass(
            username.clone(),
            password.clone(),
            server.clone(),
            identity.clone(),
        )
        .unwrap_or_else(|_| {
            // TODO: show an error
            error!("Can't store the password using libsecret");
        });

        self.backend
            .send(BKCommand::Login(username, password, server, identity))
            .unwrap();
    }

    pub fn disconnect(&self) {
        self.backend.send(BKCommand::ShutDown).unwrap();
    }

    pub fn logout(&mut self) {
        let login_data = unwrap_or_unit_return!(self.login_data.clone());
        let _ = self.delete_pass("fractal");
        self.backend
            .send(BKCommand::Logout(
                login_data.server_url,
                login_data.access_token,
            ))
            .unwrap();
        self.bk_logout();
        *self.room_back_history.borrow_mut() = vec![];
    }
}

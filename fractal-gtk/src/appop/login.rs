use crate::i18n::i18n;
use log::error;

use gtk;
use gtk::prelude::*;

use url::Url;

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
use crate::widgets::ErrorDialog;

impl AppOp {
    pub fn bk_login(&mut self, uid: String, token: String, device: Option<String>) {
        self.logged_in = true;
        if let Err(_) = self.store_token(uid.clone(), token) {
            error!("Can't store the token using libsecret");
        }

        self.set_state(AppState::NoRoom);
        self.set_uid(Some(uid.clone()));
        if self.device_id == None {
            self.set_device(device);
        }
        /* Do we need to set the username to uid
        self.set_username(Some(uid));*/
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

        self.logged_in = false;
        self.syncing = false;

        self.set_state(AppState::Login);
        self.set_uid(None);
        self.set_device(None);
        self.set_username(None);
        self.set_avatar(None);

        // stoping the backend and starting again, we don't want to receive more messages from
        // backend
        self.backend.send(BKCommand::ShutDown).unwrap();

        let (tx, rx): (Sender<BKResponse>, Receiver<BKResponse>) = channel();
        let bk = Backend::new(tx);
        self.backend = bk.run();
        backend_loop(rx);
    }

    #[allow(dead_code)]
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

        if let Some(s) = server_entry.get_text() {
            match Url::parse(&s) {
                Ok(u) => {
                    self.server_url = u;
                }
                Err(_) => {
                    let msg = i18n("Malformed server URL");
                    ErrorDialog::new(false, &msg);
                    return;
                }
            }
        }
        /* FIXME ask also for the identity server */

        //self.store_pass(username.clone(), password.clone(), server_url.clone())
        //    .unwrap_or_else(|_| {
        //        // TODO: show an error
        //        error!("Can't store the password using libsecret");
        //    });

        let uname = username.clone();
        let pass = password.clone();
        let ser = self.server_url.to_string();
        self.backend
            .send(BKCommand::Register(uname, pass, ser))  // TODO: Change command type to url
            .unwrap();
    }

    pub fn connect(
        &mut self,
        username: Option<String>,
        password: Option<String>,
        server: Url,
        identity: Url,
    ) -> Option<()> {
        self.server_url = server;
        self.identity_url = identity;

        self.store_pass(
            username.clone()?,
            password.clone()?,
            self.server_url.to_string(),
            self.identity_url.to_string(),
        )
        .unwrap_or_else(|_| {
            // TODO: show an error
            error!("Can't store the password using libsecret");
        });

        let uname = username?;
        let pass = password?;
        let ser = self.server_url.to_string();
        self.backend
            .send(BKCommand::Login(uname, pass, ser))  // TODO: Change command type to url
            .unwrap();
        Some(())
    }

    pub fn set_token(
        &mut self,
        token: Option<String>,
        uid: Option<String>,
        server: Url,
    ) -> Option<()> {
        self.server_url = server;

        self.backend
            .send(BKCommand::SetToken(token?, uid?))
            .unwrap();
        Some(())
    }

    #[allow(dead_code)]
    pub fn connect_guest(&mut self, server: Url) {
        self.server_url = server;

        self.backend
            .send(BKCommand::Guest(self.server_url.to_string()))  // TODO: Change command type to url
            .unwrap();
    }

    pub fn disconnect(&self) {
        self.backend.send(BKCommand::ShutDown).unwrap();
    }

    pub fn logout(&mut self) {
        let _ = self.delete_pass("fractal");
        self.backend
            .send(BKCommand::Logout(self.server_url.clone()))
            .unwrap();
        self.bk_logout();
    }
}

extern crate gtk;
extern crate gettextrs;

use self::gtk::prelude::*;
use self::gettextrs::gettext;

use appop::AppOp;
use appop::state::AppState;

use cache;
use backend::Backend;
use backend::BKCommand;
use backend::BKResponse;

use std::sync::mpsc::channel;
use std::sync::mpsc::{Sender, Receiver};

use app::backend_loop;

use passwd::PasswordStorage;

impl AppOp {
    pub fn bk_login(&mut self, uid: String, token: String) {
        self.logged_in = true;
        self.clean_login();
        if let Err(_) = self.store_token(uid.clone(), token) {
            println!("Error: Can't store the token using libsecret");
        }

        self.set_state(AppState::Chat);
        self.set_uid(Some(uid.clone()));
        /* Do we need to set the username to uid
        self.set_username(Some(uid));*/
        self.get_username();

        // initial sync, we're shoing some feedback to the user
        self.initial_sync(true);

        self.sync();

        self.init_protocols();
    }

    pub fn bk_logout(&mut self) {
        self.set_rooms(&vec![], None);
        if let Err(_) = cache::destroy() {
            println!("Error removing cache file");
        }

        self.logged_in = false;
        self.syncing = false;

        self.set_state(AppState::Login);
        self.set_uid(None);
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

    pub fn clean_login(&self) {
        let user_entry: gtk::Entry = self.ui.builder
            .get_object("login_username")
            .expect("Can't find login_username in ui file.");
        let pass_entry: gtk::Entry = self.ui.builder
            .get_object("login_password")
            .expect("Can't find login_password in ui file.");
        let server_entry: gtk::Entry = self.ui.builder
            .get_object("login_server")
            .expect("Can't find login_server in ui file.");
        let idp_entry: gtk::Entry = self.ui.builder
            .get_object("login_idp")
            .expect("Can't find login_idp in ui file.");

        user_entry.set_text("");
        pass_entry.set_text("");
        server_entry.set_text("https://matrix.org");
        idp_entry.set_text("https://vector.im");
    }

    pub fn login(&mut self) {
        let user_entry: gtk::Entry = self.ui.builder
            .get_object("login_username")
            .expect("Can't find login_username in ui file.");
        let pass_entry: gtk::Entry = self.ui.builder
            .get_object("login_password")
            .expect("Can't find login_password in ui file.");
        let server_entry: gtk::Entry = self.ui.builder
            .get_object("login_server")
            .expect("Can't find login_server in ui file.");
        let login_error: gtk::Label = self.ui.builder
            .get_object("login_error_msg")
            .expect("Can't find login_error_msg in ui file.");

        let username = user_entry.get_text();
        let password = pass_entry.get_text();

        if username.clone().unwrap_or_default().is_empty() ||
           password.clone().unwrap_or_default().is_empty() {
            login_error.set_text(gettext("Invalid username or password").as_str());
            login_error.show();
            return;
        } else {
            login_error.set_text(gettext("Unknown Error").as_str());
            login_error.hide();
        }

        self.set_state(AppState::Loading);
        self.since = None;
        self.connect(username, password, server_entry.get_text());
    }

    pub fn set_login_pass(&self, username: &str, password: &str, server: &str) {
        let user_entry: gtk::Entry = self.ui.builder
            .get_object("login_username")
            .expect("Can't find login_username in ui file.");
        let pass_entry: gtk::Entry = self.ui.builder
            .get_object("login_password")
            .expect("Can't find login_password in ui file.");
        let server_entry: gtk::Entry = self.ui.builder
            .get_object("login_server")
            .expect("Can't find login_server in ui file.");

        user_entry.set_text(username);
        pass_entry.set_text(password);
        server_entry.set_text(server);
    }

    #[allow(dead_code)]
    pub fn register(&mut self) {
        let user_entry: gtk::Entry = self.ui.builder
            .get_object("register_username")
            .expect("Can't find register_username in ui file.");
        let pass_entry: gtk::Entry = self.ui.builder
            .get_object("register_password")
            .expect("Can't find register_password in ui file.");
        let pass_conf: gtk::Entry = self.ui.builder
            .get_object("register_password_confirm")
            .expect("Can't find register_password_confirm in ui file.");
        let server_entry: gtk::Entry = self.ui.builder
            .get_object("register_server")
            .expect("Can't find register_server in ui file.");

        let username = match user_entry.get_text() {
            Some(s) => s,
            None => String::from(""),
        };
        let password = match pass_entry.get_text() {
            Some(s) => s,
            None => String::from(""),
        };
        let passconf = match pass_conf.get_text() {
            Some(s) => s,
            None => String::from(""),
        };

        if password != passconf {
            self.show_error(gettext("Passwords didn't match, try again"));
            return;
        }

        self.server_url = match server_entry.get_text() {
            Some(s) => s,
            None => String::from("https://matrix.org"),
        };

        //self.store_pass(username.clone(), password.clone(), server_url.clone())
        //    .unwrap_or_else(|_| {
        //        // TODO: show an error
        //        println!("Error: Can't store the password using libsecret");
        //    });

        let uname = username.clone();
        let pass = password.clone();
        let ser = self.server_url.clone();
        self.backend.send(BKCommand::Register(uname, pass, ser)).unwrap();
    }

    pub fn connect(&mut self, username: Option<String>, password: Option<String>, server: Option<String>) -> Option<()> {
        self.server_url = match server {
            Some(s) => s,
            None => String::from("https://matrix.org"),
        };

        self.store_pass(username.clone()?, password.clone()?, self.server_url.clone())
            .unwrap_or_else(|_| {
                // TODO: show an error
                println!("Error: Can't store the password using libsecret");
            });

        let uname = username?;
        let pass = password?;
        let ser = self.server_url.clone();
        self.backend.send(BKCommand::Login(uname, pass, ser)).unwrap();
        Some(())
    }

    pub fn set_token(&mut self, token: Option<String>, uid: Option<String>, server: Option<String>) -> Option<()> {
        self.server_url = match server {
            Some(s) => s,
            None => String::from("https://matrix.org"),
        };

        let ser = self.server_url.clone();
        self.backend.send(BKCommand::SetToken(token?, uid?, ser)).unwrap();
        Some(())
    }

    #[allow(dead_code)]
    pub fn connect_guest(&mut self, server: Option<String>) {
        self.server_url = match server {
            Some(s) => s,
            None => String::from("https://matrix.org"),
        };

        self.backend.send(BKCommand::Guest(self.server_url.clone())).unwrap();
    }

    pub fn disconnect(&self) {
        self.backend.send(BKCommand::ShutDown).unwrap();
    }

    pub fn logout(&mut self) {
        let _ = self.delete_pass("fractal");
        self.backend.send(BKCommand::Logout).unwrap();
        self.bk_logout();
    }
}

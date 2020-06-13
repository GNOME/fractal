use log::error;

use fractal_api::backend::register;
use fractal_api::identifiers::UserId;
use fractal_api::r0::AccessToken;
use fractal_api::util::ResultExpectLog;

use fractal_api::url::Url;

use crate::app::App;
use crate::appop::AppOp;

use crate::backend::BKCommand;
use crate::backend::BKResponse;
use crate::backend::Backend;
use crate::cache;

use std::sync::mpsc::channel;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;

use crate::app::backend_loop;

use crate::passwd::PasswordStorage;

use crate::actions::AppState;

use super::LoginData;

impl AppOp {
    pub fn bk_login(
        &mut self,
        uid: UserId,
        access_token: AccessToken,
        device: Option<String>,
        server_url: Url,
        identity_url: Url,
    ) {
        if self.store_token(uid.clone(), access_token.clone()).is_err() {
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
        self.sync(true, 0);
        self.init_protocols();
    }

    pub fn bk_logout(&mut self) {
        self.set_rooms(vec![], true);
        if cache::get().destroy().is_err() {
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

        let tx = self.backend.clone();
        thread::spawn(
            move || match register::login(username, password, server.clone()) {
                Ok((uid, tk, dev)) => {
                    APPOP!(bk_login, (uid, tk, dev, server, identity));
                }
                Err(err) => {
                    tx.send(BKCommand::SendBKResponse(BKResponse::LoginError(err)))
                        .expect_log("Connection closed");
                }
            },
        );
    }

    pub fn disconnect(&self) {
        self.backend.send(BKCommand::ShutDown).unwrap();
    }

    pub fn logout(&mut self) {
        let login_data = unwrap_or_unit_return!(self.login_data.clone());
        let _ = self.delete_pass("fractal");
        let tx = self.backend.clone();
        thread::spawn(move || {
            match register::logout(login_data.server_url, login_data.access_token) {
                Ok(_) => {
                    APPOP!(bk_logout);
                }
                Err(err) => {
                    tx.send(BKCommand::SendBKResponse(BKResponse::LogoutError(err)))
                        .expect_log("Connection closed");
                }
            }
        });
        self.bk_logout();
        *self.room_back_history.borrow_mut() = vec![];
    }
}
